//! Orquestra o trabalho completo: fonte → transcrição → IA → cortes renderizados.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cache::{params_key, source_key, Cache};
use crate::clips::{refine, ClipPlan};
use crate::config::{ClipOptions, LlmProvider, Settings, TranscriptionEngine};
use crate::download;
use crate::llm;
use crate::local_ai;
use crate::media::{self, MediaInfo};
use crate::process::{is_cancelled_error, Cancel};
use crate::selector;
use crate::render::{self, RenderJob};
use crate::tools::Tools;
use crate::transcribe;
use crate::transcript::{Transcript, Word};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Source {
    File(PathBuf),
    Url(String),
}

impl Source {
    /// Interpreta o texto digitado: link → Url, caso contrário → arquivo.
    pub fn parse(s: &str) -> Self {
        let s = s.trim().trim_matches('"');
        if download::looks_like_url(s) {
            Source::Url(s.to_string())
        } else {
            Source::File(PathBuf::from(s))
        }
    }
}

/// Etapas do trabalho, enviadas para a interface.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Download,
    Probe,
    Transcribe,
    Analyze,
    Render,
    Done,
}

#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub stage: Stage,
    /// Progresso total, 0.0–1.0.
    pub overall: f32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipResult {
    pub index: usize,
    #[serde(flatten)]
    pub plan: ClipPlan,
    pub file: PathBuf,
    pub thumbnail: Option<PathBuf>,
    pub transcript: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub project_dir: PathBuf,
    pub source_title: String,
    pub source_file: PathBuf,
    pub clips: Vec<ClipResult>,
    /// Avisos para o usuário (ex.: IA local indisponível, usado plano B).
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// Faixas de progresso de cada etapa.
fn span(stage: Stage) -> (f32, f32) {
    match stage {
        Stage::Download => (0.0, 0.12),
        Stage::Probe => (0.12, 0.14),
        Stage::Transcribe => (0.14, 0.45),
        Stage::Analyze => (0.45, 0.65),
        Stage::Render => (0.65, 1.0),
        Stage::Done => (1.0, 1.0),
    }
}

fn report(cb: &mut dyn FnMut(Progress), stage: Stage, local: f32, message: impl Into<String>) {
    let (a, b) = span(stage);
    cb(Progress { stage, overall: a + (b - a) * local.clamp(0.0, 1.0), message: message.into() });
}

pub fn slug(s: &str, max: usize) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let c = deunicode_char(c);
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
        if out.len() >= max {
            break;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "video".into()
    } else {
        out
    }
}

fn deunicode_char(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
        'ç' | 'Ç' => 'c',
        'ñ' | 'Ñ' => 'n',
        other => other,
    }
}

