//! `simetro-bridge` binary.
//!
//! Stub harness — prints the wired-in backends and exits. The full
//! stdio/WS protocol loop ships with the LLM agent end-to-end work in
//! Phase 2 (PLAN §23). Until then this binary exists so the crate
//! builds and a future `cargo run -p simetro-agent-bridge` won't be
//! a missing-binary error.

use simetro_agent_bridge::{CopilotBackend, MockBackend};

fn main() {
    tracing_subscriber::fmt::init();
    let mock = MockBackend::new();
    let copilot = CopilotBackend::new();
    tracing::info!(
        "simetro-bridge ready: backends=[{}, {}] (phase-1 stub)",
        simetro_agent_bridge::Backend::name(&mock),
        simetro_agent_bridge::Backend::name(&copilot),
    );
}
