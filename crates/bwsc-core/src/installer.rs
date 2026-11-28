//! Instalador: ferramentas (Windows) e modelos de IA (qualquer sistema).
//!
//! Tudo é baixado uma única vez para `%LOCALAPPDATA%\BWSmartClips`; depois o app
//! funciona 100% offline. Também dá para importar modelos de uma pasta/pendrive.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::{app_data_dir, default_models_dir, find_model_file, legacy_data_dir, models_dir, tools_dir, DATA_ITEMS};
use crate::process::{Cancel, Cancelled};

/// Um modelo que o instalador sabe baixar.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    pub size_mb: u64,
    pub recommended: bool,
}

#[cfg(test)]
const HF_WHISPER: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

pub const WHISPER_MODELS: &[ModelInfo] = &[
    ModelInfo { id: "tiny", label: "Tiny — muito rápido, precisão baixa", file: "ggml-tiny.bin", url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin", size_mb: 75, recommended: false },
    ModelInfo { id: "base", label: "Base — rápido", file: "ggml-base.bin", url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin", size_mb: 142, recommended: false },
    ModelInfo { id: "small", label: "Small — bom equilíbrio (recomendado para CPU)", file: "ggml-small.bin", url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin", size_mb: 466, recommended: true },
    ModelInfo { id: "medium", label: "Medium — mais preciso, lento em CPU", file: "ggml-medium.bin", url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin", size_mb: 1530, recommended: false },
    ModelInfo { id: "large-v3-turbo-q5_0", label: "Large v3 Turbo — muito preciso, exige PC forte", file: "ggml-large-v3-turbo-q5_0.bin", url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin", size_mb: 547, recommended: false },
];

pub const LLM_MODELS: &[ModelInfo] = &[
    ModelInfo { id: "qwen3.5-2b", label: "Qwen 3.5 2B — o mais rápido, análise mais simples", file: "Qwen_Qwen3.5-2B-Q4_K_M.gguf", url: "https://huggingface.co/bartowski/Qwen_Qwen3.5-2B-GGUF/resolve/main/Qwen_Qwen3.5-2B-Q4_K_M.gguf", size_mb: 1400, recommended: false },
    ModelInfo { id: "qwen3.5-4b", label: "Qwen 3.5 4B — bom equilíbrio (recomendado)", file: "Qwen_Qwen3.5-4B-Q4_K_M.gguf", url: "https://huggingface.co/bartowski/Qwen_Qwen3.5-4B-GGUF/resolve/main/Qwen_Qwen3.5-4B-Q4_K_M.gguf", size_mb: 3010, recommended: true },
    ModelInfo { id: "qwen3.5-9b", label: "Qwen 3.5 9B — mais esperto, bem mais lento em CPU", file: "Qwen_Qwen3.5-9B-Q4_K_M.gguf", url: "https://huggingface.co/bartowski/Qwen_Qwen3.5-9B-GGUF/resolve/main/Qwen_Qwen3.5-9B-Q4_K_M.gguf", size_mb: 6170, recommended: false },
];

pub const VAD_MODEL: ModelInfo = ModelInfo {
    id: "silero",
    label: "Detector de voz Silero",
    file: "ggml-silero-v5.1.2.bin",
    url: "https://huggingface.co/ggml-org/whisper-vad/resolve/main/ggml-silero-v5.1.2.bin",
    size_mb: 1,
    recommended: true,
};

/// Tamanho aproximado das ferramentas (ffmpeg, yt-dlp, Deno, whisper.cpp, llama.cpp).
pub const TOOLS_SIZE_MB: u64 = 380;
/// Tamanho aproximado do llama.cpp para placa de vídeo (Vulkan).
pub const LLAMA_GPU_SIZE_MB: u64 = 60;

const YTDLP_REPO: &str = "yt-dlp/yt-dlp";
/// Espelho oficial no GitHub das versões do ffmpeg de gyan.dev (com SHA-256 publicado).
const FFMPEG_REPO: &str = "GyanD/codexffmpeg";
const WHISPER_REPO: &str = "ggml-org/whisper.cpp";
const WHISPER_ASSET: &str = "whisper-bin-x64.zip";
const LLAMA_REPO: &str = "ggml-org/llama.cpp";
/// Gerador de "token de origem" do YouTube (permite baixar sem login na maioria dos casos).
const POT_REPO: &str = "jim60105/bgutil-ytdlp-pot-provider-rs";
const POT_PLUGIN_ZIP: &str = "bgutil-ytdlp-pot-provider-rs.zip";
const DENO_REPO: &str = "denoland/deno";

pub fn whisper_model(id: &str) -> Option<&'static ModelInfo> {
    WHISPER_MODELS.iter().find(|m| m.id == id)
}
pub fn llm_model(id: &str) -> Option<&'static ModelInfo> {
    LLM_MODELS.iter().find(|m| m.id == id)
}

/// O que instalar. Ids vazios = não baixar aquele modelo.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InstallRequest {
    pub whisper_model: String,
    pub llm_model: String,
    pub vad: bool,
    /// Baixa as ferramentas de novo (atualização).
    pub force: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallProgress {
    pub step: String,
    /// 0.0–1.0 do passo atual.
    pub progress: f32,
    pub message: String,
}

/// Espaço livre (bytes) no disco onde ficam os dados do app.
pub fn free_space() -> Option<u64> {
    let mut dir = models_dir();
    while !dir.exists() {
        dir = dir.parent()?.to_path_buf();
    }
    fs4::available_space(&dir).ok()
}

/// Estimativa (MB) do que falta baixar para este pedido.
pub fn estimate_mb(req: &InstallRequest) -> u64 {
    let mut mb = 0;
    if cfg!(feature = "store") {
        // ferramentas já vêm no pacote
    } else if cfg!(windows) && (req.force || !tools_dir().join("llama").join("llama-server.exe").exists()) {
        mb += TOOLS_SIZE_MB;
    } else if cfg!(windows) && !tools_dir().join("llama-gpu").join("llama-server.exe").exists() {
        mb += LLAMA_GPU_SIZE_MB;
    }
    let missing = |m: &ModelInfo| if find_model_file(m.file).is_some() { 0 } else { m.size_mb };
    if let Some(m) = whisper_model(&req.whisper_model) {
        mb += missing(m);
    }
    if let Some(m) = llm_model(&req.llm_model) {
        mb += missing(m);
    }
    mb
}

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(4 * 3600))
        .connect_timeout(Duration::from_secs(30))
        .user_agent("BWSmartClips")
        .build()?)
}

