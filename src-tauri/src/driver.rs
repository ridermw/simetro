//! Engine driver — runs the simulation loop in a Tokio task and emits
//! `Envelope<SimMessage>` to the frontend via Tauri events.
//!
//! ```text
//!   ┌──────────────────────────────────────────────────────────────┐
//!   │                        EngineDriver                          │
//!   │                                                              │
//!   │  tokio task (speed-scaled tick) ──▶ app.emit("sim", …)      │
//!   │       ▲                                                      │
//!   │       │ DriverCommand (mpsc)                                 │
//!   │       │                                                      │
//!   │  Tauri commands: pause / step / set_speed / reload           │
//!   └──────────────────────────────────────────────────────────────┘
//! ```

use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use simetro_engine::{
    encode_snapshot, encode_static, load_scene_str, AgentHost, LoadedScene, RunState, SpeedTuner,
    TickRunner, World,
};
use simetro_protocol::{SimEvent, SimMessage, SnapshotPayload};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio::time;

// -------------------------------------------------------------------------
//  Public types
// -------------------------------------------------------------------------

/// Commands sent from Tauri command handlers to the driver task.
#[derive(Debug, Clone)]
pub enum DriverCommand {
    TogglePause,
    Step,
    SetSpeed(f32),
    Reload,
    /// Frontend has connected and is ready to receive messages.
    Subscribe,
}

/// Shared state that Tauri commands use to talk to the driver.
pub struct DriverState {
    pub tx: mpsc::UnboundedSender<DriverCommand>,
}

// -------------------------------------------------------------------------
//  Driver task
// -------------------------------------------------------------------------

const INTERNAL_HZ: u64 = 60;
const SNAPSHOT_HZ: u64 = 20;
const TICKS_PER_SNAPSHOT: u64 = INTERNAL_HZ / SNAPSHOT_HZ; // 3
const MIN_SPEED_FACTOR: f32 = 0.1;
const MAX_SPEED_FACTOR: f32 = 10.0;

/// Spawn the engine driver as a background task. Returns the command
/// sender for Tauri commands to use.
pub fn spawn_driver(app: AppHandle, scene_path: PathBuf, seed: u64) -> DriverState {
    let (tx, rx) = mpsc::unbounded_channel();
    tauri::async_runtime::spawn(driver_loop(app, scene_path, seed, rx));
    DriverState { tx }
}

async fn driver_loop(
    app: AppHandle,
    scene_path: PathBuf,
    seed: u64,
    mut rx: mpsc::UnboundedReceiver<DriverCommand>,
) {
    // Wait for frontend to subscribe before emitting anything.
    loop {
        match rx.recv().await {
            Some(DriverCommand::Subscribe) => break,
            None => return, // channel closed
            _ => {}         // ignore commands before subscribe
        }
    }

    let mut state = match load_and_init(&app, &scene_path, seed) {
        Some(s) => s,
        None => return,
    };

    let mut interval = tick_interval(state.speed_factor);
    let mut ticks_since_snapshot: u64 = 0;

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if state.paused {
                    continue;
                }
                tick_and_emit(&app, &mut state, &mut ticks_since_snapshot);
            }
            cmd = rx.recv() => {
                match cmd {
                    Some(c) => {
                        let is_set_speed = matches!(c, DriverCommand::SetSpeed(_));
                        handle_command(&app, &mut state, c, &scene_path, seed, &mut ticks_since_snapshot);
                        if is_set_speed {
                            interval = tick_interval(state.speed_factor);
                        }
                    }
                    None => break, // channel closed
                }
            }
        }
    }
}

// -------------------------------------------------------------------------
//  Internal state
// -------------------------------------------------------------------------

/// Metadata from LoadedScene that is needed for re-encoding static payloads
/// on late subscribe. We keep this separate from the mutable World.
struct SceneMeta {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    theme: simetro_engine::Theme,
    #[allow(dead_code)]
    id_map: simetro_engine::IdMap,
    #[allow(dead_code)]
    agents: Vec<simetro_engine::AgentSpec>,
}

struct SimState {
    world: World,
    runner: TickRunner,
    #[allow(dead_code)]
    meta: SceneMeta,
    snapshot_buf: SnapshotPayload,
    last_static: simetro_protocol::StaticPayload,
    seq: u64,
    paused: bool,
    speed_factor: f32,
}

