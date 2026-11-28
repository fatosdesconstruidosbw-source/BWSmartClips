//! Contas (YouTube, Twitch, Kick): abre uma janela do próprio app com o login da plataforma
//! e, quando o usuário termina, salva os cookies em `<plataforma>-cookies.txt` para o yt-dlp.
//!
//! No Windows, ler cookies do WebView2 trava se for feito em comandos síncronos ou
//! dentro de eventos — por isso a leitura acontece numa thread separada, em intervalos.

use std::time::{Duration, Instant};

use bwsc_core::secret;
use serde::Serialize;
use tauri::webview::Cookie;
use tauri::{AppHandle, Emitter, Manager, Url, WebviewUrl, WebviewWindowBuilder};

/// O Google recusa login em navegadores "embutidos" desconhecidos; nos apresentamos como o Edge.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36 Edg/142.0.0.0";

pub struct Platform {
    pub id: &'static str,
    pub name: &'static str,
    login_url: &'static str,
    /// Endereços cujos cookies vão para o arquivo.
    cookie_urls: &'static [&'static str],
    /// Cookies que só existem quando a conta está logada.
    login_cookies: &'static [&'static str],
}

pub const PLATFORMS: &[Platform] = &[
    Platform {
        id: "youtube",
        name: "YouTube",
        login_url: "https://accounts.google.com/ServiceLogin?service=youtube&uilel=3&passive=true&continue=https%3A%2F%2Fwww.youtube.com%2F",
        cookie_urls: &["https://www.youtube.com/"],
        login_cookies: &["SAPISID", "__Secure-3PAPISID", "LOGIN_INFO", "__Secure-1PSID"],
    },
    Platform {
        id: "twitch",
        name: "Twitch",
        login_url: "https://www.twitch.tv/login",
        cookie_urls: &["https://www.twitch.tv/", "https://gql.twitch.tv/"],
        login_cookies: &["auth-token"],
    },
    Platform {
        id: "kick",
        name: "Kick",
        login_url: "https://kick.com/",
        cookie_urls: &["https://kick.com/"],
        login_cookies: &["session_token"],
    },
];

pub fn find(id: &str) -> Option<&'static Platform> {
    PLATFORMS.iter().find(|p| p.id == id)
}

fn label(p: &Platform) -> String {
    format!("login-{}", p.id)
}

#[derive(Clone, Serialize)]
pub struct LoginEvent {
    pub platform: String,
    pub ok: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct AccountStatus {
    pub id: &'static str,
    pub name: &'static str,
    pub connected: bool,
}

pub fn status() -> Vec<AccountStatus> {
    PLATFORMS
        .iter()
        .map(|p| AccountStatus { id: p.id, name: p.name, connected: secret::has_login(p.id) })
        .collect()
}

pub fn logout(id: &str) -> std::io::Result<()> {
    secret::delete_login(id)
}

/// Abre a janela de login (ou traz para frente) e começa a vigiar os cookies.
pub fn open(app: &AppHandle, p: &'static Platform) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(&label(p)) {
        let _ = w.set_focus();
        return Ok(());
    }
    let url: Url = p.login_url.parse().expect("URL fixa válida");
    WebviewWindowBuilder::new(app, label(p), WebviewUrl::External(url))
        .title(format!("Entrar no {} — BWSmartClips", p.name))
        .inner_size(520.0, 780.0)
        .center()
        .user_agent(USER_AGENT)
        .data_directory(crate::webview_data_dir())
        .build()?;
    let app = app.clone();
    std::thread::spawn(move || watch(app, p));
    Ok(())
}

fn watch(app: AppHandle, p: &'static Platform) {
    let started = Instant::now();
    let emit = |ok: bool, message: String| {
        let _ = app.emit("account://login", LoginEvent { platform: p.id.into(), ok, message });
    };
    loop {
        std::thread::sleep(Duration::from_millis(1500));
        let Some(win) = app.get_webview_window(&label(p)) else {
            if !secret::has_login(p.id) {
                emit(false, format!("Login no {} cancelado.", p.name));
            }
            return;
        };
        let mut cookies: Vec<Cookie<'static>> = Vec::new();
        for u in p.cookie_urls {
            if let Ok(c) = win.cookies_for_url(u.parse().unwrap()) {
                cookies.extend(c);
            }
        }
        cookies.sort_by(|a, b| (a.domain(), a.name()).cmp(&(b.domain(), b.name())));
        cookies.dedup_by(|a, b| a.domain() == b.domain() && a.name() == b.name());
        if cookies.iter().any(|c| p.login_cookies.contains(&c.name())) {
            // Salvo protegido pelo Windows (DPAPI): só esta conta, neste PC, consegue abrir.
            let result = secret::save_login(p.id, &netscape(&cookies));
            let _ = win.close();
            match result {
                Ok(()) => emit(true, format!("Conectado ao {} ✔", p.name)),
                Err(e) => emit(false, format!("Não foi possível salvar o login: {e}")),
            }
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
    let mut out = String::from("# Netscape HTTP Cookie File\n# Gerado pelo BWSmartClips (login feito dentro do app)\n\n");
    for c in cookies {
        let raw = c.domain().unwrap_or("localhost").trim_start_matches('.');
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