/// Baixa e instala o que falta.
pub fn install(req: &InstallRequest, cancel: &Cancel, on_progress: &mut dyn FnMut(InstallProgress)) -> Result<()> {
    let models = models_dir();
    std::fs::create_dir_all(&models)?;
    let client = client()?;
    let mut step = |name: &str, p: f32, msg: String| on_progress(InstallProgress { step: name.into(), progress: p, message: msg });

    // A edição da Microsoft Store traz as ferramentas assinadas dentro do pacote e nunca
    // baixa programas: aqui só entram modelos (arquivos de dados).
    if cfg!(windows) && !cfg!(feature = "store") {
        install_windows_tools(&client, req.force, cancel, &mut step)?;
    }

    let mut wanted: Vec<(&str, &ModelInfo)> = Vec::new();
    if req.vad {
        wanted.push(("vad", &VAD_MODEL));
    }
    if let Some(m) = whisper_model(&req.whisper_model) {
        wanted.push(("whisper", m));
    }
    if let Some(m) = llm_model(&req.llm_model) {
        wanted.push(("ia", m));
    }
    for (kind, m) in wanted {
        let target = models.join(m.file);
        if find_model_file(m.file).is_some() {
            continue;
        }
        let what = match kind {
            "vad" => "detector de voz".to_string(),
            "whisper" => format!("modelo Whisper \"{}\"", m.id),
            _ => format!("modelo de IA \"{}\"", m.id),
        };
        download_file(&client, m.url, &target, cancel, &mut |p, done, total| {
            step(kind, p, format!("Baixando {what}… {}", fmt_progress(done, total)))
        })?;
    }

    if !cfg!(windows) {
        step(
            "aviso",
            1.0,
            "Modelos instalados. No Linux/macOS instale ffmpeg, yt-dlp, whisper-cli e llama-server pelo gerenciador de pacotes.".into(),
        );
    }
    on_progress(InstallProgress { step: "ok".into(), progress: 1.0, message: "Tudo instalado!".into() });
    Ok(())
}

