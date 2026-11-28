//! Licença da edição Microsoft Store.
//!
//! A compra, o pagamento e a licença vitalícia ficam com a Microsoft Store: o app só
//! pergunta ao Windows (`Windows.Services.Store.StoreContext`) se a conta atual tem direito
//! de usar o BWSmartClips. Sem servidores próprios. A Store guarda a licença no PC, então a
//! consulta funciona sem internet; se mesmo assim falhar, o app aceita a última confirmação
//! por até [`OFFLINE_DAYS`] dias.
//!
//! Na edição direta (instalador próprio) não há verificação.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct LicenseStatus {
    /// Esta edição exige licença da Store.
    pub required: bool,
    /// Pode usar todas as funções.
    pub ok: bool,
    /// Licença de avaliação (se você oferecer avaliação grátis na Store).
    pub trial: bool,
    /// "direct", "ok", "trial", "offline", "no-license", "not-packaged" ou "error".
    pub state: &'static str,
    pub message: String,
    /// Link para a página do app na Microsoft Store.
    pub store_url: String,
}

/// Dias que o app funciona sem conseguir confirmar a licença (ex.: serviço da Store fora do ar).
#[allow(dead_code)]
pub const OFFLINE_DAYS: u64 = 30;

/// ID do produto na Store (ex.: 9NXXXXXXXXXX), definido na compilação da edição Store.
pub fn store_url() -> String {
    match option_env!("BWSC_STORE_ID").filter(|s| !s.trim().is_empty()) {
        Some(id) => format!("ms-windows-store://pdp/?ProductId={}", id.trim()),
        None => "ms-windows-store://search/?query=BWSmartClips".to_string(),
    }
}

/// Situação da licença. O resultado fica guardado por alguns minutos; `force` consulta de novo.
pub fn status(force: bool) -> LicenseStatus {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static CACHE: Mutex<Option<(Instant, LicenseStatus)>> = Mutex::new(None);
    if !force {
        if let Some((at, s)) = CACHE.lock().unwrap().as_ref() {
            if at.elapsed() < Duration::from_secs(600) && s.ok {
                return s.clone();
            }
        }
    }
    let s = query();
    *CACHE.lock().unwrap() = Some((Instant::now(), s.clone()));
    s
}

fn make(required: bool, ok: bool, trial: bool, state: &'static str, message: impl Into<String>) -> LicenseStatus {
    LicenseStatus { required, ok, trial, state, message: message.into(), store_url: store_url() }
}

#[cfg(not(feature = "store"))]
fn query() -> LicenseStatus {
    make(false, true, false, "direct", "")
}

#[cfg(all(feature = "store", not(windows)))]
fn query() -> LicenseStatus {
    // Só para desenvolvimento fora do Windows.
    make(true, cfg!(debug_assertions), false, "not-packaged", "A licença da Microsoft Store só existe no Windows.")
}

#[cfg(all(feature = "store", windows))]
fn query() -> LicenseStatus {
    if !store::is_packaged() {
        // O executável foi copiado para fora do pacote instalado pela Store.
        return make(
            true,
            cfg!(debug_assertions),
            false,
            "not-packaged",
            "Esta cópia do BWSmartClips não foi instalada pela Microsoft Store. Instale o app pela Store para usá-lo.",
        );
    }
    match store::app_license() {
        Ok((true, trial)) => {
            store::remember_ok();
            if trial {
                make(true, true, true, "trial", "Versão de avaliação.")
            } else {
                make(true, true, false, "ok", "")
            }
        }
        Ok((false, _)) => make(
            true,
            false,
            false,
            "no-license",
            "Não encontramos uma compra do BWSmartClips nesta conta da Microsoft. Compre ou restaure a licença na Microsoft Store.",
        ),
        Err(e) => match store::days_since_ok() {
            Some(d) if d <= OFFLINE_DAYS => make(true, true, false, "offline", format!("Licença confirmada há {d} dia(s); a Store não respondeu agora.")),
            _ => make(
                true,
                false,
                false,
                "error",
                format!("Não foi possível confirmar a licença com a Microsoft Store ({e}). Verifique se a Store está funcionando e tente de novo."),
            ),
        },
    }
}

#[cfg(all(feature = "store", windows))]
mod store {
    use std::time::{SystemTime, UNIX_EPOCH};

    use windows::ApplicationModel::Package;
    use windows::Services::Store::StoreContext;
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

    /// O processo está rodando de dentro de um pacote MSIX (identidade de pacote).
    pub fn is_packaged() -> bool {
        Package::Current().is_ok()
    }

    /// (licença ativa, avaliação). Roda numa thread própria (apartamento MTA), porque a
    /// janela principal usa outro modelo de threads.
    pub fn app_license() -> Result<(bool, bool), String> {
        std::thread::spawn(|| {
            // SAFETY: inicializa o WinRT nesta thread nova; o erro "já inicializado" é inofensivo.
            unsafe {
                let _ = RoInitialize(RO_INIT_MULTITHREADED);
            }
            let ctx = StoreContext::GetDefault().map_err(|e| e.message())?;
            let lic = ctx.GetAppLicenseAsync().and_then(|op| op.join()).map_err(|e| e.message())?;
            Ok((lic.IsActive().map_err(|e| e.message())?, lic.IsTrial().unwrap_or(false)))
        })
        .join()
        .unwrap_or_else(|_| Err("erro interno ao consultar a Store".into()))
    }

    fn marker() -> std::path::PathBuf {
        bwsc_core::config::app_data_dir().join("license.dat")
    }

    fn now() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    }

    /// Guarda (protegido pelo Windows) quando a licença foi confirmada pela última vez.
    pub fn remember_ok() {
        let _ = bwsc_core::secret::write(&marker(), now().to_string().as_bytes());
    }

    pub fn days_since_ok() -> Option<u64> {
        let data = bwsc_core::secret::read(&marker()).ok()?;
        let at: u64 = String::from_utf8(data).ok()?.trim().parse().ok()?;
        let now = now();
        // Relógio voltando para trás não estende o prazo.
        (now >= at).then(|| (now - at) / 86_400)
    }
}
