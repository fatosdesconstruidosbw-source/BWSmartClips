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
fn save_settings(state: State<AppState>, settings: Settings) -> CmdResult<()> {
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
    free_space_mb: Option<u64>,
    tools_installer_supported: bool,
    installing: bool,
}

fn list_models(pred: impl Fn(&str) -> bool) -> Vec<ModelFile> {
    let mut v: Vec<ModelFile> = std::fs::read_dir(models_dir())
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| pred(&n.to_lowercase())))
                .map(|p| ModelFile {
                    name: p.file_name().unwrap().to_string_lossy().into_owned(),
                    size_mb: std::fs::metadata(&p).map(|m| m.len() / 1_048_576).unwrap_or(0),
                    path: p.to_string_lossy().into_owned(),
                })
                .collect()
        })
        .unwrap_or_default();
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
    let source = Source::parse(&source);
    let job_id = id.clone();
    std::thread::spawn(move || {
        let mut last_emit = std::time::Instant::now() - std::time::Duration::from_secs(1);
        let mut last_msg = String::new();
        let result = run_job(&source, &options, &settings, &cancel, &mut |p: Progress| {
            // Limita a frequência de eventos para não sobrecarregar a interface.
            let now = std::time::Instant::now();
            if p.message != last_msg || now.duration_since(last_emit).as_millis() > 150 {
                last_emit = now;
                last_msg = p.message.clone();
                let _ = app.emit("job://progress", JobProgress { id: job_id.clone(), progress: p });
            }
        });
        app.state::<AppState>().jobs.lock().unwrap().remove(&job_id);
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

#[tauri::command]
fn cancel_job(state: State<AppState>, id: String) {
    if let Some(c) = state.jobs.lock().unwrap().get(&id) {
        c.cancel();
    }
}

/// Lista os projetos já gerados (lendo `cortes.json` de cada pasta).
#[tauri::command]
fn list_projects(state: State<AppState>) -> Vec<JobResult> {
    let dir = PathBuf::from(&state.settings.lock().unwrap().output_dir);
    let mut out: Vec<JobResult> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| std::fs::read_to_string(e.path().join("cortes.json")).ok())
                .filter_map(|t| serde_json::from_str::<JobResult>(&t).ok())
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.project_dir.cmp(&a.project_dir));
    out.truncate(30);
    out
}

fn main() {
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
            list_projects
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o BWSmartClips");
}