fn install_windows_tools(
    client: &reqwest::blocking::Client,
    force: bool,
    cancel: &Cancel,
    step: &mut dyn FnMut(&str, f32, String),
) -> Result<()> {
    let tools = tools_dir();
    std::fs::create_dir_all(&tools)?;
    let tmp = tools.join("_download");
    std::fs::create_dir_all(&tmp)?;

    let ytdlp = tools.join("yt-dlp.exe");
    if force || !ytdlp.exists() {
        let asset = find_release_asset(client, YTDLP_REPO, |n| n == "yt-dlp.exe")?;
        let part = tmp.join("yt-dlp.exe");
        download_verified(client, &asset, &part, cancel, &mut |p, d, t| step("yt-dlp", p, format!("Baixando yt-dlp… {}", fmt_progress(d, t))))?;
        std::fs::rename(&part, &ytdlp).or_else(|_| std::fs::copy(&part, &ytdlp).map(|_| ()))?;
    }

    if force || !tools.join("ffmpeg.exe").exists() || !tools.join("ffprobe.exe").exists() {
        let zip = tmp.join("ffmpeg.zip");
        let asset = find_release_asset(client, FFMPEG_REPO, is_ffmpeg_asset)?;
        download_verified(client, &asset, &zip, cancel, &mut |p, d, t| step("ffmpeg", p, format!("Baixando ffmpeg… {}", fmt_progress(d, t))))?;
        step("ffmpeg", 1.0, "Extraindo ffmpeg…".into());
        extract_named(&zip, &tools, &["ffmpeg.exe", "ffprobe.exe"])?;
    }

    if force || !tools.join("deno.exe").exists() {
        let zip = tmp.join("deno.zip");
        let res = find_release_asset(client, DENO_REPO, |n| n == "deno-x86_64-pc-windows-msvc.zip").and_then(|asset| {
            download_verified(client, &asset, &zip, cancel, &mut |p, d, t| step("deno", p, format!("Baixando Deno (para o YouTube)… {}", fmt_progress(d, t))))
        });
        match res {
            Ok(()) => extract_named(&zip, &tools, &["deno.exe"])?,
            Err(e) if e.downcast_ref::<Cancelled>().is_some() => return Err(e),
            Err(_) => step("deno", 1.0, "Não foi possível baixar o Deno (opcional)".into()),
        }
    }

    let whisper_dir = tools.join("whisper");
    if force || !whisper_dir.join("whisper-cli.exe").exists() {
        step("whisper", 0.0, "Procurando a versão mais recente do whisper.cpp…".into());
        let asset = find_release_asset(client, WHISPER_REPO, |n| n == WHISPER_ASSET)
            .context("não foi possível encontrar o pacote do whisper.cpp para Windows")?;
        let zip = tmp.join("whisper.zip");
        download_verified(client, &asset, &zip, cancel, &mut |p, d, t| step("whisper", p, format!("Baixando whisper.cpp… {}", fmt_progress(d, t))))?;
        step("whisper", 1.0, "Extraindo whisper.cpp…".into());
        std::fs::create_dir_all(&whisper_dir)?;
        extract_flat(&zip, &whisper_dir, &[".exe", ".dll"])?;
        if !whisper_dir.join("whisper-cli.exe").exists() {
            bail!("o pacote do whisper.cpp não contém whisper-cli.exe");
        }
    }

    let llama_dir = tools.join("llama");
    if force || !llama_dir.join("llama-server.exe").exists() {
        step("llama", 0.0, "Procurando a versão mais recente do llama.cpp…".into());
        let asset = find_release_asset(client, LLAMA_REPO, is_llama_cpu_asset)
            .context("não foi possível encontrar o pacote do llama.cpp para Windows")?;
        let zip = tmp.join("llama.zip");
        download_verified(client, &asset, &zip, cancel, &mut |p, d, t| step("llama", p, format!("Baixando llama.cpp (IA local)… {}", fmt_progress(d, t))))?;
        step("llama", 1.0, "Extraindo llama.cpp…".into());
        std::fs::create_dir_all(&llama_dir)?;
        extract_flat(&zip, &llama_dir, &[".exe", ".dll"])?;
        if !llama_dir.join("llama-server.exe").exists() {
            bail!("o pacote do llama.cpp não contém llama-server.exe");
        }
    }
    // llama.cpp para placa de vídeo (Vulkan: AMD, NVIDIA e Intel). Opcional: sem ele a IA usa o processador.
    let gpu_dir = tools.join("llama-gpu");
    if force || !gpu_dir.join("llama-server.exe").exists() {
        step("llama-gpu", 0.0, "Procurando o llama.cpp para placa de vídeo…".into());
        let res = (|| -> Result<()> {
            let asset = find_release_asset(client, LLAMA_REPO, is_llama_vulkan_asset).context("pacote Vulkan do llama.cpp não encontrado")?;
            let zip = tmp.join("llama-gpu.zip");
            download_verified(client, &asset, &zip, cancel, &mut |p, d, t| {
                step("llama-gpu", p, format!("Baixando llama.cpp para placa de vídeo… {}", fmt_progress(d, t)))
            })?;
            std::fs::create_dir_all(&gpu_dir)?;
            extract_flat(&zip, &gpu_dir, &[".exe", ".dll"])?;
            if !gpu_dir.join("llama-server.exe").exists() {
                bail!("o pacote não contém llama-server.exe");
            }
            Ok(())
        })();
        match res {
            Ok(()) => {}
            Err(e) if e.downcast_ref::<Cancelled>().is_some() => return Err(e),
            Err(e) => step("llama-gpu", 1.0, format!("Não foi possível instalar o suporte à placa de vídeo (opcional): {e}")),
        }
    }
    // Token de origem do YouTube (opcional: sem ele o app usa login/cookies).
    let pot_exe = tools.join("bgutil-pot.exe");
    let plugin_dir = crate::tools::ytdlp_plugins_dir().join(crate::tools::POT_PLUGIN);
    if force || !pot_exe.exists() || !plugin_dir.join("yt_dlp_plugins").is_dir() {
        match install_pot(client, &tools, &tmp, cancel, step) {
            Ok(()) => {}
            Err(e) if e.downcast_ref::<Cancelled>().is_some() => return Err(e),
            Err(e) => step("token", 1.0, format!("Não foi possível instalar o token do YouTube (opcional): {e}")),
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(())
}

fn is_pot_binary(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("windows") && (n.contains("x86_64") || n.contains("amd64") || n.contains("x64")) && (n.ends_with(".exe") || n.ends_with(".zip"))
}

fn is_pot_plugin(name: &str) -> bool {
    name == POT_PLUGIN_ZIP
}

fn install_pot(
    client: &reqwest::blocking::Client,
    tools: &Path,
    tmp: &Path,
    cancel: &Cancel,
    step: &mut dyn FnMut(&str, f32, String),
) -> Result<()> {
    step("token", 0.0, "Procurando o gerador de token do YouTube…".into());
    let bin = find_release_asset(client, POT_REPO, is_pot_binary)?;
    let plugin = find_release_asset(client, POT_REPO, is_pot_plugin)?;
    let exe = tools.join("bgutil-pot.exe");
    if bin.url.to_lowercase().ends_with(".zip") {
        let zip = tmp.join("pot-bin.zip");
        download_verified(client, &bin, &zip, cancel, &mut |p, d, t| step("token", p, format!("Baixando token do YouTube… {}", fmt_progress(d, t))))?;
        let found = extract_first_exe(&zip, &exe)?;
        if !found {
            bail!("o pacote não contém o executável");
        }
    } else {
        download_verified(client, &bin, &exe, cancel, &mut |p, d, t| step("token", p, format!("Baixando token do YouTube… {}", fmt_progress(d, t))))?;
    }
    let zip = tmp.join("pot-plugin.zip");
    download_verified(client, &plugin, &zip, cancel, &mut |p, d, t| step("token", p, format!("Baixando plugin do yt-dlp… {}", fmt_progress(d, t))))?;
    install_plugin_zip(&zip, &crate::tools::ytdlp_plugins_dir(), crate::tools::POT_PLUGIN)
}

/// Extrai o plugin preservando as pastas e garante o formato `<pasta>/<nome>/yt_dlp_plugins/…`.
fn install_plugin_zip(zip_path: &Path, plugins_dir: &Path, name: &str) -> Result<()> {
    let staging = plugins_dir.join("_extraindo");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;
    let mut archive = zip::ZipArchive::new(File::open(zip_path)?)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(rel) = entry.enclosed_name() else { continue };
        let out = staging.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::io::copy(&mut entry, &mut File::create(&out)?)?;
        }
    }
    // Acha a pasta que contém "yt_dlp_plugins".
    fn find_root(dir: &Path, depth: usize) -> Option<PathBuf> {
        if dir.join("yt_dlp_plugins").is_dir() {
            return Some(dir.to_path_buf());
        }
        if depth == 0 {
            return None;
        }
        std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).find_map(|p| find_root(&p, depth - 1))
    }
    let root = find_root(&staging, 3).context("o pacote do plugin não tem a pasta yt_dlp_plugins")?;
    let target = plugins_dir.join(name);
    let _ = std::fs::remove_dir_all(&target);
    std::fs::rename(&root, &target)?;
    let _ = std::fs::remove_dir_all(&staging);
    Ok(())
}

