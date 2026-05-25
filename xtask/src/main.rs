//! `cargo xtask` — project-specific build helpers (project helper).
//!
//! ## Subcommands
//!
//! - `copilot-smoke` — Human-run smoke test that spawns the real
//!   `copilot --acp` subprocess once and verifies it can be
//!   launched, speaks to stdin/stdout without crashing immediately,
//!   and that the engine's stdio framing assumptions hold. **NOT
//!   run in CI** — requires `copilot` CLI on PATH and authenticated
//!   `gh auth`.
//!
//! ## Why std-only?
//!
//! Same reason `xtask` exists at all in many Rust projects: keep
//! build-helper deps zero so the helper is always trivially
//! available. `clap` would be overkill for one subcommand; if more
//! commands are added later, swap in `clap` as the second-or-later
//! subcommand justifies it.

use std::env;
use std::process::ExitCode;

mod copilot_smoke;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("copilot-smoke") => copilot_smoke::run(&args[1..]),
        Some("--help") | Some("-h") | Some("help") | None => {
            print_help();
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("xtask: unknown subcommand `{other}`");
            print_help();
            ExitCode::from(64) // EX_USAGE
        }
    }
}

fn print_help() {
    println!(
        r"cargo xtask — project-specific build helpers

USAGE:
    cargo xtask <SUBCOMMAND> [args...]

SUBCOMMANDS:
    copilot-smoke    Spawn `copilot --acp` once and verify it launches
                     cleanly. Human-run only; requires `copilot` CLI
                     on PATH and authenticated `gh auth`.
    help             Show this help."
    );
}
