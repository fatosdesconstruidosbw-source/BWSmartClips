//! Download de vídeos do YouTube (e outros sites) com yt-dlp.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::config::tools_dir;
use crate::process::{command, run_lines, Cancel};

pub struct Downloaded {
    pub path: PathBuf,
    pub title: String,
}

/// De onde o yt-dlp tira os cookies (o YouTube às vezes exige "estar logado").
#[derive(Debug, Clone, Default)]
pub struct Auth {
    pub cookies_file: Option<PathBuf>,
    pub browser: Option<String>,
}

impl Auth {
    fn args(&self) -> Vec<std::ffi::OsString> {
        if let Some(f) = &self.cookies_file {
            vec!["--cookies".into(), f.clone().into_os_string()]
        } else if let Some(b) = &self.browser {
            vec!["--cookies-from-browser".into(), b.into()]
        } else {
            Vec::new()
        }
    }
}

/// Navegadores tentados automaticamente quando o YouTube pede login.
/// O Firefox vem primeiro: no Windows, Chrome/Edge criptografam os cookies de um
/// jeito que o yt-dlp muitas vezes não consegue ler.
const AUTO_BROWSERS: &[&str] = &["firefox", "edge", "chrome", "brave", "opera"];

pub fn is_bot_check(err: &str) -> bool {
    let e = err.to_lowercase();
    e.contains("not a bot") || e.contains("sign in to confirm") || e.contains("cookies-from-browser")
}

/// Baixa `url` para `dir`, informando o progresso (0.0–1.0). Se o YouTube pedir
/// login e nenhum cookie estiver configurado, tenta os navegadores instalados.
#[allow(clippy::too_many_arguments)]
pub fn download(
    ytdlp: &Path,
    ffmpeg: &Path,
    url: &str,
    dir: &Path,
    auth: &Auth,
    cancel: &Cancel,
    mut on_progress: impl FnMut(f32, &str),
) -> Result<Downloaded> {
    let first = attempt(ytdlp, ffmpeg, url, dir, auth, cancel, &mut on_progress);
    let err = match first {
        Ok(d) => return Ok(d),
        Err(e) if crate::process::is_cancelled_error(&e) => return Err(e),
        Err(e) => e,
    };
    let text = format!("{err:#}");
    if !is_bot_check(&text) {
        return Err(err).context("falha no download do vídeo (verifique o link ou atualize o yt-dlp)");
    }
    if auth.cookies_file.is_none() && auth.browser.is_none() {
        for b in AUTO_BROWSERS {
            on_progress(0.0, &format!("YouTube pediu login — tentando os cookies do {b}…"));
            let a = Auth { cookies_file: None, browser: Some(b.to_string()) };
            match attempt(ytdlp, ffmpeg, url, dir, &a, cancel, &mut on_progress) {
                Ok(d) => return Ok(d),
                Err(e) if crate::process::is_cancelled_error(&e) => return Err(e),
                Err(_) => continue,
            }
        }
    }
    bail!(
        "O YouTube bloqueou o download pedindo para confirmar que você não é um robô.\n\n\
         Como resolver (uma vez só):\n\
         1. No navegador, entre na sua conta do YouTube.\n\
         2. Instale a extensão \"Get cookies.txt LOCALLY\" (Chrome/Edge) ou \"cookies.txt\" (Firefox).\n\
         3. Abrindo youtube.com, use a extensão para exportar o arquivo cookies.txt.\n\
         4. No app: Configurações → Ferramentas e modelos → \"Cookies do YouTube\" → escolha o arquivo.\n\n\
         Se você usa o Firefox logado no YouTube, basta escolher \"Firefox\" em \"Ler cookies do navegador\".\n\n\
         Detalhe técnico: {}",
        text.lines().find(|l| l.contains("ERROR")).unwrap_or(text.lines().last().unwrap_or_default())
    )
}

#[allow(clippy::too_many_arguments)]
fn attempt(
    ytdlp: &Path,
    ffmpeg: &Path,
    url: &str,
    dir: &Path,
    auth: &Auth,
    cancel: &Cancel,
    on_progress: &mut impl FnMut(f32, &str),
) -> Result<Downloaded> {
    std::fs::create_dir_all(dir)?;
    let template = dir.join("source.%(ext)s");
    let mut cmd = command(ytdlp);
    cmd.args([
        "--no-playlist",
        "--newline",
        "--progress",
        "--no-part",
        "--encoding",
        "utf-8",
        "-f",
        "bv*[height<=1080]+ba/b[height<=1080]/bv*+ba/b",
        "--merge-output-format",
        "mp4",
        "--print",
        "after_move:BWSC\t%(filepath)s\t%(title)s",
        "--ffmpeg-location",
    ])
    .arg(ffmpeg)
    .args(auth.args())
    .arg("-o")
    .arg(&template)
    .arg(url)
    .env("PYTHONIOENCODING", "utf-8");
    // Permite que o yt-dlp encontre o Deno (runtime JS exigido pelo YouTube)
    // instalado na pasta de ferramentas.
    let mut paths = vec![tools_dir()];
    if let Some(p) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&p));
    }
    if let Ok(joined) = std::env::join_paths(paths) {
        cmd.env("PATH", joined);
    }

    let mut result: Option<Downloaded> = None;
    run_lines(cmd, cancel, |_, line| {
        if let Some(rest) = line.strip_prefix("BWSC\t") {
            let mut parts = rest.splitn(2, '\t');
            let path = parts.next().unwrap_or_default();
            let title = parts.next().unwrap_or_default();
            result = Some(Downloaded { path: PathBuf::from(path), title: title.to_string() });
        } else if let Some(p) = parse_progress(line) {
            on_progress(p, line);
        }
    })?;

    match result {
        Some(d) if d.path.is_file() => Ok(d),
        _ => {
            // Plano B: procura o arquivo baixado na pasta.
            let found = std::fs::read_dir(dir)?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .find(|p| p.file_stem().is_some_and(|s| s == "source"));
            match found {
                Some(path) => Ok(Downloaded { path, title: "video".into() }),
                None => bail!("o yt-dlp terminou mas o arquivo baixado não foi encontrado"),
            }
        }
    }
}

/// Extrai a porcentagem de linhas como `[download]  42.3% of ...`.
fn parse_progress(line: &str) -> Option<f32> {
    let rest = line.strip_prefix("[download]")?.trim_start();
    let pct = rest.split('%').next()?.trim();
    pct.parse::<f32>().ok().map(|p| (p / 100.0).clamp(0.0, 1.0))
}

pub fn looks_like_url(s: &str) -> bool {
    let s = s.trim();
    s.starts_with("http://") || s.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_bot_check() {
        assert!(is_bot_check("ERROR: [youtube] x: Sign in to confirm you’re not a bot. Use --cookies-from-browser"));
        assert!(!is_bot_check("ERROR: Video unavailable"));
        let a = Auth { cookies_file: Some("c.txt".into()), browser: Some("firefox".into()) };
        assert_eq!(a.args(), vec![std::ffi::OsString::from("--cookies"), "c.txt".into()]);
    }

    #[test]
    fn progress_line() {
        assert_eq!(parse_progress("[download]  50.0% of   10.00MiB at 2.00MiB/s ETA 00:02"), Some(0.5));
        assert_eq!(parse_progress("[info] something"), None);
    }
}
