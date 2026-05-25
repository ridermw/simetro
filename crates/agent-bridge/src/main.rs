//! `simetro-bridge` binary — stdio framed-NDJSON bridge process.
//!
//! Spawned by `simetro-tauri-app` (or `simetro-headless`). Reads
//! `Envelope<BridgeMessage>` lines from stdin, dispatches each
//! `Request` to the active [`Backend`], and writes the resulting
//! `Reply` (or `LlmError`-mapped warning) back to stdout. Exits
//! cleanly on `Shutdown` or EOF.
//!
//! Backend selection:
//!
//! - `SIMETRO_BRIDGE_BACKEND=mock` (default) → [`MockBackend`]
//! - any other value → logs and exits with code 2 until live provider
//!   wiring is explicitly enabled
//!
//! ## Determinism
//!
//! The bridge is *not* part of the engine's deterministic core, but
//! the wire protocol is: every envelope has a stable `seq` and the
//! reply preserves the original `RequestId`. A recorded transcript
//! (NDJSON file) replays bit-for-bit through the engine.

use simetro_agent_bridge::error_mapping::llm_error_to_message;
use simetro_agent_bridge::wire::{hello_envelope, read_envelope, write_envelope, BridgeMessage};
use simetro_agent_bridge::{Backend, BackendRequest, MockBackend};
use simetro_engine::lifecycle::AgentReply;
use simetro_protocol::{Action, Envelope, SimMessage};
use std::io::{BufReader, BufWriter, Write};
use std::sync::atomic::{AtomicU64, Ordering};

/// Bridge build identifier surfaced in the `Hello` handshake.
const BRIDGE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Exit codes:
///   0 = clean Shutdown / EOF
///   1 = stdio I/O error
///   2 = unknown backend
///   3 = unhandled async runtime error
fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt::init();

    let backend_name = std::env::var("SIMETRO_BRIDGE_BACKEND").unwrap_or_else(|_| "mock".into());
    let backend: Box<dyn Backend> = match backend_name.as_str() {
        "mock" => Box::new(MockBackend::new()),
        other => {
            eprintln!(
                "simetro-bridge: unknown backend `{other}` (only `mock` is wired today; \
                 live providers remain feature-gated/default-off)"
            );
            return std::process::ExitCode::from(2);
        }
    };

    tracing::info!(
        "simetro-bridge ready: backend={}, version={BRIDGE_VERSION}",
        backend.name()
    );

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("simetro-bridge: failed to start tokio runtime: {err}");
            return std::process::ExitCode::from(3);
        }
    };

    match runtime.block_on(run_loop(backend)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("simetro-bridge: stdio loop error: {err}");
            std::process::ExitCode::from(1)
        }
    }
}

async fn run_loop(backend: Box<dyn Backend>) -> std::io::Result<()> {
    let seq = AtomicU64::new(0);
    let next_seq = || seq.fetch_add(1, Ordering::SeqCst);

    // We use sync stdin/stdout (BufReader/BufWriter) under
    // `block_on`-with-current-thread so the loop reads one line, awaits
    // the async Backend, writes the reply, and reads the next line.
    // No need for tokio's async stdio — keeps the framing logic the
    // same as in unit tests.
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());

    // Send a Hello so the parent can verify schema compatibility.
    write_envelope(&mut writer, &hello_envelope(next_seq(), BRIDGE_VERSION))?;

    loop {
        let env = match read_envelope(&mut reader)? {
            Some(env) => env,
            None => {
                tracing::info!("simetro-bridge: EOF on stdin, exiting cleanly");
                return Ok(());
            }
        };

        // Per spec (crates/protocol/src/lib.rs schema_version docs):
        // "Consumers MUST check schema_version == SCHEMA_VERSION
        // before processing payload. Receivers MUST reject on
        // mismatch; never silently process." Validate every envelope
        // before dispatch — not just the Hello handshake — so a peer
        // that bumps versions mid-session is rejected the same way.
        if !env.is_compatible() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "schema_version mismatch: peer={}, ours={}; refusing to process payload",
                    env.schema_version,
                    simetro_protocol::SCHEMA_VERSION
                ),
            ));
        }

        match env.payload {
            BridgeMessage::Hello { .. } => {
                // is_compatible above already validated. Nothing else
                // to negotiate today; future versions may add fields.
            }
            BridgeMessage::Shutdown => {
                tracing::info!("simetro-bridge: Shutdown received, exiting cleanly");
                return Ok(());
            }
            BridgeMessage::Request(req) => {
                let reply = dispatch(&*backend, &req).await;
                write_envelope(
                    &mut writer,
                    &Envelope::new(next_seq(), BridgeMessage::Reply(reply)),
                )?;
            }
            BridgeMessage::Reply(_) | BridgeMessage::Log { .. } => {
                // Parent → child Reply/Log lines are unexpected but
                // not fatal; log and continue.
                tracing::debug!("simetro-bridge: ignoring unexpected envelope from parent");
            }
        }
    }
}

/// Convert one `AgentRequest` into an `AgentReply` by calling the
/// backend, parsing the first tool call (if any), and mapping any
/// `LlmError` into a synthetic NoOp reply carrying the warning
/// rationale.
async fn dispatch(
    backend: &dyn Backend,
    req: &simetro_engine::lifecycle::AgentRequest,
) -> AgentReply {
    let backend_req = BackendRequest {
        agent_id: req.id.agent_id.clone(),
        prompt: req.observation_json.clone(),
        tools: simetro_agent_bridge::tools::action_tool_specs(),
    };
    match backend.invoke(backend_req).await {
        Ok(resp) => match resp.tool_calls.first() {
            None => {
                // No tool call returned — treat as NoOp. This is the
                // expected path when the model decides to do nothing.
                AgentReply {
                    id: req.id.clone(),
                    chosen: Some(Action::NoOp),
                    rationale: truncate(&resp.raw, 512),
                    confidence: 1.0,
                }
            }
            Some(tc) => {
                match simetro_agent_bridge::bridge::parse_tool_call(tc, &req.id.agent_id) {
                    Ok(action) => AgentReply {
                        id: req.id.clone(),
                        chosen: Some(action),
                        rationale: truncate(&resp.raw, 512),
                        confidence: 1.0,
                    },
                    Err(parse_err) => {
                        // Malformed tool calls MUST surface as a
                        // typed Warning::InvalidAction, not a silent
                        // NoOp. Route through the same error mapping
                        // as backend-level errors so observability /
                        // recovery behavior is consistent.
                        let message = llm_error_to_message(&parse_err, &req.id.agent_id, 1);
                        AgentReply {
                            id: req.id.clone(),
                            chosen: Some(Action::NoOp),
                            rationale: warning_rationale(&message),
                            confidence: 1.0,
                        }
                    }
                }
            }
        },
        Err(err) => {
            // Surface a NoOp reply so the lifecycle isn't blocked; the
            // warning rationale tells the operator what happened.
            let message = llm_error_to_message(&err, &req.id.agent_id, 1);
            AgentReply {
                id: req.id.clone(),
                chosen: Some(Action::NoOp),
                rationale: warning_rationale(&message),
                confidence: 1.0,
            }
        }
    }
}

fn warning_rationale(msg: &SimMessage) -> String {
    match msg {
        SimMessage::Warning(w) => format!("bridge warning: {w:?}"),
        SimMessage::Fault(f) => format!("bridge fault: {f:?}"),
        other => format!("bridge: {other:?}"),
    }
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect()
    }
}

// Suppress the `Write` unused-import warning when no test mod imports
// it.
#[allow(dead_code)]
fn _force_use_write(_w: &mut dyn Write) {}
