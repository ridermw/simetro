//! `simetro-headless` binary.
//!
//! Subcommands per PLAN §19 step 13:
//!
//! ```text
//!   simetro-headless run            --scene PATH --ticks N --seed S
//!   simetro-headless bench          --scene PATH --ticks N --seed S
//!   simetro-headless hash           --scene PATH --ticks N --seed S
//!   simetro-headless replay         --log PATH                    (P2 placeholder)
//!   simetro-headless export-session --scene PATH --ticks N --seed S --out DIR
//! ```
//!
//! Exits non-zero on load failures (LoadError surfaces as a printed
//! diagnostic + exit code 2). Other failure modes (IO on session
//! export) exit code 3. Process never panics — every error path
//! flows through `Result` and `std::process::exit`.

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, Subcommand};
use simetro_engine::{hash_run, load_scene_str, TickRunner};

const DEFAULT_TICKS: u64 = 10_000;
const DEFAULT_SEED: u64 = 42;

#[derive(Parser)]
#[command(name = "simetro-headless", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a scene for N ticks (prints summary).
    Run {
        #[arg(long)]
        scene: PathBuf,
        #[arg(long, default_value_t = DEFAULT_TICKS)]
        ticks: u64,
        #[arg(long, default_value_t = DEFAULT_SEED)]
        seed: u64,
    },
    /// Benchmark tick throughput; reports tps.
    Bench {
        #[arg(long)]
        scene: PathBuf,
        #[arg(long, default_value_t = DEFAULT_TICKS)]
        ticks: u64,
        #[arg(long, default_value_t = DEFAULT_SEED)]
        seed: u64,
    },
    /// Emit a deterministic SHA-256 of (world_state, event_stream).
    /// Use this to author or refresh `tests/baselines/<scene>.hash`.
    Hash {
        #[arg(long)]
        scene: PathBuf,
        #[arg(long, default_value_t = DEFAULT_TICKS)]
        ticks: u64,
        #[arg(long, default_value_t = DEFAULT_SEED)]
        seed: u64,
    },
    /// Replay an AgentLog (Phase-2 feature; placeholder in P1).
    Replay {
        #[arg(long)]
        log: PathBuf,
    },
    /// Export a session bundle (scene + AgentLog + tracing + hash).
    ExportSession {
        #[arg(long)]
        scene: PathBuf,
        #[arg(long, default_value_t = DEFAULT_TICKS)]
        ticks: u64,
        #[arg(long, default_value_t = DEFAULT_SEED)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    let code = match cli.cmd {
        Cmd::Run { scene, ticks, seed } => cmd_run(&scene, ticks, seed),
        Cmd::Bench { scene, ticks, seed } => cmd_bench(&scene, ticks, seed),
        Cmd::Hash { scene, ticks, seed } => cmd_hash(&scene, ticks, seed),
        Cmd::Replay { log } => cmd_replay(&log),
        Cmd::ExportSession {
            scene,
            ticks,
            seed,
            out,
        } => cmd_export_session(&scene, ticks, seed, &out),
    };
    std::process::exit(code);
}

fn read_scene(path: &std::path::Path) -> Result<String, i32> {
    std::fs::read_to_string(path).map_err(|e| {
        eprintln!("error: failed to read {}: {e}", path.display());
        3
    })
}

fn load(path: &std::path::Path, seed: u64) -> Result<simetro_engine::LoadedScene, i32> {
    let src = read_scene(path)?;
    load_scene_str(&src, seed).map_err(|e| {
        eprintln!("LoadError: {e}");
        2
    })
}

fn cmd_run(scene: &std::path::Path, ticks: u64, seed: u64) -> i32 {
    let mut loaded = match load(scene, seed) {
        Ok(v) => v,
        Err(c) => return c,
    };
    let mut runner = TickRunner::new();
    runner.reserve_for(loaded.world.movers.len());
    let mut events_total: u64 = 0;
    let started = Instant::now();
    for _ in 0..ticks {
        runner.tick_once(&mut loaded.world);
        events_total += runner.events().len() as u64;
    }
    let elapsed = started.elapsed();
    let tps = ticks as f64 / elapsed.as_secs_f64().max(1e-9);
    println!(
        "run: ticks={ticks} seed={seed} elapsed={:.3}s tps={tps:.0} events={events_total}",
        elapsed.as_secs_f64()
    );
    0
}

