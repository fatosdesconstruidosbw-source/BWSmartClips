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
    /// Gerador de "token de origem" (bgutil-pot) e pasta de plugins do yt-dlp.
    /// Permite passar pela checagem de robô do YouTube sem login.
    pub pot: Option<(PathBuf, PathBuf)>,
}

impl Auth {
    fn args(&self) -> Vec<std::ffi::OsString> {
        if let Some(f) = &self.cookies_file {
            vec!["--cookies".into(), f.clone().into_os_string()]
        } else if let Some(b) = &self.browser {
            vec!["--cookies-from-browser".into(), b.into()]
        } else if let Some((exe, plugins)) = &self.pot {
            // Token e cookies juntos confundem a validação do YouTube: o token só vai sem cookies.
            vec![
                "--plugin-dirs".into(),
                plugins.clone().into_os_string(),
                "--extractor-args".into(),
                format!("youtubepot-bgutilscript:script_path={}", exe.display()).into(),
            ]
        } else {
            Vec::new()
        }
    }

    fn without_cookies(&self) -> Self {
        Self { cookies_file: None, browser: None, pot: self.pot.clone() }
    }
}

/// A plataforma exigiu login e nada do que foi tentado funcionou.
#[derive(Debug)]
pub struct LoginRequired {
    /// `youtube`, `twitch` ou `kick`.
    pub platform: &'static str,
    pub detail: String,
}

impl std::fmt::Display for LoginRequired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = platform_name(self.platform);
        let why = if self.platform == "youtube" {
            "O YouTube pediu para confirmar que você não é um robô."
        } else {
            "Este vídeo exige estar logado (por exemplo, VOD só para inscritos)."
        };
        write!(
            f,
            "{why}\n\nClique em \"Entrar no {name}\": uma janela do próprio app abre, você faz login uma vez \
             e o BWSmartClips passa a baixar normalmente.\n\nDetalhe técnico: {}",
            self.detail
        )
    }
}
impl std::error::Error for LoginRequired {}

pub fn platform_name(id: &str) -> &'static str {
    match id {
        "youtube" => "YouTube",
        "twitch" => "Twitch",
        "kick" => "Kick",
        _ => "site",
    }
}

/// Identificador da plataforma do link (`youtube`, `twitch`, `kick` ou vazio).
pub fn platform_id(url: &str) -> &'static str {
    let u = url.to_lowercase();
    if u.contains("youtube.com") || u.contains("youtu.be") {
        "youtube"
    } else if u.contains("twitch.tv") {
        "twitch"
    } else if u.contains("kick.com") {
        "kick"
    } else {
        ""
    }
}

/// O link é de uma transmissão ao vivo (não dá para cortar algo que ainda não terminou).
#[derive(Debug)]
pub struct LiveStream;
impl std::fmt::Display for LiveStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "Esse link é de uma transmissão AO VIVO. Use o link de um vídeo salvo: na Twitch, um VOD \
             (twitch.tv/videos/…) ou clipe; na Kick, um vídeo (kick.com/canal/videos/…) ou clipe.",
        )
    }
}
impl std::error::Error for LiveStream {}

/// Nome da plataforma do link, para mensagens.
pub fn platform(url: &str) -> &'static str {
    platform_name(platform_id(url))
}

/// Navegadores tentados automaticamente quando o YouTube pede login.
/// O Firefox vem primeiro: no Windows, Chrome/Edge criptografam os cookies de um
/// jeito que o yt-dlp muitas vezes não consegue ler.
const AUTO_BROWSERS: &[&str] = &["firefox", "edge", "chrome", "brave", "opera"];

pub fn is_bot_check(err: &str) -> bool {
    let e = err.to_lowercase();
    e.contains("not a bot")
        || e.contains("sign in to confirm")
        || e.contains("cookies-from-browser")
        || e.contains("login required")
        || e.contains("requires authentication")
        || e.contains("subscriber")
        || e.contains("sub-only")
        || e.contains("http error 403")
}

