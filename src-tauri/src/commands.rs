//! Tauri commands: thin wrappers that resolve app paths and call the stores.

use crate::claude::{self, ClaudeStatus, ClaudeTest, SignInLaunch};
use crate::error::{CommandError, CommandResult};
use crate::projects::{self, ProjectDetail, ProjectInput, ProjectSummary};
use crate::settings::{self, Settings};
use crate::{cars, cars::CarDetail, cars::CarSummary};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub app_version: String,
    pub engine_version: String,
}

fn config_dir(app: &AppHandle) -> CommandResult<PathBuf> {
    app.path()
        .app_config_dir()
        .map_err(|e| CommandError::new("no_config_dir", e.to_string()))
}

/// The bundled car library. In a dev build that runs before resources are
/// copied next to the binary, fall back to the repo's `resources/cars`.
fn cars_dir(app: &AppHandle) -> CommandResult<PathBuf> {
    let bundled = app
        .path()
        .resource_dir()
        .map_err(|e| CommandError::new("no_resource_dir", e.to_string()))?
        .join("cars");
    if cfg!(debug_assertions) && !bundled.join("index.json").is_file() {
        return Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../resources/cars"));
    }
    Ok(bundled)
}

/// Run blocking work off the main thread.
async fn blocking<T, F>(work: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> CommandResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| CommandError::new("task_failed", e.to_string()))?
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> CommandResult<AppInfo> {
    Ok(AppInfo {
        app_version: app.package_info().version.to_string(),
        engine_version: liverai_engine::version().to_string(),
    })
}

#[tauri::command]
pub async fn settings_get(app: AppHandle) -> CommandResult<Settings> {
    let dir = config_dir(&app)?;
    blocking(move || Ok(settings::load(&dir))).await
}

#[tauri::command]
pub async fn settings_set(app: AppHandle, settings: Settings) -> CommandResult<Settings> {
    let dir = config_dir(&app)?;
    blocking(move || settings::save(&dir, settings)).await
}

#[tauri::command]
pub async fn cars_list(app: AppHandle) -> CommandResult<Vec<CarSummary>> {
    let dir = cars_dir(&app)?;
    blocking(move || cars::list(&dir)).await
}

#[tauri::command]
pub async fn car_get(app: AppHandle, id: String) -> CommandResult<CarDetail> {
    let dir = cars_dir(&app)?;
    blocking(move || cars::get(&dir, &id)).await
}

#[tauri::command]
pub async fn projects_list() -> CommandResult<Vec<ProjectSummary>> {
    blocking(|| projects::list(&projects::default_root()?)).await
}

#[tauri::command]
pub async fn project_create(app: AppHandle, input: ProjectInput) -> CommandResult<ProjectDetail> {
    let cars = cars_dir(&app)?;
    blocking(move || {
        if !cars::exists(&cars, &input.car_id)? {
            return Err(CommandError::invalid("Car is not in the library."));
        }
        projects::create(&projects::default_root()?, input)
    })
    .await
}

#[tauri::command]
pub async fn project_get(id: String) -> CommandResult<ProjectDetail> {
    blocking(move || projects::get(&projects::default_root()?, &id)).await
}

#[tauri::command]
pub async fn claude_status() -> CommandResult<ClaudeStatus> {
    blocking(|| Ok(claude::status())).await
}

#[tauri::command]
pub async fn claude_open_signin() -> CommandResult<SignInLaunch> {
    blocking(claude::open_signin).await
}

#[tauri::command]
pub async fn claude_test(app: AppHandle) -> CommandResult<ClaudeTest> {
    let dir = config_dir(&app)?;
    blocking(move || {
        let model = settings::load(&dir).model;
        settings::validate_model(&model)?;
        Ok(claude::test(&model))
    })
    .await
}
