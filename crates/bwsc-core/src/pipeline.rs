//! Orquestra o trabalho completo: fonte → transcrição → IA → cortes renderizados.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::clips::{refine, ClipPlan};
use crate::config::{ClipOptions, Settings};
use crate::download;
use crate::llm;
use crate::media::{self, MediaInfo};
use crate::process::Cancel;
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
}

/// Faixas de progresso de cada etapa.
fn span(stage: Stage) -> (f32, f32) {
    match stage {
        Stage::Download => (0.0, 0.12),
        Stage::Probe => (0.12, 0.14),
        Stage::Transcribe => (0.14, 0.50),
        Stage::Analyze => (0.50, 0.58),
        Stage::Render => (0.58, 1.0),
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
    let base = PathBuf::from(&settings.output_dir);
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();

    // 1) Fonte
    let (source_file, title, project_dir) = match source {
        Source::Url(url) => {
            let ytdlp = tools.ytdlp()?.to_path_buf();
            let dir = base.join(format!("{stamp}-download"));
            report(on_progress, Stage::Download, 0.0, "Baixando vídeo…");
            let d = download::download(&ytdlp, &ffmpeg, url, &dir, cancel, |p, _| {
                report(on_progress, Stage::Download, p, format!("Baixando vídeo… {:.0}%", p * 100.0))
            })?;
            let final_dir = base.join(format!("{stamp}-{}", slug(&d.title, 40)));
            let (file, dir) = match std::fs::rename(&dir, &final_dir) {
                Ok(()) => (final_dir.join(d.path.file_name().unwrap_or_default()), final_dir),
                Err(_) => (d.path.clone(), dir),
            };
            (file, d.title, dir)
        }
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

    // 2) Informações
    report(on_progress, Stage::Probe, 0.0, "Lendo informações do vídeo…");
    let info = media::probe(&ffprobe, &source_file, cancel)?;
    if !info.has_audio {
        anyhow::bail!("o vídeo não tem áudio — não há fala para transcrever");
    }

    // 3) Transcrição
    report(on_progress, Stage::Transcribe, 0.0, "Transcrevendo…");
    let transcript = transcribe::transcribe(
        settings,
        &tools,
        &source_file,
        info.duration,
        &opts.language,
        &work,
        cancel,
        &mut |p, m| report(on_progress, Stage::Transcribe, p, m),
    )?;
    std::fs::write(project_dir.join("transcricao.json"), serde_json::to_string_pretty(&transcript)?)?;

    // 4) IA
    report(on_progress, Stage::Analyze, 0.0, "IA procurando os melhores momentos…");
    let candidates = llm::find_candidates(settings, &transcript, opts, cancel, &mut |p, m| {
        report(on_progress, Stage::Analyze, p, m)
    })?;
    let plans = refine(candidates, &transcript, opts, info.duration);
    if plans.is_empty() {
        anyhow::bail!("a IA não encontrou trechos válidos com a duração pedida — tente aumentar a duração máxima");
    }

    // 5) Renderização
    let clips = render_plans(&plans, &transcript, &info, &ffmpeg, &source_file, opts, settings, &project_dir, &work, cancel, on_progress)?;

    let result = JobResult { project_dir: project_dir.clone(), source_title: title, source_file, clips };
    std::fs::write(project_dir.join("cortes.json"), serde_json::to_string_pretty(&result)?)?;
    let _ = std::fs::remove_dir_all(&work);
    report(on_progress, Stage::Done, 1.0, format!("{} cortes prontos!", result.clips.len()));
    Ok(result)
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
            work_dir: &clip_work,
            output: &output,
        };
        let label = format!("Corte {}/{total}", i + 1);
        render::render(&job, cancel, &mut |p, m| {
            report(on_progress, Stage::Render, (i as f32 + p) / total as f32, format!("{label}: {m}"))
        })?;
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
