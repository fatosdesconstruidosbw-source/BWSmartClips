//! Localização das ferramentas externas: ffmpeg, ffprobe, yt-dlp e whisper-cli.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::config::{find_model_file, models_dir, models_search_dirs, tools_dir, Settings};

#[derive(Debug, Clone, Serialize)]
pub struct Tools {
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    pub ytdlp: Option<PathBuf>,
    pub whisper: Option<PathBuf>,
    pub whisper_model: Option<PathBuf>,
    /// Detector de voz (Silero) do whisper.cpp.
    pub vad_model: Option<PathBuf>,
    /// `llama-server` do llama.cpp (IA local).
    pub llama: Option<PathBuf>,
    /// `llama-server` com suporte a placa de vídeo (Vulkan), instalado à parte.
    pub llama_gpu: Option<PathBuf>,
    /// Modelo .gguf da IA local.
    pub llm_model: Option<PathBuf>,
    /// Gerador de token do YouTube (bgutil-pot) e a pasta de plugins do yt-dlp.
    pub pot: Option<(PathBuf, PathBuf)>,
}

impl Tools {
    pub fn locate(settings: &Settings) -> Self {
        settings.apply();
        Self {
            ffmpeg: find_exe("ffmpeg", &settings.ffmpeg_path),
            ffprobe: find_exe("ffprobe", &settings.ffprobe_path),
            ytdlp: find_exe("yt-dlp", &settings.ytdlp_path),
            whisper: find_exe("whisper-cli", &settings.whisper_path)
                .or_else(|| find_exe("whisper-cpp", ""))
                .or_else(|| find_exe("whisper", "").filter(|p| is_whisper_cpp(p))),
            whisper_model: find_model(&settings.whisper_model),
            vad_model: find_in_models(|n| n.starts_with("ggml-silero") && n.ends_with(".bin"), false),
            llama: find_exe("llama-server", &settings.llama_path),
            llama_gpu: Some(tools_dir().join("llama-gpu").join(exe_name("llama-server"))).filter(|p| p.is_file()),
            llm_model: find_llm_model(&settings.llm_model),
            pot: find_pot(),
        }
    }

    pub fn llama(&self) -> Result<&Path> {
        self.llama.as_deref().ok_or_else(|| missing("llama-server (llama.cpp)"))
    }
    pub fn llm_model(&self) -> Result<&Path> {
        self.llm_model.as_deref().ok_or_else(|| {
            anyhow!(
                "Nenhum modelo de IA local encontrado. Use \"Instalar ferramentas\" ou coloque um arquivo .gguf em {}",
                models_dir().display()
            )
        })
    }

    pub fn ffmpeg(&self) -> Result<&Path> {
        self.ffmpeg.as_deref().ok_or_else(|| missing("ffmpeg"))
    }
    pub fn ffprobe(&self) -> Result<&Path> {
        self.ffprobe.as_deref().ok_or_else(|| missing("ffprobe"))
    }
    pub fn ytdlp(&self) -> Result<&Path> {
        self.ytdlp.as_deref().ok_or_else(|| missing("yt-dlp"))
    }
    pub fn whisper(&self) -> Result<&Path> {
        self.whisper.as_deref().ok_or_else(|| missing("whisper-cli (whisper.cpp)"))
    }
    pub fn whisper_model(&self) -> Result<&Path> {
        self.whisper_model.as_deref().ok_or_else(|| {
            anyhow!(
                "Nenhum modelo do Whisper encontrado. Use \"Instalar ferramentas\" ou coloque um arquivo ggml-*.bin em {}",
                models_dir().display()
            )
        })
    }
}

fn missing(name: &str) -> anyhow::Error {
    anyhow!(
        "{name} não encontrado. Use o botão \"Instalar ferramentas\" nas configurações, \
         informe o caminho manualmente ou instale-o no PATH."
    )
}

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Procura: caminho configurado → pasta de ferramentas do app → pasta do executável → PATH.
pub fn find_exe(name: &str, configured: &str) -> Option<PathBuf> {
    let configured = configured.trim();
    if !configured.is_empty() {
        let p = PathBuf::from(configured);
        return p.is_file().then_some(p);
    }
    let file = exe_name(name);
    let mut dirs: Vec<PathBuf> = Vec::new();
    let td = tools_dir();
    dirs.push(td.clone());
    dirs.push(td.join("whisper"));
    dirs.push(td.join("ffmpeg"));
    dirs.push(td.join("llama"));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            dirs.push(d.to_path_buf());
            dirs.push(d.join("tools"));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.into_iter().map(|d| d.join(&file)).find(|p| p.is_file())
}

fn is_whisper_cpp(p: &Path) -> bool {
    // O "whisper" do Python não aceita -m; só aceitamos se estiver na pasta do app.
    p.starts_with(tools_dir())
}

/// Pasta de plugins do yt-dlp mantida pelo app.
pub fn ytdlp_plugins_dir() -> PathBuf {
    tools_dir().join("yt-dlp-plugins")
}

/// Nome da pasta do plugin do provedor de token dentro de [`ytdlp_plugins_dir`].
pub const POT_PLUGIN: &str = "bgutil-ytdlp-pot-provider";

fn find_pot() -> Option<(PathBuf, PathBuf)> {
    let exe = find_exe("bgutil-pot", "")?;
    let plugins = ytdlp_plugins_dir();
    plugins.join(POT_PLUGIN).join("yt_dlp_plugins").is_dir().then_some((exe, plugins))
}

/// Modelo Whisper configurado ou o maior `ggml-*.bin` da pasta de modelos.
pub fn find_model(configured: &str) -> Option<PathBuf> {
    let configured = configured.trim();
    if !configured.is_empty() {
        let p = PathBuf::from(configured);
        return p.is_file().then_some(p);
    }
    find_in_models(|n| n.starts_with("ggml-") && n.ends_with(".bin") && !n.contains("silero"), true)
}

/// Modelo de IA configurado ou um `.gguf` da pasta de modelos
/// (prefere o recomendado; senão o menor, que é o mais rápido).
pub fn find_llm_model(configured: &str) -> Option<PathBuf> {
    let configured = configured.trim();
    if !configured.is_empty() {
        let p = PathBuf::from(configured);
        return p.is_file().then_some(p);
    }
    let recommended = crate::installer::LLM_MODELS.iter().find(|m| m.recommended).map(|m| m.file);
    if let Some(file) = recommended {
        if let Some(p) = find_model_file(file) {
            return Some(p);
        }
    }
    find_in_models(|n| n.ends_with(".gguf") && !n.starts_with("mmproj"), false)
}

/// Todos os arquivos das pastas de modelos que passam no filtro (nome em minúsculas).
/// Se o mesmo nome existir nas duas pastas, vale o da pasta atual.
pub fn model_files(filter: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for dir in models_search_dirs() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for p in rd.filter_map(|e| e.ok()).map(|e| e.path()) {
            let Some(name) = p.file_name().and_then(|n| n.to_str()).map(str::to_lowercase) else { continue };
            if p.is_file() && filter(&name) && !out.iter().any(|o| o.file_name() == p.file_name()) {
                out.push(p);
            }
        }
    }
    out
}

/// Procura nas pastas de modelos; `largest` escolhe o maior arquivo, senão o menor.
fn find_in_models(filter: impl Fn(&str) -> bool, largest: bool) -> Option<PathBuf> {
    let files = model_files(filter).into_iter();
    let size = |p: &PathBuf| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    if largest {
        files.max_by_key(size)
    } else {
        files.min_by_key(size)
    }
}
