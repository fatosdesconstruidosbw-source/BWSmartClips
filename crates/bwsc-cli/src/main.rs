//! `bwsmartclips` — gera cortes pela linha de comando (útil para testes e automação).

use std::io::Write;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use bwsc_core::clips::ClipPlan;
use bwsc_core::config::ReframeMode;
use bwsc_core::pipeline::{render_plans, slug};
use bwsc_core::tools::Tools;
use bwsc_core::transcript::Transcript;
use bwsc_core::{media, run_job, Cancel, Progress, Settings, Source};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "bwsmartclips", version, about = "Cortes virais estilo Opus Clip, feito em Rust")]
struct Cli {
    /// Arquivo de configuração (padrão: o mesmo do app desktop).
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
enum Reframe {
    Face,
    Center,
    Blur,
    Original,
}

#[derive(Subcommand)]
enum Cmd {
    /// Gera cortes a partir de um arquivo ou link.
    Run {
        /// Caminho do vídeo ou link (YouTube etc.).
        source: String,
        #[arg(short = 'n', long)]
        count: Option<usize>,
        #[arg(long)]
        min: Option<f64>,
        #[arg(long)]
        max: Option<f64>,
        /// Idioma (pt, en, es…). Vazio = detectar.
        #[arg(short, long)]
        lang: Option<String>,
        /// Instrução extra para a IA.
        #[arg(short, long)]
        focus: Option<String>,
        #[arg(short, long, value_enum)]
        reframe: Option<Reframe>,
        #[arg(long)]
        no_captions: bool,
        /// Pasta de saída.
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Usa uma transcrição já existente (transcricao.json) em vez de transcrever.
        #[arg(long)]
        transcript: Option<PathBuf>,
        /// Cortes manuais "INICIO-FIM" em segundos (pula a IA). Pode repetir.
        #[arg(long = "clip")]
        clips: Vec<String>,
    },
    /// Mostra onde estão ffmpeg, yt-dlp, whisper e o modelo.
    Tools,
    /// Baixa ferramentas (Windows) e modelos (Whisper, detector de voz e IA local).
    Install {
        #[arg(long, default_value = "small")]
        whisper: String,
        /// Modelo de IA local: qwen3.5-2b, qwen3.5-4b ou qwen3.5-9b ("" = nenhum).
        #[arg(long, default_value = "qwen3.5-4b")]
        llm: String,
        #[arg(long)]
        no_vad: bool,
        #[arg(long)]
        force: bool,
    },
    /// Copia modelos (.gguf/.bin) de arquivos ou pastas para a pasta de modelos.
    Import { paths: Vec<PathBuf> },
    /// Mostra o caminho e o conteúdo da configuração.
    Config,
}

fn main() {
    // Aberto com dois cliques (sem argumentos): explica em vez de fechar na hora.
    if std::env::args().len() == 1 {
        println!("Este é o BWSmartClips de LINHA DE COMANDO (para usar no terminal).\n");
        println!("Para usar o aplicativo com janela, instale o \"BWSmartClips_..._x64-setup.exe\"");
        println!("e abra o \"BWSmartClips\" pelo menu Iniciar.\n");
        println!("Uso no terminal:  bwsmartclips --help");
        if cfg!(windows) {
            println!("\nPressione Enter para fechar...");
            let _ = std::io::stdin().read_line(&mut String::new());
        }
        return;
    }
    if let Err(e) = real_main() {
        eprintln!("\nerro: {e:#}");
        std::process::exit(1);
    }
}

fn print_progress(p: Progress) {
    print!("\r[{:>3.0}%] {:<70}", p.overall * 100.0, p.message.chars().take(70).collect::<String>());
    let _ = std::io::stdout().flush();
}

fn real_main() -> Result<()> {
    let cli = Cli::parse();
    let config_path = cli.config.clone().unwrap_or_else(Settings::default_path);
    let mut settings = Settings::load(&config_path)?;
    let cancel = Cancel::new();

    match cli.cmd {
        Cmd::Tools => {
            println!("{}", serde_json::to_string_pretty(&Tools::locate(&settings))?);
        }
        Cmd::Config => {
            println!("# {}\n{}", config_path.display(), serde_json::to_string_pretty(&settings)?);
        }
        Cmd::Install { whisper, llm, no_vad, force } => {
            let req = bwsc_core::installer::InstallRequest { whisper_model: whisper, llm_model: llm, vad: !no_vad, force };
            println!("A baixar: ~{} MB", bwsc_core::installer::estimate_mb(&req));
            bwsc_core::installer::install(&req, &cancel, &mut |p| {
                print!("\r[{:>3.0}%] {:<70}", p.progress * 100.0, p.message);
                let _ = std::io::stdout().flush();
            })?;
            println!();
        }
        Cmd::Import { paths } => {
            let done = bwsc_core::installer::import_models(&paths, &cancel, &mut |p| {
                print!("\r[{:>3.0}%] {:<70}", p.progress * 100.0, p.message);
                let _ = std::io::stdout().flush();
            })?;
            println!();
            for f in done {
                println!("  {}", f.display());
            }
        }
        Cmd::Run { source, count, min, max, lang, focus, reframe, no_captions, out, transcript, clips } => {
            let mut opts = settings.defaults.clone();
            if let Some(v) = count {
                opts.clip_count = v;
            }
            if let Some(v) = min {
                opts.min_duration = v;
            }
            if let Some(v) = max {
                opts.max_duration = v;
            }
            if let Some(v) = lang {
                opts.language = v;
            }
            if let Some(v) = focus {
                opts.focus = v;
            }
            if let Some(r) = reframe {
                opts.reframe = match r {
                    Reframe::Face => ReframeMode::Face,
                    Reframe::Center => ReframeMode::Center,
                    Reframe::Blur => ReframeMode::Blur,
                    Reframe::Original => ReframeMode::Original,
                };
            }
            if no_captions {
                opts.captions.enabled = false;
            }
            if let Some(o) = out {
                settings.output_dir = o.to_string_lossy().into_owned();
            }
            let source = Source::parse(&source);

            let result = if transcript.is_some() || !clips.is_empty() {
                manual(&source, transcript, &clips, &opts, &settings, &cancel)?
            } else {
                run_job(&source, &opts, &settings, &cancel, &mut print_progress)?
            };
            println!("\n\nPasta: {}", result.project_dir.display());
            for c in &result.clips {
                println!(
                    "  {:>2}. [{:>3.0}] {:>6.1}s–{:>6.1}s  {}\n      {}",
                    c.index,
                    c.plan.score,
                    c.plan.start,
                    c.plan.end,
                    c.plan.title,
                    c.file.display()
                );
            }
        }
    }
    Ok(())
}

/// Modo manual: transcrição pronta e/ou cortes informados à mão.
fn manual(
    source: &Source,
    transcript_path: Option<PathBuf>,
    clips: &[String],
    opts: &bwsc_core::ClipOptions,
    settings: &Settings,
    cancel: &Cancel,
) -> Result<bwsc_core::JobResult> {
    let Source::File(file) = source else {
        bail!("o modo manual (--clip/--transcript) aceita apenas arquivos locais");
    };
    let tools = Tools::locate(settings);
    let ffmpeg = tools.ffmpeg()?.to_path_buf();
    let info = media::probe(tools.ffprobe()?, file, cancel)?;
    let title = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let project_dir = PathBuf::from(&settings.output_dir).join(format!("manual-{}", slug(&title, 40)));
    let work = project_dir.join(".work");
    std::fs::create_dir_all(&work)?;

    let transcript: Transcript = match transcript_path {
        Some(p) => {
            let t: Transcript = serde_json::from_str(&std::fs::read_to_string(&p)?).context("transcrição inválida")?;
            if t.segments.is_empty() { Transcript::from_words(t.words, t.language) } else { t }
        }
        None => bwsc_core::transcribe::transcribe(
            settings,
            &tools,
            file,
            info.duration,
            &opts.language,
            &work,
            cancel,
            &mut |p, m| print!("\r[{:>3.0}%] {m:<60}", p * 100.0),
        )?,
    };

    let plans: Vec<ClipPlan> = if clips.is_empty() {
        let c = bwsc_core::llm::find_candidates(settings, &transcript, opts, cancel, &mut |_, m| print!("\r{m:<70}"))?;
        bwsc_core::clips::refine(c, &transcript, opts, info.duration)
    } else {
        clips
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let (a, b) = c.split_once('-').context("use o formato INICIO-FIM, ex.: 12.5-48")?;
                let start: f64 = bwsc_core::llm::parse_clock(a).context("início inválido")?;
                let end: f64 = bwsc_core::llm::parse_clock(b).context("fim inválido")?;
                if end <= start {
                    bail!("o fim deve ser maior que o início em {c}");
                }
                Ok(ClipPlan {
                    start,
                    end: end.min(info.duration),
                    title: format!("corte {}", i + 1),
                    hook: transcript.text_between(start, start + 4.0),
                    score: 0.0,
                    reason: "manual".into(),
                    hashtags: vec![],
                })
            })
            .collect::<Result<_>>()?
    };
    let results = render_plans(&plans, &transcript, &info, &ffmpeg, file, opts, settings, &project_dir, &work, cancel, &mut print_progress)?;
    let _ = std::fs::remove_dir_all(&work);
    let result = bwsc_core::JobResult { project_dir, source_title: title, source_file: file.clone(), clips: results, warnings: vec![] };
    std::fs::write(result.project_dir.join("cortes.json"), serde_json::to_string_pretty(&result)?)?;
    Ok(result)
}
