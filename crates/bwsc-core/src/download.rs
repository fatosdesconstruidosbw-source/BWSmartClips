//! Download de vídeos do YouTube (e outros sites) com yt-dlp.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::config::tools_dir;
use crate::process::{command, run_lines, Cancel};

pub struct Downloaded {
    pub path: PathBuf,
    pub title: String,
}

/// Baixa `url` para `dir`, informando o progresso (0.0–1.0).
pub fn download(
    ytdlp: &Path,
    ffmpeg: &Path,
    url: &str,
    dir: &Path,
    cancel: &Cancel,
    mut on_progress: impl FnMut(f32, &str),
) -> Result<Downloaded> {
    std::fs::create_dir_all(dir)?;
    let template = dir.join("source.%(ext)s");
    let mut cmd = command(ytdlp);
    cmd.args([
        "--no-playlist",
        "--newline",
        "--progress",
        "--no-part",
        "-f",
        "bv*[height<=1080]+ba/b[height<=1080]/bv*+ba/b",
        "--merge-output-format",
        "mp4",
        "--print",
        "after_move:BWSC\t%(filepath)s\t%(title)s",
        "--ffmpeg-location",
    ])
    .arg(ffmpeg)
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
    })
    .context("falha no download do vídeo (verifique o link ou atualize o yt-dlp)")?;

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
    fn progress_line() {
        assert_eq!(parse_progress("[download]  50.0% of   10.00MiB at 2.00MiB/s ETA 00:02"), Some(0.5));
        assert_eq!(parse_progress("[info] something"), None);
    }
}