fn extract_first_exe(zip_path: &Path, dest: &Path) -> Result<bool> {
    let mut archive = zip::ZipArchive::new(File::open(zip_path)?)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let is_exe = entry.enclosed_name().and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase())).is_some_and(|n| n.ends_with(".exe"));
        if is_exe {
            std::io::copy(&mut entry, &mut File::create(dest)?)?;
            return Ok(true);
        }
    }
    Ok(false)
}

/// Procura, da versão mais nova para a mais antiga (incluindo pré-lançamentos/builds
/// numerados), o primeiro arquivo que satisfaz `want`. Alguns projetos publicam os
/// executáveis só nos builds numerados, e não na versão marcada como "latest".
/// Arquivo publicado numa versão do GitHub, com o SHA-256 que o próprio GitHub calculou.
#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub url: String,
    pub sha256: Option<String>,
}

fn find_release_asset(client: &reqwest::blocking::Client, repo: &str, want: fn(&str) -> bool) -> Result<Asset> {
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=30");
    let mut req = client.get(&url).header("Accept", "application/vnd.github+json");
    // No CI usamos o token para não esbarrar no limite de consultas anônimas.
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        req = req.bearer_auth(token);
    }
    let releases: serde_json::Value = req
        .send()?
        .error_for_status()
        .with_context(|| format!("o GitHub recusou a consulta de versões de {repo} (tente de novo mais tarde)"))?
        .json()?;
    pick_asset(&releases, want).with_context(|| format!("nenhuma versão recente de {repo} tem o pacote para Windows"))
}

pub fn pick_asset(releases: &serde_json::Value, want: fn(&str) -> bool) -> Option<Asset> {
    releases.as_array()?.iter().filter(|r| r["draft"] != true).find_map(|r| {
        r["assets"].as_array()?.iter().find_map(|a| {
            let name = a["name"].as_str()?;
            if !want(name) {
                return None;
            }
            let url = a["browser_download_url"].as_str()?.to_string();
            let sha256 = a["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).map(str::to_lowercase);
            Some(Asset { url, sha256 })
        })
    })
}

/// SHA-256 de um arquivo, em hexadecimal.
pub fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h)?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Baixa e confere o SHA-256 publicado. Arquivo adulterado ou corrompido é apagado e o
/// download falha: nada é executado sem conferência.
fn download_verified(
    client: &reqwest::blocking::Client,
    asset: &Asset,
    dest: &Path,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(f32, u64, u64),
) -> Result<()> {
    let Some(expected) = &asset.sha256 else {
        bail!("{} não tem SHA-256 publicado; por segurança o arquivo não foi baixado", asset.url);
    };
    download_file(client, &asset.url, dest, cancel, on_progress)?;
    let got = sha256_file(dest)?;
    if &got != expected {
        let _ = std::fs::remove_file(dest);
        bail!("o arquivo baixado de {} não confere (SHA-256 diferente do publicado); download descartado", asset.url);
    }
    Ok(())
}

fn is_ffmpeg_asset(name: &str) -> bool {
    name.starts_with("ffmpeg-") && name.ends_with("-essentials_build.zip")
}

fn is_llama_cpu_asset(name: &str) -> bool {
    name.contains("bin-win-cpu-x64") && name.ends_with(".zip")
}

fn is_llama_vulkan_asset(name: &str) -> bool {
    name.contains("bin-win-vulkan-x64") && name.ends_with(".zip")
}