fn load_and_init(app: &AppHandle, scene_path: &PathBuf, seed: u64) -> Option<SimState> {
    let json = match std::fs::read_to_string(scene_path) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("failed to read scene {}: {e}", scene_path.display());
            emit_fault(app, 0, &format!("cannot read scene file: {e}"));
            return None;
        }
    };

    let loaded = match load_scene_str(&json, seed) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("load error: {e}");
            let msg = SimMessage::Fault(simetro_protocol::FaultPayload::LoadError {
                message: e.to_string(),
                line: None,
                col: None,
            });
            emit_sim(app, 0, msg);
            return None;
        }
    };

    let mut runner = TickRunner::new();
    runner.reserve_for(loaded.world.movers.len());

    // Register agents from the scene spec.
    for spec in &loaded.agents {
        if spec.kind == "speed_tuner" {
            runner.register_agent(AgentHost::new(Box::new(SpeedTuner::new(
                spec.interval_ticks,
            ))));
        }
    }

    let static_payload = encode_static(&loaded);

    // Destructure LoadedScene: world goes mutable, rest becomes metadata.
    let LoadedScene {
        name,
        theme,
        goals: _,
        agents,
        id_map,
        world,
    } = loaded;

    let meta = SceneMeta {
        name,
        theme,
        id_map,
        agents,
    };

    let mut snapshot_buf = SnapshotPayload::default();
    encode_snapshot(&world, &mut snapshot_buf);

    // Emit initial static + snapshot.
    let mut seq: u64 = 0;
    emit_sim(app, seq, SimMessage::Static(static_payload.clone()));
    seq += 1;
    emit_sim(app, seq, SimMessage::Snapshot(snapshot_buf.clone()));
    seq += 1;

    Some(SimState {
        world,
        runner,
        meta,
        snapshot_buf: SnapshotPayload::default(),
        last_static: static_payload,
        seq,
        paused: false,
        speed_factor: 1.0,
    })
}

fn tick_and_emit(app: &AppHandle, state: &mut SimState, ticks_since_snapshot: &mut u64) {
    // catch_unwind around tick to prevent engine panics from crashing the app.
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        state.runner.tick_once(&mut state.world);
    }));

    if let Err(panic_info) = result {
        let msg = match panic_info.downcast_ref::<&str>() {
            Some(s) => s.to_string(),
            None => "engine panic (unknown payload)".to_string(),
        };
        tracing::error!("engine panic: {msg}");
        state.world.state = RunState::Faulted;
        state.paused = true;
        emit_sim(
            app,
            state.seq,
            SimMessage::Fault(simetro_protocol::FaultPayload::EngineFault { message: msg }),
        );
        state.seq += 1;
        return;
    }

    // Emit semantic events (skip tick-only batches).
    let events = state.runner.events();
    let has_semantic = events.iter().any(|e| !matches!(e, SimEvent::Tick { .. }));
    if has_semantic {
        emit_sim(app, state.seq, SimMessage::Events(events.to_vec()));
        state.seq += 1;
    }

    // Emit non-event messages (AgentReport, Fault, Warning).
    for msg in state.runner.messages() {
        emit_sim(app, state.seq, msg.clone());
        state.seq += 1;
    }

    // Emit snapshot at 20Hz.
    *ticks_since_snapshot += 1;
    if *ticks_since_snapshot >= TICKS_PER_SNAPSHOT {
        *ticks_since_snapshot = 0;
        encode_snapshot(&state.world, &mut state.snapshot_buf);
        emit_sim(
            app,
            state.seq,
            SimMessage::Snapshot(state.snapshot_buf.clone()),
        );
        state.seq += 1;
    }
}

