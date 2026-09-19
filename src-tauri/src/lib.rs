mod container;
mod domain;
mod model;

use container::{has_project_magic, is_project_path, Container, ContentEntry};
use domain::{slugify, AssetDefinition, AssetDependency, ModelLayer, TrainDefinition};
use model::ImportAnalysis;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSummary {
    name: String,
    path: String,
    last_opened: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectData {
    name: String,
    namespace: String,
    description: String,
    content: Vec<ContentEntry>,
    path: String,
    recovered: bool,
}

struct ProjectSession {
    container: Container,
}

#[derive(Default)]
struct AppState {
    active: Mutex<Option<ProjectSession>>,
    pending_paths: Mutex<VecDeque<PathBuf>>,
}

fn project_path_from_arguments<I, S>(arguments: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    arguments
        .into_iter()
        .map(|value| PathBuf::from(value.as_ref()))
        .find_map(|path| {
            if path.is_file() && is_project_path(&path) && has_project_magic(&path) {
                path.canonicalize().ok()
            } else {
                None
            }
        })
}

fn queue_project_path(app: &AppHandle, path: PathBuf) {
    if let Ok(mut pending) = app.state::<AppState>().pending_paths.lock() {
        if !pending.iter().any(|queued| paths_equal(queued, &path)) {
            pending.push_back(path.clone());
        }
    }
    let _ = app.emit("open-project-file", path.to_string_lossy().into_owned());
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}
fn lock_error() -> String {
    "The project session is unavailable.".into()
}

fn project_data(container: &Container) -> ProjectData {
    ProjectData {
        name: container.index.name.clone(),
        namespace: container.index.namespace.clone(),
        description: container.index.description.clone(),
        content: container.index.content.clone(),
        path: container.path().to_string_lossy().into_owned(),
        recovered: container.recovered,
    }
}

fn project_summary(container: &Container) -> ProjectSummary {
    ProjectSummary {
        name: container.index.name.clone(),
        path: container.path().to_string_lossy().into_owned(),
        last_opened: now_ms(),
    }
}

fn validate_project_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 80 || name.chars().any(char::is_control) {
        Err("Enter a project name up to 80 characters.".into())
    } else {
        Ok(name)
    }
}

fn validate_resource_id(value: &str) -> Result<&str, String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 80 || !value.chars().all(|character| character.is_ascii_lowercase() || character.is_ascii_digit() || matches!(character, '_' | '-' | '.')) {
        Err("Use lowercase letters, numbers, dots, underscores, or hyphens for the export ID.".into())
    } else { Ok(value) }
}

fn train_entry_index(container: &Container, id: &str) -> Result<usize, String> {
    container.index.content.iter().position(|item| item.kind == "train" && item.id == id)
        .ok_or_else(|| "The selected train no longer exists.".to_string())
}

fn read_train_document(container: &mut Container, index: usize) -> Result<TrainDefinition, String> {
    let entry = container.index.content.get(index).cloned().ok_or_else(|| "The selected train no longer exists.".to_string())?;
    let hash = entry.resources.first().ok_or_else(|| "The train document is missing.".to_string())?;
    let bytes = container.read_blob(hash)?;
    if let Ok(train) = serde_json::from_slice::<TrainDefinition>(&bytes) { return Ok(train); }
    let legacy: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| format!("Invalid train document: {e}"))?;
    let name = legacy.get("name").and_then(|value| value.as_str()).unwrap_or(&entry.name);
    let mut train = TrainDefinition::new(name, &slugify(name, "train"));
    train.id = entry.id;
    Ok(train)
}

fn write_train_document(container: &mut Container, index: usize, train: &TrainDefinition) -> Result<ContentEntry, String> {
    let bytes = serde_json::to_vec(train).map_err(|e| e.to_string())?;
    let hash = container.put_blob(&bytes, "application/vnd.mtrpack.train+json")?;
    let entry = container.index.content.get_mut(index).ok_or_else(|| "The selected train no longer exists.".to_string())?;
    entry.name = train.name.clone(); entry.updated_at = now_ms() as u128; entry.resources = vec![hash];
    Ok(entry.clone())
}

fn read_asset(container: &mut Container, id: &str) -> Result<AssetDefinition, String> {
    let hash = container.index.assets.get(id).cloned().ok_or_else(|| "The selected model asset no longer exists.".to_string())?;
    serde_json::from_slice(&container.read_blob(&hash)?).map_err(|e| format!("Invalid model asset: {e}"))
}

fn media_type(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()).unwrap_or_default().to_ascii_lowercase().as_str() {
        "png" => "image/png", "jpg" | "jpeg" => "image/jpeg", "webp" => "image/webp", "obj" => "model/obj",
        "mtl" => "model/mtl", "fbx" => "model/vnd.fbx", "mqo" => "model/vnd.mqo", _ => "application/octet-stream",
    }
}

