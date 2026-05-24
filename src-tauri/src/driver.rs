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

use std::any::Any;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use simetro_engine::{
    encode_snapshot, encode_static, load_error_to_fault, load_scene_str, AgentHost, LoadError,
    LoadedScene, RunState, SpeedTuner, TickRunner, World,
};
use simetro_protocol::{Envelope, SimEvent, SimMessage, SnapshotPayload};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};
use tokio::time;

use crate::scene_registry::SceneRef;

// -------------------------------------------------------------------------
//  Public types
// -------------------------------------------------------------------------

/// Commands sent from Tauri command handlers to the driver task.
#[derive(Debug)]
pub enum DriverCommand {
    TogglePause,
    Step,
    SetSpeed(f32),
    SetScene {
        scene: SceneRef,
        reply: oneshot::Sender<Result<(), String>>,
    },
    /// Re-read the scene from disk. Used by both the UI reload button and
    /// the live scene file watcher.
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
const SCENE_WATCH_POLL_PERIOD: Duration = Duration::from_millis(250);
const SCENE_WATCH_DEBOUNCE: Duration = Duration::from_millis(300);

/// Spawn the engine driver as a background task. Returns the command
/// sender for Tauri commands to use.
pub fn spawn_driver(app: AppHandle, initial_scene: SceneRef, seed: u64) -> DriverState {
    let (tx, rx) = mpsc::unbounded_channel();
    let active_scene = Arc::new(RwLock::new(initial_scene.clone()));
    tauri::async_runtime::spawn(driver_loop(
        app,
        initial_scene,
        active_scene.clone(),
        seed,
        rx,
    ));
    spawn_scene_file_watcher(active_scene, tx.clone());
    DriverState { tx }
}

fn spawn_scene_file_watcher(
    active_scene: Arc<RwLock<SceneRef>>,
    tx: mpsc::UnboundedSender<DriverCommand>,
) {
    tauri::async_runtime::spawn(scene_file_watcher_loop(active_scene, tx));
}

async fn scene_file_watcher_loop(
    active_scene: Arc<RwLock<SceneRef>>,
    tx: mpsc::UnboundedSender<DriverCommand>,
) {
    let mut watched_scene = read_active_scene(&active_scene);
    let mut watcher = DebouncedFileWatch::new(
        observe_scene_file(&watched_scene.path),
        SCENE_WATCH_DEBOUNCE,
    );
    let mut interval = time::interval(SCENE_WATCH_POLL_PERIOD);
    interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;
        let current_scene = read_active_scene(&active_scene);
        if current_scene != watched_scene {
            tracing::info!(
                "scene watcher retargeted to {} ({})",
                current_scene.path.display(),
                current_scene.scene_id
            );
            watcher = DebouncedFileWatch::new(
                observe_scene_file(&current_scene.path),
                SCENE_WATCH_DEBOUNCE,
            );
            watched_scene = current_scene;
            continue;
        }

        let now = Instant::now();
        if watcher.observe(observe_scene_file(&watched_scene.path), now) {
            tracing::info!(
                "scene file changed; reloading {} ({})",
                watched_scene.path.display(),
                watched_scene.scene_id
            );
            if tx.send(DriverCommand::Reload).is_err() {
                break;
            }
        }
    }
}

