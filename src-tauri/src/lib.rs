mod container;

use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
use tauri::{AppHandle, Manager};

const MANIFEST: &str = "mtrpack.project.json";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSummary { name: String, path: String, last_opened: u128 }

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectManifest {
    schema_version: u32,
    name: String,
    target: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    content: Vec<ContentEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    trains: Vec<serde_json::Value>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContentEntry { id: String, kind: String, name: String, file: String, updated_at: u128 }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectData { name: String, target: String, description: String, content: Vec<ContentEntry> }

fn read_manifest(folder: &Path) -> Result<ProjectManifest, String> {
    let content = fs::read_to_string(folder.join(MANIFEST)).map_err(|_| "This folder does not contain an MTR Pack Studio project.".to_string())?;
    let manifest: ProjectManifest = serde_json::from_str(&content).map_err(|e| format!("Invalid project manifest: {e}"))?;
    if !matches!(manifest.schema_version, 1 | 2) { return Err("This project uses an unsupported schema version.".into()); }
    Ok(manifest)
}

fn save_manifest(folder: &Path, manifest: &ProjectManifest) -> Result<(), String> {
    let path = folder.join(MANIFEST);
    fs::write(path, serde_json::to_vec_pretty(manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

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
    let manifest = ProjectManifest { schema_version: 2, name: name.into(), target: "mtr4".into(), description: String::new(), content: Vec::new(), trains: Vec::new() };
    save_manifest(&path, &manifest)?;
    fs::create_dir(path.join("assets")).map_err(|e| e.to_string())?;
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    remember(&app, ProjectSummary { name: name.into(), path: path.to_string_lossy().into_owned(), last_opened: now_ms() })
}

#[tauri::command]
fn open_project(app: AppHandle, path: String) -> Result<ProjectSummary, String> {
    let folder = Path::new(&path).canonicalize().map_err(|e| e.to_string())?;
    if !folder.is_dir() { return Err("Choose a project folder.".into()); }
    let manifest = read_manifest(&folder)?;
    remember(&app, ProjectSummary { name: manifest.name, path: folder.to_string_lossy().into_owned(), last_opened: now_ms() })
}

#[tauri::command]
fn get_project(path: String) -> Result<ProjectData, String> {
    let folder = Path::new(&path).canonicalize().map_err(|e| e.to_string())?;
    let manifest = read_manifest(&folder)?;
    Ok(ProjectData { name: manifest.name, target: manifest.target, description: manifest.description, content: manifest.content })
}

#[tauri::command]
fn create_train(path: String, name: String) -> Result<ContentEntry, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 80 { return Err("Enter a train name up to 80 characters.".into()); }
    let folder = Path::new(&path).canonicalize().map_err(|e| e.to_string())?;
    let mut manifest = read_manifest(&folder)?;
    let id = format!("train-{}-{}", now_ms(), manifest.content.len());
    let relative = format!("content/trains/{id}.json");
    let entry = ContentEntry { id, kind: "train".into(), name: name.into(), file: relative.clone(), updated_at: now_ms() };
    let train_dir = folder.join("content").join("trains");
    fs::create_dir_all(&train_dir).map_err(|e| e.to_string())?;
    let train_file = folder.join(&relative);
    fs::write(&train_file, serde_json::to_vec_pretty(&serde_json::json!({"id": entry.id, "name": entry.name, "model": null, "texture": null, "properties": {}})).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    manifest.content.push(entry.clone());
    manifest.schema_version = 2;
    if let Err(error) = save_manifest(&folder, &manifest) { let _ = fs::remove_file(train_file); return Err(error); }
    Ok(entry)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![list_recent_projects, remove_recent_project, create_project, open_project, get_project, create_train])
        .run(tauri::generate_context!())
        .expect("failed to run MTR Pack Studio");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_one_projects_still_open() {
        let folder = std::env::temp_dir().join(format!("mtr-pack-studio-v1-{}", now_ms()));
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join(MANIFEST), r#"{"schemaVersion":1,"name":"Legacy","target":"mtr4","trains":[]}"#).unwrap();
        let project = read_manifest(&folder).unwrap();
        assert_eq!(project.name, "Legacy");
        assert!(project.content.is_empty());
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn creating_train_persists_content_index_and_document() {
        let folder = std::env::temp_dir().join(format!("mtr-pack-studio-train-{}", now_ms()));
        fs::create_dir(&folder).unwrap();
        let manifest = ProjectManifest { schema_version: 2, name: "Test".into(), target: "mtr4".into(), description: String::new(), content: Vec::new(), trains: Vec::new() };
        save_manifest(&folder, &manifest).unwrap();
        let entry = create_train(folder.to_string_lossy().into_owned(), "Example Train".into()).unwrap();
        assert!(folder.join(&entry.file).is_file());
        let reopened = read_manifest(&folder).unwrap();
        assert_eq!(reopened.content.len(), 1);
        assert_eq!(reopened.content[0].name, "Example Train");
        fs::remove_dir_all(folder).unwrap();
    }
}
