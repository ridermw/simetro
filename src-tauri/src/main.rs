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

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("simetro-desktop: starting");

    tauri::Builder::default()
        .setup(|_app| {
            tracing::info!("tauri setup complete");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running simetro-desktop");
}