fn cmd_bench(scene: &std::path::Path, ticks: u64, seed: u64) -> i32 {
    // bench differs from run by minimizing per-tick overhead: no event
    // counter, no log, only tps.
    let mut loaded = match load(scene, seed) {
        Ok(v) => v,
        Err(c) => return c,
    };
    let mut runner = TickRunner::new();
    runner.reserve_for(loaded.world.movers.len());
    // Warmup (200 ticks) so we measure steady-state.
    let warmup = ticks.min(200);
    for _ in 0..warmup {
        runner.tick_once(&mut loaded.world);
    }
    let started = Instant::now();
    for _ in 0..ticks {
        runner.tick_once(&mut loaded.world);
    }
    let elapsed = started.elapsed();
    let tps = ticks as f64 / elapsed.as_secs_f64().max(1e-9);
    println!(
        "bench: ticks={ticks} seed={seed} warmup={warmup} elapsed={:.3}s tps={tps:.0}",
        elapsed.as_secs_f64()
    );
    0
}

fn cmd_hash(scene: &std::path::Path, ticks: u64, seed: u64) -> i32 {
    let mut loaded = match load(scene, seed) {
        Ok(v) => v,
        Err(c) => return c,
    };
    let mut runner = TickRunner::new();
    runner.reserve_for(loaded.world.movers.len());
    let hex = hash_run(&mut loaded.world, &mut runner, ticks);
    println!("{hex}");
    0
}

fn cmd_replay(log: &std::path::Path) -> i32 {
    // P2: deserialize each AgentLogEntry and re-apply parsed_action.
    // P1: parse the file, validate shape, report counts so the bin is
    // at least useful for sanity checks.
    let src = match std::fs::read_to_string(log) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to read {}: {e}", log.display());
            return 3;
        }
    };
    let mut total = 0u64;
    let mut bad = 0u64;
    for line in src.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<simetro_engine::AgentLogEntry>(line) {
            Ok(_) => total += 1,
            Err(_) => bad += 1,
        }
    }
    println!("replay (P1 placeholder): entries={total} malformed={bad}");
    if bad > 0 {
        2
    } else {
        0
    }
}

fn cmd_export_session(
    scene: &std::path::Path,
    ticks: u64,
    seed: u64,
    out: &std::path::Path,
) -> i32 {
    // Layout per PLAN §15:
    //  out/
    //    scene.json
    //    baseline.hash
    //    manifest.json
    //    agent-log.jsonl  (empty in P1; populated when an agent is registered)
    //    tracing.jsonl    (empty in P1; populated when tracing subscriber is wired)
    if let Err(e) = std::fs::create_dir_all(out) {
        eprintln!("error: failed to create {}: {e}", out.display());
        return 3;
    }
    let src = match read_scene(scene) {
        Ok(s) => s,
        Err(c) => return c,
    };
    if let Err(e) = std::fs::write(out.join("scene.json"), &src) {
        eprintln!("error: copying scene: {e}");
        return 3;
    }
    let mut loaded = match load_scene_str(&src, seed) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("LoadError: {e}");
            return 2;
        }
    };
    let mut runner = TickRunner::new();
    runner.reserve_for(loaded.world.movers.len());
    let hex = hash_run(&mut loaded.world, &mut runner, ticks);
    if let Err(e) = std::fs::write(out.join("baseline.hash"), format!("{hex}\n")) {
        eprintln!("error: writing baseline: {e}");
        return 3;
    }
    // Placeholder empty log + tracing files so the bundle layout is
    // stable across builds.
    if let Err(e) = std::fs::File::create(out.join("agent-log.jsonl")) {
        eprintln!("error: creating agent-log.jsonl: {e}");
        return 3;
    }
    if let Err(e) = std::fs::File::create(out.join("tracing.jsonl")) {
        eprintln!("error: creating tracing.jsonl: {e}");
        return 3;
    }
    let manifest = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "build": option_env!("VERGEN_GIT_SHA").unwrap_or("unknown"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "seed": seed,
        "ticks": ticks,
        "scene": scene.file_name().and_then(|s| s.to_str()).unwrap_or(""),
    });
    let manifest_str = match serde_json::to_string_pretty(&manifest) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: serializing manifest: {e}");
            return 3;
        }
    };
    let mut f = match std::fs::File::create(out.join("manifest.json")) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: creating manifest.json: {e}");
            return 3;
        }
    };
    if let Err(e) = f.write_all(manifest_str.as_bytes()) {
        eprintln!("error: writing manifest.json: {e}");
        return 3;
    }
    println!("export-session: {} (hash={hex})", out.display());
    0
}
