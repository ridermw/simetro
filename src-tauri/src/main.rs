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

use std::path::PathBuf;

use driver::{spawn_driver, DriverCommand, DriverState};
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
            // Resolve the scene path relative to the project root.
            // In dev, CWD is src-tauri; in production this would use a
            // bundled resource. For now, resolve relative to src-tauri's parent.
            let scene_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join("games/demo-paths.json");

            tracing::info!("scene path: {}", scene_path.display());

            let driver_state = spawn_driver(app.handle().clone(), scene_path, 0);
            app.manage(driver_state);

            tracing::info!("tauri setup complete — engine driver spawned");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cmd_subscribe,
            cmd_toggle_pause,
            cmd_step,
            cmd_reload,
            cmd_set_speed,
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
