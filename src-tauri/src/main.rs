// Esconde o console no Windows em modo release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accounts;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use bwsc_core::config::{models_dir, tools_dir};
use bwsc_core::cache::Cache;
use bwsc_core::installer::{self, InstallProgress, InstallRequest};
use bwsc_core::process::is_cancelled_error;
use bwsc_core::tools::Tools;
use bwsc_core::{run_job, Cancel, ClipOptions, JobResult, Progress, Settings, Source};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

struct AppState {
    settings: Mutex<Settings>,
    jobs: Mutex<HashMap<String, Cancel>>,
    install: Mutex<Option<Cancel>>,
    next_id: AtomicU64,
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

// ----------------------------------------------------------------- configurações

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(state: State<AppState>, mut settings: Settings) -> CmdResult<()> {
    settings.remember_output();
    let md = settings.models_dir.trim();
    if !md.is_empty() {
        std::fs::create_dir_all(md).map_err(|e| format!("Não consegui usar a pasta de modelos {md}: {e}"))?;
    }
    settings.apply();
    settings.save(&Settings::default_path()).map_err(err)?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[derive(Serialize)]
struct ModelFile {
    path: String,
    name: String,
    size_mb: u64,
}

#[derive(Serialize)]
struct ToolStatus {
    tools: Tools,
    whisper_models: Vec<ModelFile>,
    llm_models: Vec<ModelFile>,
    whisper_catalog: &'static [installer::ModelInfo],
    llm_catalog: &'static [installer::ModelInfo],
    tools_dir: String,
    models_dir: String,
    default_models_dir: String,
    /// Pasta do programa, onde ficam configurações, ferramentas, modelos e cache.
    data_dir: String,
    /// Dados de uma instalação anterior que ficaram no disco C: (pasta, MB).
    legacy_data: Option<(String, u64)>,
    /// Modelos que ficaram na pasta padrão depois de escolher outra: (quantidade, MB).
    leftover_models: (usize, u64),
    free_space_mb: Option<u64>,
    tools_installer_supported: bool,
    installing: bool,
}

fn list_models(pred: impl Fn(&str) -> bool) -> Vec<ModelFile> {
    let mut v: Vec<ModelFile> = bwsc_core::tools::model_files(pred)
        .into_iter()
        .map(|p| ModelFile {
            name: p.file_name().unwrap().to_string_lossy().into_owned(),
            size_mb: std::fs::metadata(&p).map(|m| m.len() / 1_048_576).unwrap_or(0),
            path: p.to_string_lossy().into_owned(),
        })
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

#[tauri::command]
fn tool_status(state: State<AppState>) -> ToolStatus {
    let settings = state.settings.lock().unwrap().clone();
    ToolStatus {
        tools: Tools::locate(&settings),
        whisper_models: list_models(|n| n.starts_with("ggml-") && n.ends_with(".bin") && !n.contains("silero")),
        llm_models: list_models(|n| n.ends_with(".gguf") && !n.starts_with("mmproj")),
        whisper_catalog: installer::WHISPER_MODELS,
        llm_catalog: installer::LLM_MODELS,
        tools_dir: tools_dir().to_string_lossy().into_owned(),
        models_dir: models_dir().to_string_lossy().into_owned(),
        default_models_dir: bwsc_core::config::default_models_dir().to_string_lossy().into_owned(),
        data_dir: bwsc_core::config::app_data_dir().to_string_lossy().into_owned(),
        legacy_data: installer::legacy_data().map(|(p, mb)| (p.to_string_lossy().into_owned(), mb)),
        leftover_models: installer::leftover_models(),
        free_space_mb: installer::free_space().map(|b| b / 1_048_576),
        tools_installer_supported: cfg!(windows),
        installing: state.install.lock().unwrap().is_some(),
    }
}

#[tauri::command]
fn estimate_install(request: InstallRequest) -> u64 {
    installer::estimate_mb(&request)
}

/// Roda uma tarefa de instalação/importação em segundo plano, emitindo eventos.
fn spawn_install(
    app: AppHandle,
    state: &AppState,
    task: impl FnOnce(&Cancel, &mut dyn FnMut(InstallProgress)) -> anyhow::Result<()> + Send + 'static,
) -> CmdResult<()> {
    let mut slot = state.install.lock().unwrap();
    if slot.is_some() {
        return Err("Já existe uma instalação em andamento.".into());
    }
    let cancel = Cancel::new();
    *slot = Some(cancel.clone());
    drop(slot);
    std::thread::spawn(move || {
        let res = task(&cancel, &mut |p: InstallProgress| {
            let _ = app.emit("install://progress", p);
        });
        app.state::<AppState>().install.lock().unwrap().take();
        match res {
            Ok(()) => {
                let _ = app.emit("install://done", ());
            }
            Err(e) => {
                let _ = app.emit("install://error", format!("{e:#}"));
            }
        }
    });
    Ok(())
}

#[tauri::command]
fn install_tools(app: AppHandle, state: State<AppState>, request: InstallRequest) -> CmdResult<()> {
    spawn_install(app, &state, move |cancel, cb| installer::install(&request, cancel, cb))
}

#[tauri::command]
async fn import_models(app: AppHandle) -> CmdResult<bool> {
    let picked = app
        .dialog()
        .file()
        .set_title("Escolha os arquivos de modelo (.gguf / .bin)")
        .add_filter("Modelos", &["gguf", "bin"])
        .blocking_pick_files();
    let Some(files) = picked else { return Ok(false) };
    let paths: Vec<PathBuf> = files.into_iter().filter_map(|f| f.into_path().ok()).collect();
    let state = app.state::<AppState>();
    spawn_install(app.clone(), &state, move |cancel, cb| installer::import_models(&paths, cancel, cb).map(|_| ()))?;
    Ok(true)
}

#[derive(Serialize)]
struct GpuInfo {
    /// O suporte à placa de vídeo (llama.cpp Vulkan) está instalado.
    installed: bool,
    /// Placa que será usada, ex.: "AMD Radeon RX 570 Series (8192 MiB, 7900 MiB free)".
    device: Option<String>,
    /// Todas as placas encontradas (inclusive integradas, que são ignoradas).
    all: Vec<String>,
}

/// Placa de vídeo que a IA local pode usar. Assíncrono: a consulta leva ~1 s na primeira vez.
#[tauri::command]
async fn gpu_info(app: AppHandle) -> GpuInfo {
    let settings = app.state::<AppState>().settings.lock().unwrap().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let tools = Tools::locate(&settings);
        match tools.llama_gpu {
            Some(exe) => {
                let all = bwsc_core::local_ai::gpu_devices(&exe);
                let device = bwsc_core::local_ai::best_gpu(&all).map(|(_, d)| d);
                GpuInfo { installed: true, device, all }
            }
            None => GpuInfo { installed: false, device: None, all: Vec::new() },
        }
    })
    .await
    .unwrap_or(GpuInfo { installed: false, device: None, all: Vec::new() })
}

/// Move ferramentas, modelos e cache de uma instalação anterior (disco C) para a pasta do programa.
#[tauri::command]
fn migrate_data(app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    let handle = app.clone();
    spawn_install(app, &state, move |cancel, cb| {
        installer::migrate_legacy_data(cancel, cb)?;
        let settings = Settings::load(&Settings::default_path())?;
        *handle.state::<AppState>().settings.lock().unwrap() = settings;
        Ok(())
    })
}

/// Pasta de dados do navegador interno (fica junto do programa, não no disco C).
pub fn webview_data_dir() -> PathBuf {
    bwsc_core::config::app_data_dir().join("webview")
}

/// Move os modelos que ficaram na pasta padrão para a pasta escolhida.
#[tauri::command]
fn move_models(app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    spawn_install(app, &state, |cancel, cb| installer::move_models(cancel, cb).map(|_| ()))
}

// ----------------------------------------------------------------- contas (YouTube, Twitch, Kick)

/// Precisa ser `async`: no Windows, criar webviews em comando síncrono trava.
#[tauri::command]
async fn account_login(app: AppHandle, platform: String) -> CmdResult<()> {
    let p = accounts::find(&platform).ok_or("plataforma desconhecida")?;
    accounts::open(&app, p).map_err(err)
}

#[tauri::command]
fn account_status() -> Vec<accounts::AccountStatus> {
    accounts::status()
}

#[tauri::command]
fn account_logout(platform: String) -> CmdResult<()> {
    accounts::logout(&platform).map_err(err)
}

// ----------------------------------------------------------------- layout Gameplay

#[derive(Serialize)]
struct FacecamPreview {
    image: String,
    width: u32,
    height: u32,
    detected: Option<bwsc_core::facecam::NormRect>,
}

/// Arquivo de vídeo local para a fonte (arquivo, ou link já baixado antes).
fn local_video(source: &str) -> Result<PathBuf, String> {
    match Source::parse(source) {
        Source::File(p) if p.is_file() => Ok(p),
        Source::File(_) => Err("Arquivo não encontrado.".into()),
        Source::Url(u) => Cache::new(true).downloaded(&u).map(|(p, _)| p).ok_or_else(|| {
            "Para marcar a webcam de um link, gere os cortes desse vídeo uma vez (ele fica salvo) \
             ou carregue o arquivo do vídeo."
                .to_string()
        }),
    }
}

/// Um quadro do vídeo e a webcam detectada nele (para conferir/ajustar à mão).
#[tauri::command]
async fn facecam_preview(state: State<'_, AppState>, source: String) -> CmdResult<FacecamPreview> {
    let settings = state.settings.lock().unwrap().clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<FacecamPreview, String> {
        let video = local_video(&source)?;
        let tools = Tools::locate(&settings);
        let ffmpeg = tools.ffmpeg().map_err(err)?.to_path_buf();
        let cancel = Cancel::new();
        let info = bwsc_core::media::probe(tools.ffprobe().map_err(err)?, &video, &cancel).map_err(err)?;
        // Nome novo a cada vez (o WebView guarda imagens em cache); apaga as anteriores.
        let dir = bwsc_core::config::cache_dir().join("previews");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(err)?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let image = dir.join(format!("webcam-{stamp}.jpg"));
        let at = info.duration * 0.3;
        let mut cmd = bwsc_core::process::command(&ffmpeg);
        cmd.args(["-y", "-v", "error", "-ss", &format!("{at:.2}"), "-i"])
            .arg(&video)
            .args(["-frames:v", "1", "-vf", "scale=960:-2", "-q:v", "3"])
            .arg(&image);
        bwsc_core::process::run(cmd, &cancel).map_err(err)?;
        let (start, dur) = (info.duration * 0.1, (info.duration * 0.8).max(1.0));
        let detected = bwsc_core::facecam::detect_in_video(&ffmpeg, &video, info.width, info.height, start, dur, &cancel, &mut |_| {})
            .map_err(err)?
            .map(|c| c.rect);
        Ok(FacecamPreview { image: image.to_string_lossy().into_owned(), width: info.width, height: info.height, detected })
    })
    .await
    .map_err(err)?
}

