//! Localização das ferramentas externas: ffmpeg, ffprobe, yt-dlp e whisper-cli.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::config::{models_dir, tools_dir, Settings};

#[derive(Debug, Clone, Serialize)]
pub struct Tools {
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    pub ytdlp: Option<PathBuf>,
    pub whisper: Option<PathBuf>,
    pub whisper_model: Option<PathBuf>,
}

impl Tools {
    pub fn locate(settings: &Settings) -> Self {
        Self {
            ffmpeg: find_exe("ffmpeg", &settings.ffmpeg_path),
            ffprobe: find_exe("ffprobe", &settings.ffprobe_path),
            ytdlp: find_exe("yt-dlp", &settings.ytdlp_path),
            whisper: find_exe("whisper-cli", &settings.whisper_path)
                .or_else(|| find_exe("whisper-cpp", ""))
                .or_else(|| find_exe("whisper", "").filter(|p| is_whisper_cpp(p))),
            whisper_model: find_model(&settings.whisper_model),
        }
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

/// Modelo configurado ou o maior `ggml-*.bin` da pasta de modelos.
pub fn find_model(configured: &str) -> Option<PathBuf> {
    let configured = configured.trim();
    if !configured.is_empty() {
        let p = PathBuf::from(configured);
        return p.is_file().then_some(p);
    }
    let entries = std::fs::read_dir(models_dir()).ok()?;
    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("ggml-") && n.ends_with(".bin"))
        })
        .max_by_key(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
}