/// Copia modelos (.bin/.gguf) de arquivos ou pastas (ex.: um pendrive) para a pasta de modelos.
pub fn import_models(paths: &[PathBuf], cancel: &Cancel, on_progress: &mut dyn FnMut(InstallProgress)) -> Result<Vec<PathBuf>> {
    let models = models_dir();
    std::fs::create_dir_all(&models)?;
    let mut files = Vec::new();
    for p in paths {
        if p.is_dir() {
            for e in std::fs::read_dir(p)?.filter_map(|e| e.ok()) {
                files.push(e.path());
            }
        } else {
            files.push(p.clone());
        }
    }
    files.retain(|f| {
        f.is_file()
            && f.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("gguf") || e.eq_ignore_ascii_case("bin"))
    });
    if files.is_empty() {
        bail!("nenhum arquivo de modelo (.gguf ou .bin) encontrado");
    }
    let mut copied = Vec::new();
    for f in files {
        let name = f.file_name().unwrap().to_owned();
        let dest = models.join(&name);
        let label = name.to_string_lossy().into_owned();
        if dest.exists() && std::fs::metadata(&dest)?.len() == std::fs::metadata(&f)?.len() {
            copied.push(dest);
            continue;
        }
        copy_with_progress(&f, &dest, cancel, &mut |p, d, t| {
            on_progress(InstallProgress { step: "importar".into(), progress: p, message: format!("Copiando {label}… {}", fmt_progress(d, t)) })
        })?;
        copied.push(dest);
    }
    on_progress(InstallProgress { step: "ok".into(), progress: 1.0, message: format!("{} modelo(s) importado(s)", copied.len()) });
    Ok(copied)
}

/// Extensões dos arquivos de modelo que o app usa.
fn is_model_file(p: &Path) -> bool {
    p.is_file() && p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("gguf") || e.eq_ignore_ascii_case("bin"))
}

/// Modelos que ficaram na pasta padrão depois de escolher outra pasta: (quantidade, MB).
pub fn leftover_models() -> (usize, u64) {
    let (cur, def) = (models_dir(), default_models_dir());
    if cur == def {
        return (0, 0);
    }
    let files: Vec<PathBuf> = std::fs::read_dir(&def)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| is_model_file(p)).collect())
        .unwrap_or_default();
    let mb = files.iter().filter_map(|f| std::fs::metadata(f).ok()).map(|m| m.len()).sum::<u64>() / 1_048_576;
    (files.len(), mb)
}

/// Move os modelos da pasta padrão para a pasta escolhida. Na mesma unidade é instantâneo;
/// entre discos copia com progresso e só apaga o original depois da cópia completa.
pub fn move_models(cancel: &Cancel, on_progress: &mut dyn FnMut(InstallProgress)) -> Result<usize> {
    move_models_between(&default_models_dir(), &models_dir(), cancel, on_progress)
}

fn move_models_between(from: &Path, to: &Path, cancel: &Cancel, on_progress: &mut dyn FnMut(InstallProgress)) -> Result<usize> {
    if from == to {
        bail!("a pasta de modelos já é a padrão");
    }
    std::fs::create_dir_all(to).with_context(|| format!("não foi possível criar {}", to.display()))?;
    let files: Vec<PathBuf> = std::fs::read_dir(from)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| is_model_file(p)).collect())
        .unwrap_or_default();
    let mut moved = 0;
    for f in &files {
        let name = f.file_name().unwrap().to_owned();
        let label = name.to_string_lossy().into_owned();
        let dest = to.join(&name);
        let src_len = std::fs::metadata(f)?.len();
        let same = dest.is_file() && std::fs::metadata(&dest)?.len() == src_len;
        if !same && std::fs::rename(f, &dest).is_ok() {
            moved += 1;
            continue;
        }
        if !same {
            let part = to.join(format!("{label}.movendo"));
            copy_with_progress(f, &part, cancel, &mut |p, d, t| {
                on_progress(InstallProgress { step: "mover".into(), progress: p, message: format!("Movendo {label}… {}", fmt_progress(d, t)) })
            })
            .inspect_err(|_| {
                let _ = std::fs::remove_file(&part);
            })?;
            std::fs::rename(&part, &dest)?;
        }
        std::fs::remove_file(f).with_context(|| format!("copiado, mas não consegui apagar {}", f.display()))?;
        moved += 1;
    }
    on_progress(InstallProgress { step: "ok".into(), progress: 1.0, message: format!("{moved} modelo(s) movido(s) para {}", to.display()) });
    Ok(moved)
}

// ----------------------------------------------------------------- dados de uma instalação anterior

