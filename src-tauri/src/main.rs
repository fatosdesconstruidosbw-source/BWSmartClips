// Esconde o console no Windows em modo release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use bwsc_core::config::{models_dir, tools_dir};
use bwsc_core::installer::{self, InstallProgress};
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
struct ToolStatus {
    tools: Tools,
    installed_models: Vec<String>,
    available_models: Vec<(String, String)>,
    tools_dir: String,
    models_dir: String,
    installer_supported: bool,
    installing: bool,
}

#[tauri::command]
fn tool_status(state: State<AppState>) -> ToolStatus {
    let settings = state.settings.lock().unwrap().clone();
    let mut installed_models: Vec<String> = std::fs::read_dir(models_dir())
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "bin"))
                .map(|p| p.to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    installed_models.sort();
    ToolStatus {
        tools: Tools::locate(&settings),
        installed_models,
        available_models: installer::WHISPER_MODELS.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
        tools_dir: tools_dir().to_string_lossy().into_owned(),
        models_dir: models_dir().to_string_lossy().into_owned(),
        installer_supported: cfg!(windows),
        installing: state.install.lock().unwrap().is_some(),
    }
}

#[tauri::command]
fn install_tools(app: AppHandle, state: State<AppState>, model: String, force: bool) -> CmdResult<()> {
    let mut slot = state.install.lock().unwrap();
    if slot.is_some() {
        return Err("A instalação já está em andamento.".into());
    }
    let cancel = Cancel::new();
    *slot = Some(cancel.clone());
    drop(slot);
    std::thread::spawn(move || {
        let res = installer::install_tools(&model, force, &cancel, &mut |p: InstallProgress| {
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
                    JobError { id: job_id, message: format!("{e:#}"), cancelled: is_cancelled_error(&e) },
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
