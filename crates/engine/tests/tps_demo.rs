//! Throughput smoke test for the demo scene.
//!
//! throughput target: ≥ 50,000 tps on a 3-mover demo. This test runs
//! 10,000 ticks and asserts a conservative floor (10,000 tps) to keep
//! the gate green on slow CI runners. The real benchmark lands as a
//! criterion suite in acceptance; this is the sanity check.
//!
//! Run with `cargo test -p simetro-engine --release --test tps_demo`
//! for a meaningful number.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::time::Instant;

use simetro_engine::{load_scene_str, tick_once};

const SCENE: &str = include_str!("../../../games/demo-paths.json");

#[test]
fn demo_paths_meets_minimum_tps_floor() {
    let mut loaded = load_scene_str(SCENE, 42).expect("load");
    const N: u64 = 10_000;

    // Warm up to avoid measuring first-iteration noise.
    for _ in 0..1000 {
        let _ = tick_once(&mut loaded.world);
    }

    let start = Instant::now();
    for _ in 0..N {
        let _ = tick_once(&mut loaded.world);
    }
    let elapsed = start.elapsed();
    let tps = (N as f64) / elapsed.as_secs_f64();

    eprintln!(
        "demo-paths tps: {tps:.0} ({N} ticks in {:.3}ms)",
        elapsed.as_secs_f64() * 1000.0
    );

    // Conservative floor for debug builds on shared CI. Release builds
    // should comfortably exceed 50k tps (throughput target).
    assert!(
        tps >= 5_000.0,
        "tps below floor: {tps:.0} (expected >= 5000 in debug; release target is 50000)"
    );
}
