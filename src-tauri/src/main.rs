// simetro desktop shell (Tauri 2).
//
// This crate is NOT a member of the workspace `Cargo.toml` on purpose
// (see ADR-003): Tauri pulls in platform-specific WebKit/WebView2
// dependencies that we don't want CI containers or `cargo check
// --workspace` to require. Build it explicitly from this directory:
//
//     cd src-tauri && cargo build
//
// The frontend dist is built first via `npm run build` in `frontend/`,
// then this shell loads `../frontend/dist/index.html` per
// `tauri.conf.json`.

mod driver;
mod scene_registry;

use std::path::PathBuf;

use driver::{spawn_driver, DriverCommand, DriverState};
use scene_registry::SceneRegistry;
use tauri::Manager;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("simetro-desktop: starting");

    tauri::Builder::default()
        .setup(|app| {
            let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf();
            let registry =
                SceneRegistry::with_resource_root(project_root, app.path().resource_dir().ok());
            let initial_scene = registry.default_scene()?;

            tracing::info!(
                "scene path: {} ({})",
                initial_scene.path.display(),
                initial_scene.scene_id
            );

            let driver_state = spawn_driver(app.handle().clone(), initial_scene, 0);
            app.manage(driver_state);
            app.manage(registry);

            tracing::info!("tauri setup complete — engine driver spawned");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cmd_subscribe,
            cmd_toggle_pause,
            cmd_step,
            cmd_reload,
            cmd_set_speed,
            cmd_set_scene,
            set_scene,
        ])
        .run(tauri::generate_context!())
        .expect("error while running simetro-desktop");
}

// -------------------------------------------------------------------------
//  Tauri commands (frontend → driver)
// -------------------------------------------------------------------------

#[tauri::command]
fn cmd_subscribe(state: tauri::State<'_, DriverState>) {
    let _ = state.tx.send(DriverCommand::Subscribe);
}

#[tauri::command]
fn cmd_toggle_pause(state: tauri::State<'_, DriverState>) {
    let _ = state.tx.send(DriverCommand::TogglePause);
}

#[tauri::command]
fn cmd_step(state: tauri::State<'_, DriverState>) {
    let _ = state.tx.send(DriverCommand::Step);
}

#[tauri::command]
fn cmd_reload(state: tauri::State<'_, DriverState>) {
    let _ = state.tx.send(DriverCommand::Reload);
}

#[tauri::command]
fn cmd_set_speed(state: tauri::State<'_, DriverState>, factor: f32) {
    let _ = state.tx.send(DriverCommand::SetSpeed(factor));
}

#[tauri::command(rename_all = "snake_case")]
async fn cmd_set_scene(
    state: tauri::State<'_, DriverState>,
    registry: tauri::State<'_, SceneRegistry>,
    scene_id: String,
) -> Result<(), String> {
    enqueue_set_scene(state.inner(), registry.inner(), scene_id).await
}

#[tauri::command(rename_all = "snake_case")]
async fn set_scene(
    state: tauri::State<'_, DriverState>,
    registry: tauri::State<'_, SceneRegistry>,
    scene_id: String,
) -> Result<(), String> {
    enqueue_set_scene(state.inner(), registry.inner(), scene_id).await
}

async fn enqueue_set_scene(
    state: &DriverState,
    registry: &SceneRegistry,
    scene_id: String,
) -> Result<(), String> {
    let scene = registry.resolve(&scene_id).map_err(|e| e.to_string())?;
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    state
        .tx
        .send(DriverCommand::SetScene {
            scene,
            reply: reply_tx,
        })
        .map_err(|_| "engine driver is not running".to_string())?;
    reply_rx
        .await
        .map_err(|_| "engine driver dropped scene switch result".to_string())?
}