fn tree_size(p: &Path) -> u64 {
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => std::fs::read_dir(p)
            .map(|rd| rd.filter_map(|e| e.ok()).map(|e| tree_size(&e.path())).sum())
            .unwrap_or(0),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

/// Ferramentas, modelos e cache que ficaram na pasta padrão (disco C) depois de instalar o app
/// em outro lugar: (pasta, MB). `None` se não houver nada para mover.
pub fn legacy_data() -> Option<(PathBuf, u64)> {
    let (cur, old) = (app_data_dir(), legacy_data_dir());
    if cur == old {
        return None;
    }
    let bytes: u64 = DATA_ITEMS.iter().filter(|i| **i != "settings.json").map(|i| tree_size(&old.join(i))).sum();
    (bytes > 0).then_some((old, bytes / 1_048_576))
}

/// Na primeira vez numa pasta nova, aproveita as configurações da instalação anterior.
pub fn adopt_legacy_settings() {
    let (cur, old) = (app_data_dir(), legacy_data_dir());
    let (new, prev) = (cur.join("settings.json"), old.join("settings.json"));
    if cur != old && !new.exists() && prev.is_file() {
        let _ = std::fs::create_dir_all(&cur);
        if std::fs::copy(&prev, &new).is_ok() {
            let _ = rewrite_paths(&new, &old, &cur);
        }
    }
}

/// Troca, nas configurações, caminhos que apontavam para a pasta antiga.
fn rewrite_paths(settings_file: &Path, from: &Path, to: &Path) -> Result<()> {
    let text = std::fs::read_to_string(settings_file)?;
    let mut v: serde_json::Value = serde_json::from_str(&text)?;
    let (from, to) = (from.to_string_lossy().into_owned(), to.to_string_lossy().into_owned());
    fn walk(v: &mut serde_json::Value, from: &str, to: &str) {
        match v {
            serde_json::Value::String(s) if s.starts_with(from) => *s = format!("{to}{}", &s[from.len()..]),
            serde_json::Value::Array(a) => a.iter_mut().for_each(|x| walk(x, from, to)),
            serde_json::Value::Object(o) => o.values_mut().for_each(|x| walk(x, from, to)),
            _ => {}
        }
    }
    walk(&mut v, &from, &to);
    std::fs::write(settings_file, serde_json::to_string_pretty(&v)?)?;
    Ok(())
}

/// Move ferramentas, modelos, cache e registros da pasta padrão para a pasta do programa.
pub fn migrate_legacy_data(cancel: &Cancel, on_progress: &mut dyn FnMut(InstallProgress)) -> Result<()> {
    migrate_data_between(&legacy_data_dir(), &app_data_dir(), cancel, on_progress)
}

fn migrate_data_between(from: &Path, to: &Path, cancel: &Cancel, on_progress: &mut dyn FnMut(InstallProgress)) -> Result<()> {
    if from == to {
        bail!("os dados já estão na pasta do programa");
    }
    std::fs::create_dir_all(to).with_context(|| format!("não foi possível criar {}", to.display()))?;
    let items: Vec<&str> = DATA_ITEMS.iter().copied().filter(|i| from.join(i).exists()).collect();
    let total: u64 = items.iter().map(|i| tree_size(&from.join(i))).sum::<u64>().max(1);
    let mut done: u64 = 0;
    for item in items {
        let (src, dest) = (from.join(item), to.join(item));
        if item == "settings.json" && dest.exists() {
            // As configurações já foram adotadas na pasta nova; a antiga só é apagada.
            let _ = std::fs::remove_file(&src);
            continue;
        }
        move_tree(&src, &dest, cancel, &mut done, total, on_progress)?;
    }
    let settings = to.join("settings.json");
    if settings.is_file() {
        rewrite_paths(&settings, from, to)?;
    }
    on_progress(InstallProgress { step: "ok".into(), progress: 1.0, message: format!("Dados movidos para {}", to.display()) });
    Ok(())
}

/// Move arquivo ou pasta: renomeia se der (mesmo disco); senão copia e apaga o original.
fn move_tree(src: &Path, dest: &Path, cancel: &Cancel, done: &mut u64, total: u64, on_progress: &mut dyn FnMut(InstallProgress)) -> Result<()> {
    if !dest.exists() && std::fs::rename(src, dest).is_ok() {
        *done += tree_size(dest);
        return Ok(());
    }
    if src.is_dir() {
        std::fs::create_dir_all(dest)?;
        for e in std::fs::read_dir(src)?.filter_map(|e| e.ok()) {
            move_tree(&e.path(), &dest.join(e.file_name()), cancel, done, total, on_progress)?;
        }
        let _ = std::fs::remove_dir(src);
        return Ok(());
    }
    let len = std::fs::metadata(src)?.len();
    let same = dest.is_file() && std::fs::metadata(dest)?.len() == len;
    if !same {
        let label = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let base = *done;
        copy_with_progress(src, dest, cancel, &mut |_, d, _| {
            let all = base + d;
            on_progress(InstallProgress {
                step: "mover".into(),
                progress: (all as f64 / total as f64).min(1.0) as f32,
                message: format!("Movendo {label}… {}", fmt_progress(all, total)),
            })
        })?;
    }
    *done += len;
    std::fs::remove_file(src).with_context(|| format!("copiado, mas não consegui apagar {}", src.display()))?;
    Ok(())
}

fn fmt_progress(done: u64, total: u64) -> String {
    let mb = |b: u64| b as f64 / 1_048_576.0;
    if total > 0 {
        format!("{:.0} / {:.0} MB", mb(done), mb(total))
    } else {
        format!("{:.0} MB", mb(done))
    }
}

fn copy_stream(
    mut reader: impl Read,
    total: u64,
    dest: &Path,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(f32, u64, u64),
) -> Result<()> {
    let part = PathBuf::from(format!("{}.part", dest.display()));
    let mut file = File::create(&part).with_context(|| format!("não foi possível criar {}", part.display()))?;
    let mut buf = vec![0u8; 512 * 1024];
    let mut done: u64 = 0;
    let mut last_report = 0u64;
    loop {
        if cancel.is_cancelled() {
            drop(file);
            let _ = std::fs::remove_file(&part);
            bail!(Cancelled);
        }
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        done += n as u64;
        if done - last_report > 2 * 1_048_576 {
            last_report = done;
            on_progress(if total > 0 { done as f32 / total as f32 } else { 0.0 }, done, total);
        }
    }
    file.flush()?;
    drop(file);
    if total > 0 && done < total {
        let _ = std::fs::remove_file(&part);
        bail!("transferência incompleta ({done} de {total} bytes)");
    }
    std::fs::rename(&part, dest)?;
    on_progress(1.0, done, total);
    Ok(())
}

fn download_file(
    client: &reqwest::blocking::Client,
    url: &str,
    dest: &Path,
    cancel: &Cancel,
    on_progress: &mut dyn FnMut(f32, u64, u64),
) -> Result<()> {
    let resp = client.get(url).send().with_context(|| format!("falha ao baixar {url}"))?;
    if !resp.status().is_success() {
        bail!("falha ao baixar {url}: {}", resp.status());
    }
    let total = resp.content_length().unwrap_or(0);
    copy_stream(resp, total, dest, cancel, on_progress).with_context(|| format!("falha ao baixar {url}"))
}

fn copy_with_progress(src: &Path, dest: &Path, cancel: &Cancel, on_progress: &mut dyn FnMut(f32, u64, u64)) -> Result<()> {
    let total = std::fs::metadata(src)?.len();
    copy_stream(File::open(src)?, total, dest, cancel, on_progress)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_cpu_asset() {
        let rels = serde_json::json!([{"assets": [
            {"name": "llama-b9000-bin-win-cuda-12.4-x64.zip", "browser_download_url": "u1"},
            {"name": "llama-b9000-bin-win-cpu-arm64.zip", "browser_download_url": "u2"},
            {"name": "llama-b9000-bin-win-cpu-x64.zip", "browser_download_url": "u3", "digest": "sha256:ABC123"}
        ]}]);
        assert_eq!(pick_asset(&rels, is_llama_cpu_asset), Some(Asset { url: "u3".into(), sha256: Some("abc123".into()) }));
        assert_eq!(pick_asset(&rels, is_llama_vulkan_asset), None);
    }

    #[test]
    fn skips_release_without_binaries() {
        // Como no whisper.cpp: a versão "latest" (v1.9.5) só tem código-fonte;
        // os executáveis estão no build numerado (b5454).
        let rels = serde_json::json!([
            {"tag_name": "b5455", "draft": true, "assets": [{"name": "whisper-bin-x64.zip", "browser_download_url": "rascunho"}]},
            {"tag_name": "v1.9.5", "assets": []},
            {"tag_name": "b5454", "prerelease": true, "assets": [
                {"name": "whisper-blas-bin-x64.zip", "browser_download_url": "blas"},
                {"name": "whisper-bin-x64.zip", "browser_download_url": "certo"}
            ]}
        ]);
        assert_eq!(pick_asset(&rels, |n| n == WHISPER_ASSET).map(|a| a.url).as_deref(), Some("certo"));
    }

    #[test]
    fn catalog_is_consistent() {
        for m in WHISPER_MODELS.iter().chain(LLM_MODELS) {
            assert!(m.url.ends_with(m.file), "{}", m.id);
        }
        assert!(WHISPER_MODELS.iter().all(|m| m.url.starts_with(HF_WHISPER)));
        assert_eq!(LLM_MODELS.iter().filter(|m| m.recommended).count(), 1);
    }

    /// Consulta o GitHub de verdade (roda no CI do Windows).
    #[test]
    #[ignore]
    fn live_release_lookup() {
        let c = client().unwrap();
        let w = find_release_asset(&c, WHISPER_REPO, |n| n == WHISPER_ASSET).unwrap();
        assert!(w.url.ends_with("/whisper-bin-x64.zip"), "{w:?}");
        let l = find_release_asset(&c, LLAMA_REPO, is_llama_cpu_asset).unwrap();
        assert!(l.url.contains("bin-win-cpu-x64") && l.url.ends_with(".zip"), "{l:?}");
        let v = find_release_asset(&c, LLAMA_REPO, is_llama_vulkan_asset).unwrap();
        assert!(v.url.contains("bin-win-vulkan-x64") && v.url.ends_with(".zip"), "{v:?}");
        let f = find_release_asset(&c, FFMPEG_REPO, is_ffmpeg_asset).unwrap();
        let y = find_release_asset(&c, YTDLP_REPO, |n| n == "yt-dlp.exe").unwrap();
        let d = find_release_asset(&c, DENO_REPO, |n| n == "deno-x86_64-pc-windows-msvc.zip").unwrap();
        for a in [&w, &l, &v, &f, &y, &d] {
            eprintln!("{} sha256={:?}", a.url, a.sha256);
            assert!(a.sha256.as_ref().is_some_and(|h| h.len() == 64), "sem SHA-256 publicado: {}", a.url);
            let r = c.head(a.url.as_str()).send().unwrap();
            assert!(r.status().is_success(), "{}: {}", a.url, r.status());
        }
        // Confere de verdade um download pequeno (yt-dlp) contra o SHA-256 publicado.
        let tmp0 = std::env::temp_dir().join(format!("bwsc-verify-{}", std::process::id()));
        std::fs::create_dir_all(&tmp0).unwrap();
        download_verified(&c, &y, &tmp0.join("yt-dlp.exe"), &Cancel::new(), &mut |_, _, _| {}).unwrap();
        let bad = Asset { url: y.url.clone(), sha256: Some("0".repeat(64)) };
        assert!(download_verified(&c, &bad, &tmp0.join("x.exe"), &Cancel::new(), &mut |_, _, _| {}).is_err());
        assert!(!tmp0.join("x.exe").exists(), "arquivo que não confere é apagado");
        let _ = std::fs::remove_dir_all(&tmp0);
        // Gerador de token do YouTube: mostra os arquivos publicados se não achar.
        let rels: serde_json::Value = c
            .get(format!("https://api.github.com/repos/{POT_REPO}/releases?per_page=5"))
            .bearer_auth(std::env::var("GITHUB_TOKEN").unwrap_or_default())
            .send()
            .unwrap()
            .json()
            .unwrap();
        let names: Vec<String> = rels.as_array().unwrap().iter().take(1)
            .flat_map(|r| r["assets"].as_array().cloned().unwrap_or_default())
            .filter_map(|a| a["name"].as_str().map(str::to_string)).collect();
        eprintln!("pot assets: {names:?}");
        let b = pick_asset(&rels, is_pot_binary).unwrap_or_else(|| panic!("sem binário Windows: {names:?}"));
        let p = pick_asset(&rels, is_pot_plugin).unwrap_or_else(|| panic!("sem plugin: {names:?}"));
        eprintln!("pot: {b:?}\nplugin: {p:?}");

        // Instala de verdade numa pasta temporária e confere a estrutura do plugin.
        let tmp = std::env::temp_dir().join(format!("bwsc-pot-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let zip = tmp.join("plugin.zip");
        download_verified(&c, &p, &zip, &Cancel::new(), &mut |_, _, _| {}).unwrap();
        install_plugin_zip(&zip, &tmp.join("plugins"), crate::tools::POT_PLUGIN).unwrap();
        assert!(tmp.join("plugins").join(crate::tools::POT_PLUGIN).join("yt_dlp_plugins").is_dir());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn import_copies_models_only() {
        let src = std::env::temp_dir().join(format!("bwsc-import-{}", std::process::id()));
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("notas.txt"), b"x").unwrap();
        let err = import_models(std::slice::from_ref(&src), &Cancel::new(), &mut |_| {});
        assert!(err.is_err());
        std::fs::remove_dir_all(&src).unwrap();
    }
}

#[cfg(test)]
mod move_tests {
    use super::*;

    #[test]
    fn moves_only_model_files_and_removes_originals() {
        let base = std::env::temp_dir().join(format!("bwsc-move-{}", std::process::id()));
        let (from, to) = (base.join("de"), base.join("para"));
        std::fs::create_dir_all(&from).unwrap();
        std::fs::write(from.join("a.gguf"), vec![1u8; 1000]).unwrap();
        std::fs::write(from.join("ggml-base.bin"), vec![2u8; 500]).unwrap();
        std::fs::write(from.join("leia.txt"), b"x").unwrap();
        let n = move_models_between(&from, &to, &Cancel::new(), &mut |_| {}).unwrap();
        assert_eq!(n, 2);
        assert_eq!(std::fs::read(to.join("a.gguf")).unwrap().len(), 1000);
        assert!(to.join("ggml-base.bin").is_file());
        assert!(!from.join("a.gguf").exists());
        assert!(from.join("leia.txt").exists());
        assert!(!to.join("leia.txt").exists());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn migrates_data_folders_and_rewrites_paths() {
        let base = std::env::temp_dir().join(format!("bwsc-migrate-{}", std::process::id()));
        let (from, to) = (base.join("c"), base.join("d"));
        std::fs::create_dir_all(from.join("models")).unwrap();
        std::fs::create_dir_all(from.join("tools").join("llama")).unwrap();
        std::fs::create_dir_all(&to).unwrap();
        std::fs::write(from.join("models").join("m.gguf"), vec![1u8; 3000]).unwrap();
        std::fs::write(from.join("tools").join("llama").join("llama-server.exe"), b"exe").unwrap();
        std::fs::write(from.join("uninstall.exe"), b"old app").unwrap();
        let model = from.join("models").join("m.gguf").to_string_lossy().replace('\\', "\\\\");
        std::fs::write(to.join("settings.json"), format!("{{\"llm_model\": \"{model}\", \"setup_done\": true}}")).unwrap();
        std::fs::write(from.join("settings.json"), b"{}").unwrap();
        migrate_data_between(&from, &to, &Cancel::new(), &mut |_| {}).unwrap();
        assert!(to.join("models").join("m.gguf").is_file());
        assert!(to.join("tools").join("llama").join("llama-server.exe").is_file());
        assert!(!from.join("models").exists());
        assert!(!from.join("settings.json").exists());
        assert!(from.join("uninstall.exe").exists(), "o que não é dado do app fica");
        let st = std::fs::read_to_string(to.join("settings.json")).unwrap();
        assert!(st.contains(&*to.join("models").join("m.gguf").to_string_lossy().replace('\\', "\\\\")), "{st}");
        std::fs::remove_dir_all(&base).unwrap();
    }
}