/// Baixa `url` para `dir`, informando o progresso (0.0–1.0).
///
/// No YouTube, tenta nesta ordem: sem login (com token de origem, se instalado) →
/// login salvo / cookies.txt → cookies dos navegadores. Se nada funcionar, devolve
/// [`YoutubeLoginRequired`] para a interface oferecer o login dentro do app.
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
    let mut plan: Vec<(Auth, Option<String>)> = vec![(auth.without_cookies(), None)];
    if auth.cookies_file.is_some() {
        plan.push((Auth { browser: None, pot: None, ..auth.clone() }, Some("usando o login salvo".into())));
    }
    if let Some(b) = &auth.browser {
        plan.push((Auth { cookies_file: None, browser: Some(b.clone()), pot: None }, Some(format!("usando os cookies do {b}"))));
    } else if auth.cookies_file.is_none() {
        for b in AUTO_BROWSERS {
            plan.push((Auth { cookies_file: None, browser: Some(b.to_string()), pot: None }, Some(format!("tentando os cookies do {b}"))));
        }
    }

    let mut last = String::new();
    for (i, (a, label)) in plan.iter().enumerate() {
        if let Some(l) = label {
            on_progress(0.0, &format!("{} pediu login — {l}…", platform(url)));
        }
        match attempt(ytdlp, ffmpeg, url, dir, a, cancel, &mut on_progress) {
            Ok(d) => return Ok(d),
            Err(e) if crate::process::is_cancelled_error(&e) || e.downcast_ref::<LiveStream>().is_some() => return Err(e),
            Err(e) => {
                let text = format!("{e:#}");
                // Só vale insistir com outros logins se o problema for a checagem de robô/login.
                if i == 0 && !is_bot_check(&text) {
                    return Err(e).context(format!("falha no download do vídeo do {} (verifique o link)", platform(url)));
                }
                last = text;
            }
        }
    }
    let detail = last
        .lines()
        .find(|l| l.contains("ERROR"))
        .unwrap_or(last.lines().last().unwrap_or_default())
        .to_string();
    match platform_id(url) {
        "" => bail!("o site pediu login para este vídeo.\n\nDetalhe técnico: {detail}"),
        id => bail!(LoginRequired { platform: id, detail }),
    }
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
        // Twitch/Kick usam HLS: baixar vários pedaços em paralelo acelera muito.
        "--concurrent-fragments",
        "4",
        // Não "baixa" uma live (nunca terminaria).
        "--match-filters",
        "!is_live",
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
    let mut live = false;
    run_lines(cmd, cancel, |_, line| {
        if line.contains("does not pass filter") && line.contains("is_live") {
            live = true;
        }
        if let Some(rest) = line.strip_prefix("BWSC\t") {
            let mut parts = rest.splitn(2, '\t');
            let path = parts.next().unwrap_or_default();
            let title = parts.next().unwrap_or_default();
            result = Some(Downloaded { path: PathBuf::from(path), title: title.to_string() });
        } else if let Some(p) = parse_progress(line) {
            on_progress(p, line);
        }
    })?;
    if live {
        bail!(LiveStream);
    }

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
        let a = Auth { cookies_file: Some("c.txt".into()), browser: Some("firefox".into()), pot: Some(("p.exe".into(), "plug".into())) };
        assert_eq!(a.args(), vec![std::ffi::OsString::from("--cookies"), "c.txt".into()]);
        // Sem cookies, usa o token de origem.
        let args = a.without_cookies().args();
        assert_eq!(args[0], "--plugin-dirs");
        assert!(args[3].to_string_lossy().contains("script_path=p.exe"));
        assert_eq!(platform("https://www.twitch.tv/videos/123"), "Twitch");
        assert_eq!(platform("https://kick.com/x/videos/abc"), "Kick");
        assert_eq!(platform("https://youtu.be/x"), "YouTube");
    }

    #[test]
    fn progress_line() {
        assert_eq!(parse_progress("[download]  50.0% of   10.00MiB at 2.00MiB/s ETA 00:02"), Some(0.5));
        assert_eq!(parse_progress("[info] something"), None);
    }
}
