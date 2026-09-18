use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
use tauri::{AppHandle, Manager};

const MANIFEST: &str = "mtrpack.project.json";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSummary { name: String, path: String, last_opened: u128 }

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectManifest { schema_version: u32, name: String, target: String, trains: Vec<serde_json::Value> }

fn now_ms() -> u128 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() }
fn recent_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("recent-projects.json"))
}
fn read_recent(app: &AppHandle) -> Result<Vec<ProjectSummary>, String> {
    let path = recent_path(app)?;
    if !path.exists() { return Ok(Vec::new()); }
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}
fn save_recent(app: &AppHandle, recent: &[ProjectSummary]) -> Result<(), String> {
    let path = recent_path(app)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(recent).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::rename(temporary, path).map_err(|e| e.to_string())
}
fn remember(app: &AppHandle, project: ProjectSummary) -> Result<ProjectSummary, String> {
    let mut recent = read_recent(app)?;
    recent.retain(|item| item.path != project.path);
    recent.insert(0, project.clone());
    recent.truncate(30);
    save_recent(app, &recent)?;
    Ok(project)
}

#[tauri::command]
fn list_recent_projects(app: AppHandle) -> Result<Vec<ProjectSummary>, String> { read_recent(&app) }

#[tauri::command]
fn remove_recent_project(app: AppHandle, path: String) -> Result<(), String> {
    let mut recent = read_recent(&app)?;
    recent.retain(|item| item.path != path);
    save_recent(&app, &recent)
}

#[tauri::command]
fn create_project(app: AppHandle, name: String, parent: Option<String>) -> Result<ProjectSummary, String> {
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." || name.chars().any(|c| "<>:\"/\\|?*".contains(c) || c.is_control()) || name.ends_with(['.', ' ']) {
        return Err("Enter a valid project name without path separators or reserved characters.".into());
    }
    let parent = match parent.filter(|p| !p.trim().is_empty()) {
        Some(path) => PathBuf::from(path),
        None => app.path().document_dir().map_err(|e| e.to_string())?.join("MTR Pack Studio"),
    };
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    let path = parent.join(name);
    if path.exists() { return Err("A folder with this project name already exists.".into()); }
    fs::create_dir(&path).map_err(|e| e.to_string())?;
    let manifest = ProjectManifest { schema_version: 1, name: name.into(), target: "mtr4".into(), trains: Vec::new() };
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    fs::write(path.join(MANIFEST), bytes).map_err(|e| e.to_string())?;
    fs::create_dir(path.join("assets")).map_err(|e| e.to_string())?;
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    remember(&app, ProjectSummary { name: name.into(), path: path.to_string_lossy().into_owned(), last_opened: now_ms() })
}

#[tauri::command]
fn open_project(app: AppHandle, path: String) -> Result<ProjectSummary, String> {
    let folder = Path::new(&path).canonicalize().map_err(|e| e.to_string())?;
    if !folder.is_dir() { return Err("Choose a project folder.".into()); }
    let manifest_path = folder.join(MANIFEST);
    let content = fs::read_to_string(manifest_path).map_err(|_| "This folder does not contain an MTR Pack Studio project.".to_string())?;
    let manifest: ProjectManifest = serde_json::from_str(&content).map_err(|e| format!("Invalid project manifest: {e}"))?;
    if manifest.schema_version != 1 { return Err("This project uses an unsupported schema version.".into()); }
    remember(&app, ProjectSummary { name: manifest.name, path: folder.to_string_lossy().into_owned(), last_opened: now_ms() })
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![list_recent_projects, remove_recent_project, create_project, open_project])
        .run(tauri::generate_context!())
        .expect("failed to run MTR Pack Studio");
}