async fn driver_loop(
    app: AppHandle,
    initial_scene: SceneRef,
    active_scene: Arc<RwLock<SceneRef>>,
    seed: u64,
    mut rx: mpsc::UnboundedReceiver<DriverCommand>,
) {
    // Wait for frontend to subscribe before emitting anything.
    let mut pending_commands = Vec::new();
    loop {
        match rx.recv().await {
            Some(DriverCommand::Subscribe) => break,
            None => return, // channel closed
            Some(cmd) => pending_commands.push(cmd),
        }
    }

    let mut state = match load_and_init(&app, &initial_scene, seed) {
        Some(s) => s,
        None => return,
    };

    let mut ticks_since_snapshot: u64 = 0;
    for cmd in pending_commands {
        handle_command(
            &app,
            &mut state,
            cmd,
            &active_scene,
            seed,
            &mut ticks_since_snapshot,
        );
    }

    let mut interval = tick_interval(state.speed_factor);

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
                        let is_set_speed = matches!(&c, DriverCommand::SetSpeed(_));
                        handle_command(&app, &mut state, c, &active_scene, seed, &mut ticks_since_snapshot);
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

struct SceneLoad {
    world: World,
    runner: TickRunner,
    meta: SceneMeta,
    static_payload: simetro_protocol::StaticPayload,
}

#[derive(Debug)]
enum SceneLoadFailure {
    Read(std::io::Error),
    Load(LoadError),
}

impl std::fmt::Display for SceneLoadFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(e) => write!(f, "cannot read scene: {e}"),
            Self::Load(e) => write!(f, "{e}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SceneFileObservation {
    Present(SceneFileStamp),
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SceneFileStamp {
    len: u64,
    content_hash: u64,
}

#[derive(Debug)]
struct PendingFileChange {
    observation: SceneFileObservation,
    deadline: Instant,
}

#[derive(Debug)]
struct DebouncedFileWatch {
    stable_observation: SceneFileObservation,
    pending: Option<PendingFileChange>,
    debounce: Duration,
}

impl DebouncedFileWatch {
    fn new(initial: SceneFileObservation, debounce: Duration) -> Self {
        Self {
            stable_observation: initial,
            pending: None,
            debounce,
        }
    }

    fn observe(&mut self, current: SceneFileObservation, now: Instant) -> bool {
        if current == self.stable_observation {
            self.pending = None;
            return false;
        }

        match &self.pending {
            Some(pending) if pending.observation == current && now >= pending.deadline => {
                self.stable_observation = current;
                self.pending = None;
                true
            }
            Some(pending) if pending.observation == current => false,
            _ => {
                self.pending = Some(PendingFileChange {
                    observation: current,
                    deadline: now + self.debounce,
                });
                false
            }
        }
    }
}

fn observe_scene_file(path: &Path) -> SceneFileObservation {
    match std::fs::read(path) {
        Ok(bytes) => SceneFileObservation::Present(SceneFileStamp {
            len: bytes.len() as u64,
            content_hash: fnv1a64(&bytes),
        }),
        Err(_) => SceneFileObservation::Unavailable,
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100000001b3_u64);
    }
    hash
}

fn load_and_init(app: &AppHandle, scene: &SceneRef, seed: u64) -> Option<SimState> {
    let loaded = match load_scene_from_path(&scene.path, seed) {
        Ok(loaded) => loaded,
        Err(e) => {
            tracing::error!(
                "failed to load scene {} ({}): {e}",
                scene.path.display(),
                scene.scene_id
            );
            emit_scene_load_failure(app, 0, &e);
            return None;
        }
    };

    let mut snapshot_buf = SnapshotPayload::default();
    encode_snapshot(&loaded.world, &mut snapshot_buf);

    // Emit initial static + snapshot.
    let mut seq: u64 = 0;
    emit_sim(app, seq, SimMessage::Static(loaded.static_payload.clone()));
    seq += 1;
    emit_sim(app, seq, SimMessage::Snapshot(snapshot_buf.clone()));
    seq += 1;

    Some(SimState {
        world: loaded.world,
        runner: loaded.runner,
        meta: loaded.meta,
        snapshot_buf,
        last_static: loaded.static_payload,
        seq,
        paused: false,
        speed_factor: 1.0,
    })
}

fn load_scene_from_path(path: &Path, seed: u64) -> Result<SceneLoad, SceneLoadFailure> {
    let json = std::fs::read_to_string(path).map_err(SceneLoadFailure::Read)?;
    build_scene_load(&json, seed).map_err(SceneLoadFailure::Load)
}

fn build_scene_load(json: &str, seed: u64) -> Result<SceneLoad, LoadError> {
    let loaded = load_scene_str(json, seed)?;
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

    Ok(SceneLoad {
        world,
        runner,
        meta,
        static_payload,
    })
}

#[cfg(test)]
fn replace_scene_from_json(
    state: &mut SimState,
    json: &str,
    seed: u64,
    ticks_since_snapshot: &mut u64,
) -> Result<(), LoadError> {
    let loaded = build_scene_load(json, seed)?;
    apply_scene_load(state, loaded, ticks_since_snapshot);
    Ok(())
}

fn apply_scene_load(state: &mut SimState, loaded: SceneLoad, ticks_since_snapshot: &mut u64) {
    state.world = loaded.world;
    state.runner = loaded.runner;
    state.meta = loaded.meta;
    state.last_static = loaded.static_payload;
    state.paused = false;
    *ticks_since_snapshot = 0;
}

fn tick_and_emit(app: &AppHandle, state: &mut SimState, ticks_since_snapshot: &mut u64) {
    // catch_unwind around tick to prevent engine panics from crashing the app.
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        state.runner.tick_once(&mut state.world);
    }));

    if let Err(panic_info) = result {
        let msg = panic_payload_message(&*panic_info);
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
    active_scene: &Arc<RwLock<SceneRef>>,
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
            let scene = read_active_scene(active_scene);
            tracing::info!(
                "reloading scene from {} ({})",
                scene.path.display(),
                scene.scene_id
            );
            if let Err(e) = replace_scene(app, state, &scene, seed, ticks_since_snapshot) {
                tracing::warn!("scene reload failed; preserving current scene: {e}");
            }
        }
        DriverCommand::SetScene { scene, reply } => {
            tracing::info!(
                "switching scene to {} ({})",
                scene.path.display(),
                scene.scene_id
            );
            let result = replace_scene(app, state, &scene, seed, ticks_since_snapshot);
            let reply_result = match result {
                Ok(()) => {
                    write_active_scene(active_scene, scene);
                    Ok(())
                }
                Err(e) => {
                    tracing::warn!("scene switch failed; preserving current scene: {e}");
                    Err(e.to_string())
                }
            };
            let _ = reply.send(reply_result);
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

fn replace_scene(
    app: &AppHandle,
    state: &mut SimState,
    scene: &SceneRef,
    seed: u64,
    ticks_since_snapshot: &mut u64,
) -> Result<(), SceneLoadFailure> {
    let loaded = match load_scene_from_path(&scene.path, seed) {
        Ok(loaded) => loaded,
        Err(e) => {
            emit_scene_load_failure(app, state.seq, &e);
            state.seq += 1;
            return Err(e);
        }
    };

    let static_payload = loaded.static_payload.clone();
    apply_scene_load(state, loaded, ticks_since_snapshot);

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
    Ok(())
}

// -------------------------------------------------------------------------
//  Helpers
// -------------------------------------------------------------------------

fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "engine panic (unknown payload)".to_string()
    }
}

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