fn handle_command(
    app: &AppHandle,
    state: &mut SimState,
    cmd: DriverCommand,
    scene_path: &PathBuf,
    seed: u64,
    ticks_since_snapshot: &mut u64,
) {
    match cmd {
        DriverCommand::TogglePause => {
            state.paused = !state.paused;
            if state.paused {
                state.world.state = RunState::Paused;
            } else if state.world.state == RunState::Paused {
                state.world.state = RunState::Running;
            }
            tracing::info!("pause toggled: paused={}", state.paused);
        }
        DriverCommand::Step => {
            // Execute exactly one tick regardless of pause state.
            let was_paused = state.paused;
            state.paused = false;
            if state.world.state == RunState::Paused {
                state.world.state = RunState::Running;
            }
            tick_and_emit(app, state, ticks_since_snapshot);
            if was_paused {
                state.paused = true;
                state.world.state = RunState::Paused;
            }
        }
        DriverCommand::SetSpeed(factor) => {
            state.speed_factor = clamp_speed_factor(factor);
            tracing::info!("speed factor set to {}", state.speed_factor);
        }
        DriverCommand::Reload => {
            tracing::info!("reloading scene from {}", scene_path.display());
            // Attempt to reload the scene from disk.
            let json = match std::fs::read_to_string(scene_path) {
                Ok(s) => s,
                Err(e) => {
                    emit_fault(app, state.seq, &format!("cannot read scene: {e}"));
                    state.seq += 1;
                    return;
                }
            };
            let loaded = match load_scene_str(&json, seed) {
                Ok(l) => l,
                Err(e) => {
                    let msg = SimMessage::Fault(simetro_protocol::FaultPayload::LoadError {
                        message: e.to_string(),
                        line: None,
                        col: None,
                    });
                    emit_sim(app, state.seq, msg);
                    state.seq += 1;
                    return;
                }
            };

            // Replace engine state.
            let mut runner = TickRunner::new();
            runner.reserve_for(loaded.world.movers.len());
            for spec in &loaded.agents {
                if spec.kind == "speed_tuner" {
                    runner.register_agent(AgentHost::new(Box::new(SpeedTuner::new(
                        spec.interval_ticks,
                    ))));
                }
            }

            let static_payload = encode_static(&loaded);
            state.world = loaded.world;
            state.runner = runner;
            state.last_static = static_payload.clone();
            state.paused = false;
            *ticks_since_snapshot = 0;

            // Emit fresh static + snapshot.
            emit_sim(app, state.seq, SimMessage::Static(static_payload));
            state.seq += 1;
            encode_snapshot(&state.world, &mut state.snapshot_buf);
            emit_sim(
                app,
                state.seq,
                SimMessage::Snapshot(state.snapshot_buf.clone()),
            );
            state.seq += 1;
        }
        DriverCommand::Subscribe => {
            // Late subscribe — re-emit current state.
            emit_sim(
                app,
                state.seq,
                SimMessage::Static(state.last_static.clone()),
            );
            state.seq += 1;
            encode_snapshot(&state.world, &mut state.snapshot_buf);
            emit_sim(
                app,
                state.seq,
                SimMessage::Snapshot(state.snapshot_buf.clone()),
            );
            state.seq += 1;
        }
    }
}

// -------------------------------------------------------------------------
//  Helpers
// -------------------------------------------------------------------------

fn clamp_speed_factor(factor: f32) -> f32 {
    if factor.is_finite() {
        factor.clamp(MIN_SPEED_FACTOR, MAX_SPEED_FACTOR)
    } else {
        1.0
    }
}

fn tick_period(speed_factor: f32) -> Duration {
    Duration::from_secs_f64(1.0 / (INTERNAL_HZ as f64 * f64::from(speed_factor)))
}

fn tick_interval(speed_factor: f32) -> time::Interval {
    let mut interval = time::interval(tick_period(speed_factor));
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    interval
}

#[derive(Clone, Serialize)]
struct SimEnvelope {
    schema_version: u32,
    seq: u64,
    payload: SimMessage,
}

fn emit_sim(app: &AppHandle, seq: u64, msg: SimMessage) {
    let env = SimEnvelope {
        schema_version: simetro_protocol::SCHEMA_VERSION,
        seq,
        payload: msg,
    };
    if let Err(e) = app.emit("sim", &env) {
        tracing::warn!("failed to emit sim event: {e}");
    }
}

fn emit_fault(app: &AppHandle, seq: u64, message: &str) {
    emit_sim(
        app,
        seq,
        SimMessage::Fault(simetro_protocol::FaultPayload::EngineFault {
            message: message.to_string(),
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_factor_clamps_to_supported_range() {
        assert_eq!(clamp_speed_factor(0.01), MIN_SPEED_FACTOR);
        assert_eq!(clamp_speed_factor(20.0), MAX_SPEED_FACTOR);
        assert_eq!(clamp_speed_factor(2.0), 2.0);
        assert_eq!(clamp_speed_factor(f32::NAN), 1.0);
    }

    #[test]
    fn tick_period_scales_with_speed_factor() {
        assert_eq!(tick_period(1.0), Duration::from_secs_f64(1.0 / 60.0));
        assert_eq!(tick_period(2.0), Duration::from_secs_f64(1.0 / 120.0));
        assert_eq!(tick_period(0.5), Duration::from_secs_f64(1.0 / 30.0));
    }
}
