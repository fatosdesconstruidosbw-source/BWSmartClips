//! Instalador das ferramentas externas (Windows): ffmpeg, yt-dlp, Deno, whisper.cpp e modelo.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::config::{models_dir, tools_dir};
use crate::process::{Cancel, Cancelled};

pub const WHISPER_MODELS: &[(&str, &str)] = &[
    ("tiny", "75 MB — muito rápido, precisão baixa"),
    ("base", "142 MB — rápido"),
    ("small", "466 MB — bom equilíbrio (recomendado para CPU)"),
    ("medium", "1,5 GB — mais preciso, lento em CPU"),
    ("large-v3-turbo-q5_0", "547 MB — muito preciso, exige PC forte"),
];

const YTDLP_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const FFMPEG_URL: &str = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
const WHISPER_URL: &str = "https://github.com/ggml-org/whisper.cpp/releases/latest/download/whisper-bin-x64.zip";
const DENO_URL: &str = "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip";

pub fn model_url(name: &str) -> String {
    format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{name}.bin")
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallProgress {
    pub step: String,
    /// 0.0–1.0 do passo atual.
    pub progress: f32,
    pub message: String,
}

/// Baixa e instala tudo o que falta. Com `force`, baixa de novo (atualização).
pub fn install_tools(
    model: &str,
    force: bool,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(InstallProgress),
) -> Result<()> {
    if !cfg!(windows) {
        bail!(
            "O instalador automático é para Windows. No Linux/macOS instale pelo gerenciador de pacotes: \
             ffmpeg, yt-dlp e whisper.cpp (whisper-cli), e coloque o modelo ggml em {}",
            models_dir().display()
        );
    }
    let tools = tools_dir();
    let models = models_dir();
    std::fs::create_dir_all(&tools)?;
    std::fs::create_dir_all(&models)?;
    let tmp = tools.join("_download");
    std::fs::create_dir_all(&tmp)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3600))
        .connect_timeout(Duration::from_secs(30))
        .user_agent("BWSmartClips")
        .build()?;

    let mut step = |name: &str, p: f32, msg: String| {
        on_progress(InstallProgress { step: name.into(), progress: p, message: msg })
    };

    // yt-dlp
    let ytdlp = tools.join("yt-dlp.exe");
    if force || !ytdlp.exists() {
        download_file(&client, YTDLP_URL, &ytdlp, cancel, &mut |p| step("yt-dlp", p, "Baixando yt-dlp…".into()))?;
    }

    // ffmpeg + ffprobe
    if force || !tools.join("ffmpeg.exe").exists() || !tools.join("ffprobe.exe").exists() {
        let zip = tmp.join("ffmpeg.zip");
        download_file(&client, FFMPEG_URL, &zip, cancel, &mut |p| step("ffmpeg", p, "Baixando ffmpeg…".into()))?;
        step("ffmpeg", 1.0, "Extraindo ffmpeg…".into());
        extract_named(&zip, &tools, &["ffmpeg.exe", "ffprobe.exe"])?;
    }

    // Deno (runtime de JavaScript usado pelo yt-dlp no YouTube)
    if force || !tools.join("deno.exe").exists() {
        let zip = tmp.join("deno.zip");
        match download_file(&client, DENO_URL, &zip, cancel, &mut |p| step("deno", p, "Baixando Deno (para o YouTube)…".into())) {
            Ok(()) => extract_named(&zip, &tools, &["deno.exe"])?,
            Err(e) if e.downcast_ref::<Cancelled>().is_some() => return Err(e),
            // Não é fatal: o yt-dlp ainda funciona em muitos sites sem ele.
            Err(_) => step("deno", 1.0, "Não foi possível baixar o Deno (opcional)".into()),
        }
    }

    // whisper.cpp
    let whisper_dir = tools.join("whisper");
    if force || !whisper_dir.join("whisper-cli.exe").exists() {
        let zip = tmp.join("whisper.zip");
        download_file(&client, WHISPER_URL, &zip, cancel, &mut |p| step("whisper", p, "Baixando whisper.cpp…".into()))?;
        step("whisper", 1.0, "Extraindo whisper.cpp…".into());
        std::fs::create_dir_all(&whisper_dir)?;
        extract_flat(&zip, &whisper_dir, &[".exe", ".dll"])?;
        if !whisper_dir.join("whisper-cli.exe").exists() {
            bail!("o pacote do whisper.cpp não contém whisper-cli.exe");
        }
    }

    // Modelo
    if !model.trim().is_empty() {
        let target = models.join(format!("ggml-{}.bin", model.trim()));
        if force || !target.exists() {
            download_file(&client, &model_url(model.trim()), &target, cancel, &mut |p| {
                step("modelo", p, format!("Baixando modelo Whisper \"{model}\"…"))
            })?;
        }
    }

    let _ = std::fs::remove_dir_all(&tmp);
    on_progress(InstallProgress { step: "ok".into(), progress: 1.0, message: "Ferramentas instaladas!".into() });
    Ok(())
}

fn download_file(
    client: &reqwest::blocking::Client,
    url: &str,
    dest: &Path,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(f32),
) -> Result<()> {
    let mut resp = client.get(url).send().with_context(|| format!("falha ao baixar {url}"))?;
    if !resp.status().is_success() {
        bail!("falha ao baixar {url}: {}", resp.status());
    }
    let total = resp.content_length().unwrap_or(0);
    let part = PathBuf::from(format!("{}.part", dest.display()));
    let mut file = File::create(&part).with_context(|| format!("não foi possível criar {}", part.display()))?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut done: u64 = 0;
    loop {
        if cancel.is_cancelled() {
            drop(file);
            let _ = std::fs::remove_file(&part);
            bail!(Cancelled);
        }
        let n = resp.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        done += n as u64;
        if total > 0 {
            on_progress(done as f32 / total as f32);
        }
    }
    file.flush()?;
    drop(file);
    if total > 0 && done < total {
        let _ = std::fs::remove_file(&part);
        bail!("download incompleto de {url}");
    }
    std::fs::rename(&part, dest)?;
    on_progress(1.0);
    Ok(())
}

/// Extrai do zip apenas os arquivos com os nomes dados (em qualquer subpasta).
fn extract_named(zip_path: &Path, dest: &Path, names: &[&str]) -> Result<()> {
    let mut archive = zip::ZipArchive::new(File::open(zip_path)?)?;
    let mut found = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(name) = entry.enclosed_name().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())) else {
            continue;
        };
        if names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            let mut out = File::create(dest.join(&name))?;
            std::io::copy(&mut entry, &mut out)?;
            found += 1;
        }
    }
    if found < names.len() {
        bail!("arquivos esperados não encontrados em {}", zip_path.display());
    }
    Ok(())
}

/// Extrai (sem subpastas) todos os arquivos com as extensões dadas.
fn extract_flat(zip_path: &Path, dest: &Path, exts: &[&str]) -> Result<()> {
    let mut archive = zip::ZipArchive::new(File::open(zip_path)?)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let Some(name) = entry.enclosed_name().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())) else {
            continue;
        };
        let lower = name.to_lowercase();
        if exts.iter().any(|e| lower.ends_with(e)) {
            let mut out = File::create(dest.join(&name))?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}