fn recent_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("recent-projects.json"))
}

fn save_recent(app: &AppHandle, recent: &[ProjectSummary]) -> Result<(), String> {
    fs::write(
        recent_path(app)?,
        serde_json::to_vec_pretty(recent).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn read_recent(app: &AppHandle) -> Result<Vec<ProjectSummary>, String> {
    let path = recent_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut recent: Vec<ProjectSummary> = serde_json::from_str(&content).unwrap_or_default();
    recent.retain(|item| {
        let project = Path::new(&item.path);
        project.is_file() && is_project_path(project) && has_project_magic(project)
    });
    recent.truncate(30);
    save_recent(app, &recent)?;
    Ok(recent)
}

fn remember(app: &AppHandle, project: ProjectSummary) -> Result<ProjectSummary, String> {
    let mut recent = read_recent(app)?;
    recent.retain(|item| !paths_equal(Path::new(&item.path), Path::new(&project.path)));
    recent.insert(0, project.clone());
    recent.truncate(30);
    save_recent(app, &recent)?;
    Ok(project)
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
        let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn replace_active(state: &AppState, next: Container) -> Result<ProjectData, String> {
    let mut active = state.active.lock().map_err(|_| lock_error())?;
    if let Some(current) = active.as_mut() {
        current.container.flush()?;
    }
    let data = project_data(&next);
    *active = Some(ProjectSession { container: next });
    Ok(data)
}

#[tauri::command]
fn list_recent_projects(app: AppHandle) -> Result<Vec<ProjectSummary>, String> {
    read_recent(&app)
}

#[tauri::command]
fn remove_recent_project(app: AppHandle, path: String) -> Result<(), String> {
    let mut recent = read_recent(&app)?;
    recent.retain(|item| !paths_equal(Path::new(&item.path), Path::new(&path)));
    save_recent(&app, &recent)
}

#[tauri::command]
fn create_project(
    app: AppHandle,
    state: State<AppState>,
    path: String,
    name: String,
) -> Result<ProjectData, String> {
    let name = validate_project_name(&name)?;
    let container = Container::create(Path::new(&path), name)?;
    let summary = project_summary(&container);
    let data = replace_active(&state, container)?;
    remember(&app, summary)?;
    Ok(data)
}

#[tauri::command]
fn open_project(
    app: AppHandle,
    state: State<AppState>,
    path: String,
) -> Result<ProjectData, String> {
    let requested = Path::new(&path);
    {
        let mut active = state.active.lock().map_err(|_| lock_error())?;
        if let Some(current) = active.as_mut() {
            if paths_equal(current.container.path(), requested) {
                current.container.flush()?;
                let data = project_data(&current.container);
                remember(&app, project_summary(&current.container))?;
                return Ok(data);
            }
        }
    }
    let container = Container::open(requested)?;
    let summary = project_summary(&container);
    let data = replace_active(&state, container)?;
    remember(&app, summary)?;
    Ok(data)
}

#[tauri::command]
fn get_active_project(state: State<AppState>) -> Result<Option<ProjectData>, String> {
    let active = state.active.lock().map_err(|_| lock_error())?;
    Ok(active
        .as_ref()
        .map(|session| project_data(&session.container)))
}

#[tauri::command]
fn create_train(state: State<AppState>, name: String, export_id: String) -> Result<ContentEntry, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 80 {
        return Err("Enter a train name up to 80 characters.".into());
    }
    let mut active = state.active.lock().map_err(|_| lock_error())?;
    let session = active
        .as_mut()
        .ok_or_else(|| "Open a project before creating content.".to_string())?;
    let export_id = validate_resource_id(&export_id)?;
    let previous_index = session.container.index.clone();
    if previous_index.content.iter().any(|entry| entry.kind == "train" && entry.name.eq_ignore_ascii_case(name)) { return Err("A train with this name already exists.".into()); }
    let train = TrainDefinition::new(name, export_id);
    let id = train.id.clone();
    let config = serde_json::to_vec(&train).map_err(|e| e.to_string())?;
    let resource = session.container.put_blob(&config, "application/json")?;
    let entry = ContentEntry {
        id: id.clone(),
        kind: "train".into(),
        name: name.into(),
        file: format!("content/trains/{id}.json"),
        updated_at: now_ms() as u128,
        resources: vec![resource],
    };
    session.container.index.content.push(entry.clone());
    if let Err(error) = session.container.commit() {
        session.container.index = previous_index;
        return Err(error);
    }
    if session.container.should_compact().unwrap_or(false) {
        let _ = session.container.compact();
    }
    Ok(entry)
}

#[tauri::command]
fn get_train(state: State<AppState>, train_id: String) -> Result<TrainDefinition, String> {
    let mut active = state.active.lock().map_err(|_| lock_error())?;
    let session = active.as_mut().ok_or_else(|| "Open a project before editing a train.".to_string())?;
    let index = train_entry_index(&session.container, &train_id)?;
    read_train_document(&mut session.container, index)
}

#[tauri::command]
fn update_train(state: State<AppState>, mut train: TrainDefinition, expected_revision: u64) -> Result<TrainDefinition, String> {
    validate_project_name(&train.name)?; validate_resource_id(&train.export_id)?;
    let mut active = state.active.lock().map_err(|_| lock_error())?;
    let session = active.as_mut().ok_or_else(|| "Open a project before editing a train.".to_string())?;
    let index = train_entry_index(&session.container, &train.id)?;
    let current = read_train_document(&mut session.container, index)?;
    if current.revision != expected_revision { return Err("This train changed since it was opened. Reload it before saving again.".into()); }
    if session.container.index.content.iter().enumerate().any(|(position, entry)| position != index && entry.kind == "train" && entry.name.eq_ignore_ascii_case(&train.name)) { return Err("A train with this name already exists.".into()); }
    train.revision = expected_revision.checked_add(1).ok_or_else(|| "Train revision overflow.".to_string())?;
    let previous = session.container.index.clone();
    write_train_document(&mut session.container, index, &train)?;
    if let Err(error) = session.container.commit() { session.container.index = previous; return Err(error); }
    Ok(train)
}

#[tauri::command]
fn delete_train(state: State<AppState>, train_id: String) -> Result<(), String> {
    let mut active = state.active.lock().map_err(|_| lock_error())?;
    let session = active.as_mut().ok_or_else(|| "Open a project before deleting a train.".to_string())?;
    let index = train_entry_index(&session.container, &train_id)?;
    let previous = session.container.index.clone(); session.container.index.content.remove(index);
    if let Err(error) = session.container.commit() { session.container.index = previous; return Err(error); }
    Ok(())
}

#[tauri::command]
fn analyze_model_import(path: String) -> Result<ImportAnalysis, String> {
    let path = Path::new(&path); if !path.is_file() { return Err("Choose an existing model file.".into()); }
    model::analyze(path)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelImportResult { train: TrainDefinition, asset: AssetDefinition }

#[tauri::command]
fn import_model(state: State<AppState>, train_id: String, carriage_id: String, slot: String, path: String, dependency_overrides: std::collections::BTreeMap<String, String>) -> Result<ModelImportResult, String> {
    let path = Path::new(&path); if !path.is_file() { return Err("Choose an existing model file.".into()); }
    let document = model::parse(path, &dependency_overrides)?; let preview = model::to_glb(&document)?;
    let mut active = state.active.lock().map_err(|_| lock_error())?;
    let session = active.as_mut().ok_or_else(|| "Open a project before importing a model.".to_string())?;
    let entry_index = train_entry_index(&session.container, &train_id)?; let mut train = read_train_document(&mut session.container, entry_index)?;
    let carriage = train.carriages.iter_mut().find(|item| item.id == carriage_id).ok_or_else(|| "The selected carriage no longer exists.".to_string())?;
    let previous = session.container.index.clone();
    let source = fs::read(path).map_err(|e| e.to_string())?; let source_hash = session.container.put_blob(&source, media_type(path))?;
    let mut dependencies = Vec::new();
    for referenced in model::referenced_files(path)? {
        let name = referenced.file_name().and_then(|value| value.to_str()).unwrap_or("dependency").to_string();
        let resolved = if referenced.exists() { referenced } else { dependency_overrides.get(&name).map(PathBuf::from).ok_or_else(|| format!("Locate the missing model dependency: {name}"))? };
        let bytes = fs::read(&resolved).map_err(|e| format!("Unable to read {name}: {e}"))?; let hash = session.container.put_blob(&bytes, media_type(&resolved))?;
        dependencies.push(AssetDependency { name, hash, media_type: media_type(&resolved).into() });
    }
    let document_hash = session.container.put_blob(&rmp_serde::to_vec_named(&document).map_err(|e| e.to_string())?, "application/vnd.mtrpack.model+msgpack")?;
    let preview_hash = session.container.put_blob(&preview, "model/gltf-binary")?; let asset_id = Uuid::new_v4().to_string();
    let asset = AssetDefinition { id: asset_id.clone(), name: path.file_stem().and_then(|value| value.to_str()).unwrap_or("Model").into(), source_format: model::model_format(path)?, source_hash, document_hash, preview_hash, dependencies, parts: model::summaries(&document), warnings: document.warnings.clone() };
    let asset_hash = session.container.put_blob(&serde_json::to_vec(&asset).map_err(|e| e.to_string())?, "application/vnd.mtrpack.asset+json")?; session.container.index.assets.insert(asset_id.clone(), asset_hash);
    let layer = ModelLayer { id: Uuid::new_v4().to_string(), name: asset.name.clone(), asset_id, flip_texture_v: false, visible: true, material_bindings: Vec::new(), part_rules: Default::default() };
    match slot.as_str() { "body" => carriage.body_models.push(layer), "bogie1" => carriage.bogie_1_models.push(layer), "bogie2" => carriage.bogie_2_models.push(layer), _ => { session.container.index = previous; return Err("Choose a valid carriage model slot.".into()); } }
    train.revision += 1; write_train_document(&mut session.container, entry_index, &train)?;
    if let Err(error) = session.container.commit() { session.container.index = previous; return Err(error); }
    Ok(ModelImportResult { train, asset })
}

#[tauri::command]
fn get_model_asset(state: State<AppState>, asset_id: String) -> Result<AssetDefinition, String> {
    let mut active = state.active.lock().map_err(|_| lock_error())?; let session = active.as_mut().ok_or_else(|| "No project is open.".to_string())?;
    read_asset(&mut session.container, &asset_id)
}

#[tauri::command]
fn get_model_preview(state: State<AppState>, asset_id: String) -> Result<tauri::ipc::Response, String> {
    let mut active = state.active.lock().map_err(|_| lock_error())?; let session = active.as_mut().ok_or_else(|| "No project is open.".to_string())?;
    let asset = read_asset(&mut session.container, &asset_id)?; Ok(tauri::ipc::Response::new(session.container.read_blob(&asset.preview_hash)?))
}

#[tauri::command]
fn save_project(state: State<AppState>) -> Result<(), String> {
    let mut active = state.active.lock().map_err(|_| lock_error())?;
    active
        .as_mut()
        .ok_or_else(|| "No project is open.".to_string())?
        .container
        .flush()
}

#[tauri::command]
fn close_project(state: State<AppState>) -> Result<(), String> {
    let mut active = state.active.lock().map_err(|_| lock_error())?;
    if let Some(session) = active.as_mut() {
        session.container.flush()?;
    }
    *active = None;
    Ok(())
}

#[tauri::command]
fn take_pending_project_path(state: State<AppState>) -> Result<Option<String>, String> {
    Ok(state
        .pending_paths
        .lock()
        .map_err(|_| lock_error())?
        .pop_front()
        .map(|path| path.to_string_lossy().into_owned()))
}

pub fn run() {
    let mut pending_paths = VecDeque::new();
    if let Some(path) = project_path_from_arguments(std::env::args_os()) {
        pending_paths.push_back(path);
    }
    let state = AppState {
        active: Mutex::new(None),
        pending_paths: Mutex::new(pending_paths),
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(path) = project_path_from_arguments(argv) {
                queue_project_path(app, path);
            }
        }))
        .manage(state)
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_recent_projects,
            remove_recent_project,
            create_project,
            open_project,
            get_active_project,
            create_train,
            get_train,
            update_train,
            delete_train,
            analyze_model_import,
            import_model,
            get_model_asset,
            get_model_preview,
            save_project,
            close_project,
            take_pending_project_path
        ])
        .run(tauri::generate_context!())
        .expect("failed to run MTR Pack Studio");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_name_is_independent_from_file_name() {
        let path = std::env::temp_dir().join(format!("renamed-{}.mtrpack", now_ms()));
        let container = Container::create(&path, "Internal Name").unwrap();
        assert_eq!(container.index.name, "Internal Name");
        drop(container);
        let renamed = path.with_file_name(format!("different-{}.mtrpack", now_ms()));
        fs::rename(&path, &renamed).unwrap();
        assert_eq!(
            Container::open(&renamed).unwrap().index.name,
            "Internal Name"
        );
        fs::remove_file(renamed).unwrap();
    }

    #[test]
    fn startup_arguments_only_accept_valid_project_files() {
        let path = std::env::temp_dir().join(format!("argument-{}.mtrpack", now_ms()));
        drop(Container::create(&path, "Argument").unwrap());
        let selected = project_path_from_arguments([
            std::ffi::OsString::from("app.exe"),
            path.clone().into_os_string(),
        ])
        .unwrap();
        assert_eq!(selected, path.canonicalize().unwrap());
        let selected_without_executable =
            project_path_from_arguments([path.clone().into_os_string()]).unwrap();
        assert_eq!(selected_without_executable, path.canonicalize().unwrap());
        assert!(project_path_from_arguments([
            std::ffi::OsString::from("app.exe"),
            std::env::temp_dir().into_os_string()
        ])
        .is_none());
        fs::remove_file(path).unwrap();
    }
}