/// Executa o trabalho inteiro.
pub fn run_job(
    source: &Source,
    opts: &ClipOptions,
    settings: &Settings,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<JobResult> {
    let tools = Tools::locate(settings);
    let ffmpeg = tools.ffmpeg()?.to_path_buf();
    let ffprobe = tools.ffprobe()?.to_path_buf();
    let base = settings.output_base();
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    // Downloads são sempre reaproveitados (se o cache estiver ligado);
    // transcrição e análise são refeitas quando o usuário pede "refazer do zero".
    let download_cache = Cache::new(settings.use_cache);
    let cache = Cache::new(settings.use_cache && !opts.reanalyze);
    let mut warnings = Vec::new();

    // 1) Fonte
    let (source_file, title, project_dir) = match source {
        Source::Url(url) => match download_cache.downloaded(url) {
            Some((file, title)) => {
                report(on_progress, Stage::Download, 1.0, "Vídeo já baixado antes — reaproveitando");
                let dir = base.join(format!("{stamp}-{}", slug(&title, 40)));
                (file, title, dir)
            }
            None => {
                let ytdlp = tools.ytdlp()?.to_path_buf();
                let dir = base.join(format!("{stamp}-download"));
                report(on_progress, Stage::Download, 0.0, format!("Baixando do {}…", download::platform(url)));
                let cookies = settings.youtube_cookies_file.trim();
                let cookies_file = Some(PathBuf::from(cookies))
                    .filter(|p| !cookies.is_empty() && p.is_file())
                    .or_else(|| {
                        let id = download::platform_id(url);
                        Some(crate::config::login_cookies_path(id)).filter(|p| !id.is_empty() && p.is_file())
                    });
                let auth = download::Auth {
                    cookies_file,
                    browser: Some(settings.cookies_browser.trim().to_string()).filter(|b| !b.is_empty()),
                    pot: tools.pot.clone(),
                };
                let site = download::platform(url);
                let d = download::download(&ytdlp, &ffmpeg, url, &dir, &auth, cancel, |p, m| {
                    let msg = if m.contains("pediu login") {
                        m.to_string()
                    } else {
                        format!("Baixando do {site}… {:.0}%", p * 100.0)
                    };
                    report(on_progress, Stage::Download, p, msg)
                })?;
                let final_dir = base.join(format!("{stamp}-{}", slug(&d.title, 40)));
                let (file, dir) = match std::fs::rename(&dir, &final_dir) {
                    Ok(()) => (final_dir.join(d.path.file_name().unwrap_or_default()), final_dir),
                    Err(_) => (d.path.clone(), dir),
                };
                download_cache.remember_download(url, &file, &d.title);
                (file, d.title, dir)
            }
        },
        Source::File(path) => {
            if !path.is_file() {
                anyhow::bail!("arquivo não encontrado: {}", path.display());
            }
            let title = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let dir = base.join(format!("{stamp}-{}", slug(&title, 40)));
            (path.clone(), title, dir)
        }
    };
    std::fs::create_dir_all(&project_dir)
        .with_context(|| format!("não foi possível criar a pasta {}", project_dir.display()))?;
    let work = project_dir.join(".work");
    std::fs::create_dir_all(&work)?;
    let src_key = source_key(&source_file).unwrap_or_else(|| stamp.clone());

    // 2) Informações
    report(on_progress, Stage::Probe, 0.0, "Lendo informações do vídeo…");
    let info = media::probe(&ffprobe, &source_file, cancel)?;
    if !info.has_audio {
        anyhow::bail!("o vídeo não tem áudio — não há fala para transcrever");
    }

    // 3) Transcrição (com cache)
    let engine_id = match settings.transcription {
        TranscriptionEngine::Local => format!(
            "local:{}:{}",
            tools.whisper_model.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            settings.whisper_vad && tools.vad_model.is_some()
        ),
        TranscriptionEngine::OpenAi => format!("api:{}:{}", settings.openai_base_url, settings.openai_transcription_model),
    };
    let tkey = format!("{src_key}-{}", params_key(&(&engine_id, &opts.language)));
    let transcript = match cache.get::<Transcript>("transcripts", &tkey) {
        Some(t) => {
            report(on_progress, Stage::Transcribe, 1.0, "Transcrição reaproveitada do cache");
            t
        }
        None => {
            report(on_progress, Stage::Transcribe, 0.0, "Transcrevendo…");
            let t = transcribe::transcribe(
                settings,
                &tools,
                &source_file,
                info.duration,
                &opts.language,
                &work,
                cancel,
                &mut |p, m| report(on_progress, Stage::Transcribe, p, m),
            )?;
            Cache::new(settings.use_cache).put("transcripts", &tkey, &t);
            t
        }
    };
    std::fs::write(project_dir.join("transcricao.json"), serde_json::to_string_pretty(&transcript)?)?;

    // 4) Análise (com cache)
    let model_id = match settings.llm_provider {
        LlmProvider::Local => tools.llm_model.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        LlmProvider::Ollama => settings.ollama_model.clone(),
        LlmProvider::Anthropic => settings.anthropic_model.clone(),
        LlmProvider::OpenAi => settings.openai_llm_model.clone(),
    };
    let akey = format!(
        "{src_key}-{}",
        params_key(&(
            &tkey,
            settings.llm_provider,
            &model_id,
            opts.clip_count,
            opts.min_duration,
            opts.max_duration,
            &opts.focus,
            settings.llm_candidates,
        ))
    );
    let plans = match cache.get::<Vec<ClipPlan>>("analysis", &akey) {
        Some(p) if !p.is_empty() => {
            report(on_progress, Stage::Analyze, 1.0, "Análise reaproveitada do cache");
            p
        }
        _ => {
            let (plans, complete) = analyze(settings, &tools, &transcript, &info, &ffmpeg, &source_file, opts, &work, cancel, &mut warnings, on_progress)?;
            // Resultados do plano B (sem IA) não vão para o cache, para a IA ser usada da próxima vez.
            if complete {
                Cache::new(settings.use_cache).put("analysis", &akey, &plans);
            }
            plans
        }
    };
    if plans.is_empty() {
        anyhow::bail!("não foram encontrados trechos válidos com a duração pedida — tente aumentar a duração máxima");
    }

    // 5) Renderização
    let clips = render_plans(&plans, &transcript, &info, &ffmpeg, &source_file, opts, settings, &project_dir, &work, cancel, on_progress)?;

    let result = JobResult { project_dir: project_dir.clone(), source_title: title, source_file, clips, warnings };
    std::fs::write(project_dir.join("cortes.json"), serde_json::to_string_pretty(&result)?)?;
    let _ = std::fs::remove_dir_all(&work);
    report(on_progress, Stage::Done, 1.0, format!("{} cortes prontos!", result.clips.len()));
    Ok(result)
}

/// Escolhe os cortes. Devolve também se a IA foi usada (`false` = plano B por regras).
#[allow(clippy::too_many_arguments)]
fn analyze(
    settings: &Settings,
    tools: &Tools,
    transcript: &Transcript,
    info: &MediaInfo,
    ffmpeg: &Path,
    source_file: &Path,
    opts: &ClipOptions,
    work: &Path,
    cancel: &Cancel,
    warnings: &mut Vec<String>,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<(Vec<ClipPlan>, bool)> {
    if !settings.llm_provider.is_local() {
        report(on_progress, Stage::Analyze, 0.0, "IA procurando os melhores momentos…");
        let candidates = llm::find_candidates(settings, transcript, opts, cancel, &mut |p, m| {
            report(on_progress, Stage::Analyze, p, m)
        })?;
        return Ok((refine(candidates, transcript, opts, info.duration), true));
    }

    report(on_progress, Stage::Analyze, 0.0, "Medindo a energia do áudio…");
    let levels = media::loudness_profile(ffmpeg, source_file, cancel).ok();
    cancel.check()?;
    report(on_progress, Stage::Analyze, 0.05, "Carregando a IA local…");
    let backend = match local_ai::start_backend(settings, tools, work, cancel) {
        Ok(b) => {
            if let Some(w) = b.warning {
                warnings.push(w);
            }
            let msg = match &b.gpu {
                Some(g) => format!("IA local na placa de vídeo: {}", g.split(" (").next().unwrap_or(g)),
                None => "IA local no processador".to_string(),
            };
            report(on_progress, Stage::Analyze, 0.08, &msg);
            Some(b.chat)
        }
        Err(e) if is_cancelled_error(&e) => return Err(e),
        Err(e) => {
            warnings.push(format!(
                "A IA local não pôde ser usada ({}). Os cortes foram escolhidos só por regras automáticas — \
                 instale a IA local em Configurações → Ferramentas para resultados melhores.",
                first_line(&format!("{e:#}"))
            ));
            None
        }
    };
    let (plans, report) = selector::select(
        backend.as_deref(),
        transcript,
        levels.as_deref(),
        opts,
        settings.llm_candidates,
        info.duration,
        cancel,
        &mut |p, m| report(on_progress, Stage::Analyze, 0.08 + p * 0.92, m),
    )?;
    if report.failed > 0 {
        warnings.push(format!(
            "A IA local falhou em {} de {} trechos (esses ficaram com a nota das regras). Primeiro erro: {}",
            report.failed,
            report.ok + report.failed,
            first_line(report.first_error.as_deref().unwrap_or_default())
        ));
    }
    // Só vai para o cache se a IA avaliou a maioria dos trechos.
    let trustworthy = backend.is_some() && report.ok >= report.failed;
    Ok((plans, trustworthy))
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or_default().to_string()
}

/// Renderiza uma lista de cortes já planejados (usado também pela CLI).
#[allow(clippy::too_many_arguments)]
pub fn render_plans(
    plans: &[ClipPlan],
    transcript: &Transcript,
    info: &MediaInfo,
    ffmpeg: &Path,
    source_file: &Path,
    opts: &ClipOptions,
    settings: &Settings,
    project_dir: &Path,
    work: &Path,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<Vec<ClipResult>> {
    let total = plans.len();
    let mut clips = Vec::with_capacity(total);
    for (i, plan) in plans.iter().enumerate() {
        cancel.check()?;
        let name = format!("{:02}-{}", i + 1, slug(&plan.title, 50));
        let output = project_dir.join(format!("{name}.mp4"));
        let clip_work = work.join(&name);
        let words: Vec<Word> = transcript
            .words_between(plan.start, plan.end)
            .iter()
            .map(|w| Word { start: w.start - plan.start, end: (w.end - plan.start).min(plan.duration()), text: w.text.clone() })
            .collect();
        let job = RenderJob {
            ffmpeg,
            input: source_file,
            info,
            start: plan.start,
            end: plan.end,
            words: &words,
            reframe: opts.reframe,
            captions: &opts.captions,
            output_height: settings.output_height,
            x264_preset: &settings.x264_preset,
            x264_crf: settings.x264_crf,
            cam_ratio: opts.cam_ratio,
            facecam: opts.facecam,
            work_dir: &clip_work,
            output: &output,
        };
        let label = format!("Corte {}/{total}", i + 1);
        let mut progress = |p: f32, m: &str| report(on_progress, Stage::Render, (i as f32 + p) / total as f32, format!("{label}: {m}"));
        // Um erro interno (panic) num corte não pode derrubar o trabalho em silêncio:
        // vira erro normal e, no Gameplay, tentamos de novo com "Preencher".
        let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render::render(&job, cancel, &mut progress)));
        match first {
            Ok(r) => r?,
            Err(panic) => {
                let msg = panic_message(&panic);
                if job.reframe == crate::config::ReframeMode::Gameplay {
                    progress(0.1, "Erro no layout Gameplay — refazendo com Preencher");
                    let fallback = RenderJob { reframe: crate::config::ReframeMode::Center, ..job };
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render::render(&fallback, cancel, &mut progress)))
                        .map_err(|p| anyhow::anyhow!("erro interno ao renderizar o corte {}: {}", i + 1, panic_message(&p)))??;
                } else {
                    anyhow::bail!("erro interno ao renderizar o corte {}: {msg}", i + 1);
                }
            }
        }
        let thumb = project_dir.join(format!("{name}.jpg"));
        let thumbnail = media::thumbnail(ffmpeg, &output, &thumb, plan.duration().min(2.0) / 2.0, cancel)
            .ok()
            .map(|_| thumb);
        let _ = std::fs::remove_dir_all(&clip_work);
        clips.push(ClipResult {
            index: i + 1,
            plan: plan.clone(),
            file: output,
            thumbnail,
            transcript: transcript.text_between(plan.start, plan.end),
        });
    }
    Ok(clips)
}

/// Texto de um panic capturado.
pub fn panic_message(p: &Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "erro desconhecido".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slug("Como Ganhar R$ 1.000 em São Paulo!", 60), "como-ganhar-r-1-000-em-sao-paulo");
        assert_eq!(slug("???", 10), "video");
    }

    #[test]
    fn source_parse() {
        assert!(matches!(Source::parse(" https://youtu.be/x "), Source::Url(_)));
        assert!(matches!(Source::parse("\"C:\\v.mp4\""), Source::File(_)));
    }
}
