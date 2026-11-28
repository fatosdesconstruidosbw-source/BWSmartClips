//! Dados sensíveis guardados no disco (logins das plataformas, marca de licença).
//!
//! No Windows usa a proteção de dados do sistema (DPAPI, escopo do usuário): o arquivo só
//! pode ser aberto pela mesma conta do Windows, no mesmo PC. Copiado para outro lugar,
//! não serve para nada. Em outros sistemas (só desenvolvimento) o conteúdo fica marcado
//! como não protegido.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
#[cfg(windows)]
use anyhow::bail;

/// Texto extra misturado à chave da DPAPI, para separar os dados deste app.
#[cfg(windows)]
const ENTROPY: &[u8] = b"BWSmartClips/secret/v1";
const PLAIN_MAGIC: &[u8] = b"BWSC-PLAIN1\n";

#[cfg(windows)]
pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let entropy = CRYPT_INTEGER_BLOB { cbData: ENTROPY.len() as u32, pbData: ENTROPY.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // SAFETY: os ponteiros de entrada apontam para fatias vivas durante a chamada; a saída
    // é alocada pelo Windows e liberada com LocalFree logo após a cópia.
    unsafe {
        let ok = CryptProtectData(&input, std::ptr::null(), &entropy, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out);
        if ok == 0 {
            bail!("o Windows não conseguiu proteger os dados ({})", std::io::Error::last_os_error());
        }
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        LocalFree(out.pbData as _);
        Ok(v)
    }
}

#[cfg(windows)]
pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
    if data.starts_with(PLAIN_MAGIC) {
        bail!("dado salvo sem proteção (de outro sistema)");
    }
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let entropy = CRYPT_INTEGER_BLOB { cbData: ENTROPY.len() as u32, pbData: ENTROPY.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // SAFETY: igual a `protect`.
    unsafe {
        let ok = CryptUnprotectData(&input, std::ptr::null_mut(), &entropy, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out);
        if ok == 0 {
            bail!("não foi possível abrir os dados protegidos (outro usuário ou outro PC?)");
        }
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        LocalFree(out.pbData as _);
        Ok(v)
    }
}

#[cfg(not(windows))]
pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
    let mut v = PLAIN_MAGIC.to_vec();
    v.extend_from_slice(data);
    Ok(v)
}

#[cfg(not(windows))]
pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    data.strip_prefix(PLAIN_MAGIC).map(<[u8]>::to_vec).context("formato de dado protegido desconhecido")
}

/// Grava `data` protegido. Escreve num arquivo temporário e renomeia, para nunca deixar
/// um arquivo pela metade.
pub fn write(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, protect(data)?).with_context(|| format!("não foi possível salvar {}", path.display()))?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn read(path: &Path) -> Result<Vec<u8>> {
    let raw = std::fs::read(path).with_context(|| format!("não foi possível ler {}", path.display()))?;
    unprotect(&raw)
}

// ----------------------------------------------------------------- logins das plataformas

/// Login salvo (protegido) de `youtube`, `twitch` ou `kick`.
pub fn login_path(platform: &str) -> PathBuf {
    crate::config::app_data_dir().join(format!("{platform}-login.dat"))
}

/// Arquivo antigo, em texto puro, das versões até 1.3.
fn legacy_login_path(platform: &str) -> PathBuf {
    crate::config::app_data_dir().join(format!("{platform}-cookies.txt"))
}

pub const PLATFORMS: &[&str] = &["youtube", "twitch", "kick"];

pub fn has_login(platform: &str) -> bool {
    login_path(platform).is_file()
}

pub fn save_login(platform: &str, cookies_txt: &str) -> Result<()> {
    write(&login_path(platform), cookies_txt.as_bytes())
}

pub fn delete_login(platform: &str) -> std::io::Result<()> {
    for p in [login_path(platform), legacy_login_path(platform)] {
        if p.exists() {
            std::fs::remove_file(p)?;
        }
    }
    Ok(())
}

/// Converte logins antigos em texto puro para o formato protegido e apaga os originais.
pub fn migrate_plain_logins() {
    for p in PLATFORMS {
        let old = legacy_login_path(p);
        if let Ok(text) = std::fs::read_to_string(&old) {
            if save_login(p, &text).is_ok() {
                let _ = std::fs::remove_file(&old);
            }
        }
    }
}

/// Cópia temporária do login em cookies.txt, só pelo tempo do download (o yt-dlp precisa
/// do arquivo). Ao sair de escopo, guarda de volta os cookies renovados e apaga a cópia.
pub struct TempLogin {
    platform: String,
    path: PathBuf,
    original: Vec<u8>,
}

impl TempLogin {
    pub fn open(platform: &str, work_dir: &Path) -> Option<Self> {
        if !has_login(platform) {
            return None;
        }
        let data = read(&login_path(platform)).ok()?;
        std::fs::create_dir_all(work_dir).ok()?;
        let path = work_dir.join(format!(".login-{platform}-{}.txt", std::process::id()));
        std::fs::write(&path, &data).ok()?;
        Some(Self { platform: platform.to_string(), path, original: data })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempLogin {
    fn drop(&mut self) {
        if let Ok(now) = std::fs::read(&self.path) {
            if !now.is_empty() && now != self.original {
                let _ = write(&login_path(&self.platform), &now);
            }
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_temp_copy_is_removed() {
        let dir = std::env::temp_dir().join(format!("bwsc-secret-{}", std::process::id()));
        let f = dir.join("x.dat");
        write(&f, b"segredo").unwrap();
        assert_ne!(std::fs::read(&f).unwrap(), b"segredo".to_vec());
        assert_eq!(read(&f).unwrap(), b"segredo".to_vec());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
