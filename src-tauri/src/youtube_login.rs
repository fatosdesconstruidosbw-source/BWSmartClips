//! "Entrar no YouTube": abre uma janela do próprio app com o login do Google/YouTube,
//! e quando o usuário termina, salva os cookies em `youtube-cookies.txt` para o yt-dlp.
//!
//! No Windows, ler cookies do WebView2 trava se for feito em comandos síncronos ou
//! dentro de eventos — por isso a leitura acontece numa thread separada, em intervalos.

use std::time::{Duration, Instant};

use bwsc_core::config::youtube_cookies_path;
use serde::Serialize;
use tauri::webview::Cookie;
use tauri::{AppHandle, Emitter, Manager, Url, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "yt-login";
const LOGIN_URL: &str =
    "https://accounts.google.com/ServiceLogin?service=youtube&uilel=3&passive=true&continue=https%3A%2F%2Fwww.youtube.com%2F";
/// O Google recusa login em navegadores "embutidos" desconhecidos; nos apresentamos como o Edge.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36 Edg/142.0.0.0";
/// Cookies que só existem quando a conta está logada.
const LOGIN_COOKIES: &[&str] = &["SAPISID", "__Secure-3PAPISID", "LOGIN_INFO", "__Secure-1PSID"];

#[derive(Clone, Serialize)]
pub struct LoginEvent {
    pub ok: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct LoginStatus {
    pub connected: bool,
    pub path: String,
}

pub fn status() -> LoginStatus {
    let p = youtube_cookies_path();
    LoginStatus { connected: p.is_file(), path: p.to_string_lossy().into_owned() }
}

pub fn logout() -> std::io::Result<()> {
    let p = youtube_cookies_path();
    if p.exists() {
        std::fs::remove_file(p)?;
    }
    Ok(())
}

/// Abre a janela de login (ou traz para frente) e começa a vigiar os cookies.
pub fn open(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.set_focus();
        return Ok(());
    }
    let url: Url = LOGIN_URL.parse().expect("URL fixa válida");
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External(url))
        .title("Entrar no YouTube — BWSmartClips")
        .inner_size(520.0, 760.0)
        .center()
        .user_agent(USER_AGENT)
        .build()?;

    let app = app.clone();
    std::thread::spawn(move || watch(app));
    Ok(())
}

fn watch(app: AppHandle) {
    let started = Instant::now();
    let yt: Url = "https://www.youtube.com/".parse().unwrap();
    loop {
        std::thread::sleep(Duration::from_millis(1500));
        let Some(win) = app.get_webview_window(LABEL) else {
            // Janela fechada sem concluir.
            if !youtube_cookies_path().is_file() {
                let _ = app.emit("youtube://login", LoginEvent { ok: false, message: "Login cancelado.".into() });
            }
            return;
        };
        let cookies = match win.cookies_for_url(yt.clone()) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if cookies.iter().any(|c| LOGIN_COOKIES.contains(&c.name())) {
            let result = std::fs::write(youtube_cookies_path(), netscape(&cookies));
            let _ = win.close();
            let ev = match result {
                Ok(()) => LoginEvent { ok: true, message: "Conectado ao YouTube ✔".into() },
                Err(e) => LoginEvent { ok: false, message: format!("Não foi possível salvar o login: {e}") },
            };
            let _ = app.emit("youtube://login", ev);
            return;
        }
        if started.elapsed() > Duration::from_secs(30 * 60) {
            let _ = win.close();
            return;
        }
    }
}

/// Converte para o formato "Netscape cookies.txt" que o yt-dlp lê.
pub fn netscape(cookies: &[Cookie<'static>]) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mut out = String::from("# Netscape HTTP Cookie File\n# Gerado pelo BWSmartClips (login do YouTube)\n\n");
    for c in cookies {
        let raw = c.domain().unwrap_or("youtube.com").trim_start_matches('.');
        // Cookies de "www." são do host exato; os demais valem para subdomínios.
        let (domain, sub) = if raw.starts_with("www.") { (raw.to_string(), "FALSE") } else { (format!(".{raw}"), "TRUE") };
        let path = c.path().unwrap_or("/");
        let secure = if c.secure().unwrap_or(false) { "TRUE" } else { "FALSE" };
        // Cookies de sessão ganham validade de 1 ano (o yt-dlp descarta os "expirados").
        let expires = c.expires_datetime().map(|t| t.unix_timestamp()).unwrap_or(now + 365 * 24 * 3600);
        out.push_str(&format!("{domain}\t{sub}\t{path}\t{secure}\t{expires}\t{}\t{}\n", c.name(), c.value()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn netscape_format() {
        let mut a = Cookie::new("SAPISID", "abc");
        a.set_domain(".youtube.com");
        a.set_path("/");
        a.set_secure(true);
        let mut b = Cookie::new("VISITOR", "x");
        b.set_domain("www.youtube.com");
        let txt = netscape(&[a, b]);
        let lines: Vec<&str> = txt.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).collect();
        let f: Vec<&str> = lines[0].split('\t').collect();
        assert_eq!((f[0], f[1], f[2], f[3], f[5], f[6]), (".youtube.com", "TRUE", "/", "TRUE", "SAPISID", "abc"));
        assert!(f[4].parse::<i64>().unwrap() > 0);
        assert!(lines[1].starts_with("www.youtube.com\tFALSE"));
    }
}