fn emit_sim(app: &AppHandle, seq: u64, msg: SimMessage) {
    let env = Envelope::new(seq, msg);
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

fn emit_scene_load_failure(app: &AppHandle, seq: u64, error: &SceneLoadFailure) {
    match error {
        SceneLoadFailure::Read(e) => emit_fault(app, seq, &format!("cannot read scene: {e}")),
        SceneLoadFailure::Load(e) => emit_sim(app, seq, SimMessage::Fault(load_error_to_fault(e))),
    }
}

fn read_active_scene(active_scene: &Arc<RwLock<SceneRef>>) -> SceneRef {
    match active_scene.read() {
        Ok(scene) => scene.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

fn write_active_scene(active_scene: &Arc<RwLock<SceneRef>>, scene: SceneRef) {
    match active_scene.write() {
        Ok(mut active) => *active = scene,
        Err(poisoned) => *poisoned.into_inner() = scene,
    }
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

    #[test]
    fn panic_payload_message_preserves_string_payloads() {
        let borrowed: &'static str = "borrowed panic";
        let owned = String::from("owned panic");

        assert_eq!(panic_payload_message(&borrowed), "borrowed panic");
        assert_eq!(panic_payload_message(&owned), "owned panic");
    }

    #[test]
    fn file_watch_debounces_stable_change() {
        let debounce = Duration::from_millis(100);
        let initial = observed_file(10, 1);
        let changed = observed_file(11, 2);
        let now = Instant::now();
        let mut watcher = DebouncedFileWatch::new(initial, debounce);

        assert!(!watcher.observe(changed.clone(), now));
        assert!(!watcher.observe(changed.clone(), now + Duration::from_millis(99)));
        assert!(watcher.observe(changed.clone(), now + debounce));
        assert!(!watcher.observe(changed, now + Duration::from_millis(200)));
    }

    #[test]
    fn file_watch_resets_debounce_when_observation_changes_again() {
        let debounce = Duration::from_millis(100);
        let initial = observed_file(10, 1);
        let first_change = observed_file(11, 2);
        let second_change = observed_file(12, 3);
        let now = Instant::now();
        let mut watcher = DebouncedFileWatch::new(initial, debounce);

        assert!(!watcher.observe(first_change, now));
        assert!(!watcher.observe(second_change.clone(), now + Duration::from_millis(50)));
        assert!(!watcher.observe(second_change.clone(), now + Duration::from_millis(100)));
        assert!(watcher.observe(second_change, now + Duration::from_millis(150)));
    }

    #[test]
    fn file_watch_cancels_pending_change_when_observation_returns_to_stable() {
        let debounce = Duration::from_millis(100);
        let initial = observed_file(10, 1);
        let changed = observed_file(11, 2);
        let now = Instant::now();
        let mut watcher = DebouncedFileWatch::new(initial.clone(), debounce);

        assert!(!watcher.observe(changed.clone(), now));
        assert!(!watcher.observe(initial, now + Duration::from_millis(50)));
        assert!(!watcher.observe(changed.clone(), now + Duration::from_millis(120)));
        assert!(watcher.observe(changed, now + Duration::from_millis(220)));
    }

    #[test]
    fn file_watch_detects_same_length_content_changes() {
        assert_ne!(fnv1a64(b"abc"), fnv1a64(b"abd"));
        assert_eq!(b"abc".len(), b"abd".len());
    }

    #[test]
    fn scene_replacement_preserves_current_state_on_load_failure() {
        let mut state = state_from_json(include_str!("../../games/demo-paths.json"));
        state.seq = 42;
        state.paused = true;
        state.speed_factor = 2.5;
        let before_scene_name = state.last_static.name.clone();
        let before_nodes = state.last_static.nodes.clone();
        let before_seq = state.seq;
        let before_paused = state.paused;
        let before_speed = state.speed_factor;
        let mut ticks_since_snapshot = 2;

        let result = replace_scene_from_json(
            &mut state,
            r#"{"schema_version":1,"name":"broken","pieces":{"nodes":["#,
            0,
            &mut ticks_since_snapshot,
        );

        assert!(result.is_err());
        assert_eq!(state.last_static.name, before_scene_name);
        assert_eq!(state.last_static.nodes, before_nodes);
        assert_eq!(state.seq, before_seq);
        assert_eq!(state.paused, before_paused);
        assert_eq!(state.speed_factor, before_speed);
        assert_eq!(ticks_since_snapshot, 2);
    }

    #[test]
    fn scene_replacement_commits_loaded_scene_atomically() {
        let mut state = state_from_json(include_str!("../../games/demo-paths.json"));
        state.paused = true;
        let mut ticks_since_snapshot = 2;

        replace_scene_from_json(
            &mut state,
            include_str!("../../games/demo-paths.json"),
            0,
            &mut ticks_since_snapshot,
        )
        .expect("valid scene reload");

        assert_eq!(state.last_static.name, "demo-paths");
        assert!(!state.paused);
        assert_eq!(ticks_since_snapshot, 0);
    }

    fn observed_file(len: u64, content_hash: u64) -> SceneFileObservation {
        SceneFileObservation::Present(SceneFileStamp { len, content_hash })
    }

    fn state_from_json(json: &str) -> SimState {
        let loaded = build_scene_load(json, 0).expect("valid scene fixture");
        let mut snapshot_buf = SnapshotPayload::default();
        encode_snapshot(&loaded.world, &mut snapshot_buf);
        SimState {
            world: loaded.world,
            runner: loaded.runner,
            meta: loaded.meta,
            snapshot_buf,
            last_static: loaded.static_payload,
            seq: 0,
            paused: false,
            speed_factor: 1.0,
        }
    }
}
