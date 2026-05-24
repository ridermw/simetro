//! Determinism baseline gate (PLAN §16).
//!
//! Re-hashes the canonical demo scene at the canonical seed and tick
//! count and diffs against the committed baseline. Any drift fails
//! `cargo test` — same gate CI enforces, so a contributor catches the
//! drift before pushing.
//!
//! Update procedure when a deliberate change to engine determinism
//! lands:
//!   1. cargo run --release -p simetro-headless -- hash \
//!      --scene games/demo-paths.json --ticks 10000 --seed 42
//!   2. Replace the contents of tests/baselines/demo-paths.hash
//!   3. Document the cause in the commit message (PLAN §17).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{hash_run, load_scene_str, TickRunner};

const SCENE: &str = include_str!("../../../games/demo-paths.json");
const BASELINE: &str = include_str!("../../../tests/baselines/demo-paths.hash");
const TICKS: u64 = 10_000;
const SEED: u64 = 42;

#[test]
fn demo_paths_matches_committed_baseline() {
    let mut loaded = load_scene_str(SCENE, SEED).expect("load demo scene");
    let mut runner = TickRunner::new();
    runner.reserve_for(loaded.world.movers.len());
    let actual = hash_run(&mut loaded.world, &mut runner, TICKS);
    let expected = BASELINE.trim();
    assert_eq!(
        actual, expected,
        "determinism baseline drift detected.\n\
         expected: {expected}\n\
         actual:   {actual}\n\
         If this drift is intentional, refresh tests/baselines/demo-paths.hash \
         per the procedure in this file's header comment."
    );
}