#[tauri::command]
fn clear_cache() -> CmdResult<()> {
    Cache::new(true).clear().map_err(err)
}

#[tauri::command]
fn cancel_install(state: State<AppState>) {
    if let Some(c) = state.install.lock().unwrap().as_ref() {
        c.cancel();
    }
}

// ----------------------------------------------------------------- diálogos

#[tauri::command]
async fn pick_video(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .set_title("Escolha o vídeo")
        .add_filter("Vídeos", &["mp4", "mov", "mkv", "webm", "avi", "m4v", "flv", "wmv", "ts"])
        .blocking_pick_file()
        .and_then(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .set_title("Escolha a pasta")
        .blocking_pick_folder()
        .and_then(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
async fn pick_file(app: AppHandle, title: String) -> Option<String> {
    app.dialog()
        .file()
        .set_title(title)
        .blocking_pick_file()
        .and_then(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
fn reveal(app: AppHandle, path: String) -> CmdResult<()> {
    let p = PathBuf::from(&path);
    if p.is_dir() {
        app.opener().open_path(path, None::<&str>).map_err(err)
    } else {
        app.opener().reveal_item_in_dir(p).map_err(err)
    }
}

#[tauri::command]
fn open_file(app: AppHandle, path: String) -> CmdResult<()> {
    app.opener().open_path(path, None::<&str>).map_err(err)
}

// ----------------------------------------------------------------- trabalhos

#[derive(Clone, Serialize)]
struct JobProgress {
    id: String,
    #[serde(flatten)]
    progress: Progress,
}

#[derive(Clone, Serialize)]
struct JobDone {
    id: String,
    result: JobResult,
}

#[derive(Clone, Serialize)]
struct JobError {
    id: String,
    message: String,
    cancelled: bool,
    /// "login:<plataforma>" quando a plataforma exige login (a interface mostra o botão "Entrar no …").
    kind: String,
}

#[tauri::command]
fn start_job(app: AppHandle, state: State<AppState>, source: String, options: ClipOptions) -> CmdResult<String> {
    if source.trim().is_empty() {
        return Err("Informe um link ou escolha um arquivo de vídeo.".into());
    }
    let id = format!("job-{}", state.next_id.fetch_add(1, Ordering::SeqCst));
    let cancel = Cancel::new();
    state.jobs.lock().unwrap().insert(id.clone(), cancel.clone());
    let settings = state.settings.lock().unwrap().clone();
    let source_text = source.clone();
    let source = Source::parse(&source);
    let job_id = id.clone();
    std::thread::spawn(move || {
        let mut log = JobLog::new(&source_text, &options);
        let mut last_emit = std::time::Instant::now() - std::time::Duration::from_secs(1);
        let mut last_msg = String::new();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_job(&source, &options, &settings, &cancel, &mut |p: Progress| {
            log.progress(&p);
            // Limita a frequência de eventos para não sobrecarregar a interface.
            let now = std::time::Instant::now();
            if p.message != last_msg || now.duration_since(last_emit).as_millis() > 150 {
                last_emit = now;
                last_msg = p.message.clone();
                let _ = app.emit("job://progress", JobProgress { id: job_id.clone(), progress: p });
            }
        })))
        // Nunca deixar a tela esperando para sempre: erro interno vira mensagem.
        .unwrap_or_else(|p| Err(anyhow::anyhow!("erro interno: {}", bwsc_core::pipeline::panic_message(&p))));
        app.state::<AppState>().jobs.lock().unwrap().remove(&job_id);
        match &result {
            Ok(r) => log.line(&format!("CONCLUÍDO: {} cortes em {}", r.clips.len(), r.project_dir.display())),
            Err(e) => log.line(&format!("ERRO: {e:#}")),
        }
        match result {
            Ok(result) => {
                let _ = app.emit("job://done", JobDone { id: job_id, result });
            }
            Err(e) => {
                let _ = app.emit(
                    "job://error",
                    JobError {
                        id: job_id,
                        message: format!("{e:#}"),
                        cancelled: is_cancelled_error(&e),
                        kind: e
                            .chain()
                            .find_map(|c| c.downcast_ref::<bwsc_core::download::LoginRequired>())
                            .map(|l| format!("login:{}", l.platform))
                            .unwrap_or_default(),
                    },
                );
            }
        }
    });
    Ok(id)
}

/// Registro do último trabalho (`%LOCALAPPDATA%\\BWSmartClips\\logs\\ultimo-trabalho.log`),
/// com a hora de cada etapa — ajuda a descobrir onde algo travou.
struct JobLog {
    file: Option<std::fs::File>,
    started: std::time::Instant,
    last: String,
}

impl JobLog {
    fn path() -> PathBuf {
        bwsc_core::config::app_data_dir().join("logs").join("ultimo-trabalho.log")
    }

    fn new(source: &str, opts: &ClipOptions) -> Self {
        let path = Self::path();
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let mut log = Self { file: std::fs::File::create(&path).ok(), started: std::time::Instant::now(), last: String::new() };
        log.line(&format!("BWSmartClips {} — {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS));
        log.line(&format!("Fonte: {source}"));
        log.line(&format!(
            "Opções: {} cortes, {}–{} s, formato {:?}, webcam {:.0}%{}",
            opts.clip_count,
            opts.min_duration,
            opts.max_duration,
            opts.reframe,
            opts.cam_ratio * 100.0,
            if opts.facecam.is_some() { " (marcada à mão)" } else { "" }
        ));
        log
    }

    fn line(&mut self, text: &str) {
        use std::io::Write;
        if let Some(f) = self.file.as_mut() {
            let t = self.started.elapsed().as_secs_f64();
            let _ = writeln!(f, "[{:>4}:{:05.2}] {text}", (t / 60.0) as u64, t % 60.0);
            let _ = f.flush();
        }
    }

    /// Registra só quando a mensagem muda (sem as porcentagens).
    fn progress(&mut self, p: &Progress) {
        let key: String = p.message.chars().filter(|c| !c.is_ascii_digit() && *c != '%').collect();
        if key != self.last {
            self.last = key;
            self.line(&format!("{:>3.0}% {:?}: {}", p.overall * 100.0, p.stage, p.message));
        }
    }
}

#[tauri::command]
fn log_path() -> String {
    JobLog::path().to_string_lossy().into_owned()
}

#[tauri::command]
fn cancel_job(state: State<AppState>, id: String) {
    if let Some(c) = state.jobs.lock().unwrap().get(&id) {
        c.cancel();
    }
}

/// Lista os projetos já gerados (lendo `cortes.json` de cada pasta).
#[tauri::command]
fn list_projects(state: State<AppState>) -> Vec<JobResult> {
    let dirs = state.settings.lock().unwrap().project_dirs();
    let mut out: Vec<JobResult> = dirs
        .iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flat_map(|rd| rd.filter_map(|e| e.ok()))
        .filter_map(|e| std::fs::read_to_string(e.path().join("cortes.json")).ok())
        .filter_map(|t| serde_json::from_str::<JobResult>(&t).ok())
        .collect();
    // As pastas começam com data e hora: ordena pelo nome, mais novas primeiro.
    let name = |r: &JobResult| r.project_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    out.sort_by_key(|r| std::cmp::Reverse(name(r)));
    out.dedup_by(|a, b| a.project_dir == b.project_dir);
    out.truncate(30);
    out
}

/// Abre a pasta onde os cortes são salvos (criando-a se ainda não existir).
#[tauri::command]
fn open_output_dir(app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    let dir = state.settings.lock().unwrap().output_base();
    std::fs::create_dir_all(&dir).map_err(err)?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(err)
}

/// Pasta que o app usa quando o usuário não escolhe uma.
#[tauri::command]
fn default_output_dir() -> String {
    bwsc_core::config::default_output_dir().to_string_lossy().into_owned()
}

fn main() {
    // Os dados ficam na pasta onde o programa foi instalado (ver config::app_data_dir).
    bwsc_core::config::update_data_pointer();
    installer::adopt_legacy_settings();
    // Logins antigos em texto puro passam a ser guardados protegidos.
    bwsc_core::secret::migrate_plain_logins();
    let settings = Settings::load(&Settings::default_path()).unwrap_or_default();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            settings: Mutex::new(settings),
            jobs: Mutex::new(HashMap::new()),
            install: Mutex::new(None),
            next_id: AtomicU64::new(1),
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            tool_status,
            install_tools,
            estimate_install,
            import_models,
            clear_cache,
            facecam_preview,
            account_login,
            account_status,
            account_logout,
            cancel_install,
            pick_video,
            pick_folder,
            pick_file,
            reveal,
            open_file,
            start_job,
            cancel_job,
            log_path,
            list_projects,
            default_output_dir,
            move_models,
            migrate_data,
            gpu_info,
            open_output_dir
        ])
        .setup(|app| {
            // Janela principal criada aqui para guardar os dados do navegador interno
            // na pasta do programa (o padrão seria o disco C).
            let cfg = app.config().app.windows.iter().find(|w| w.label == "main").cloned().ok_or("janela principal não configurada")?;
            tauri::WebviewWindowBuilder::from_config(app.handle(), &cfg)?
                .data_directory(webview_data_dir())
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o BWSmartClips");
}
