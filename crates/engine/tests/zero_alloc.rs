//! Zero-allocation invariant for the steady-state tick loop (PLAN §14).
//!
//! After a warmup phase (during which `TickRunner` grows its scratch
//! buffers), subsequent ticks must allocate **zero** blocks. This test
//! uses `dhat` as the global allocator to count allocations precisely.
//!
//! NOTE: dhat replaces the global allocator for this test binary only.
//! Other test binaries in the crate are unaffected.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{load_scene_str, TickRunner};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const SCENE: &str = include_str!("../../../games/demo-paths.json");

#[test]
fn tick_makes_no_allocations_after_warmup() {
    let _profiler = dhat::Profiler::builder().testing().build();

    let mut loaded = load_scene_str(SCENE, 42).expect("load scene");
    let mut runner = TickRunner::new();
    runner.reserve_for(loaded.world.movers.len());

    // Warmup phase: scratch buffers grow to steady-state capacity.
    // Run enough ticks to cover lifecycle (spawn), movement (arrive),
    // and interaction (re-route) — all three system pipelines must
    // have populated their internal Vecs at least once.
    for _ in 0..200 {
        runner.tick_once(&mut loaded.world);
    }

    let baseline = dhat::HeapStats::get();

    // Steady-state phase: assert zero allocation deltas.
    for _ in 0..1000 {
        runner.tick_once(&mut loaded.world);
    }

    let after = dhat::HeapStats::get();

    let block_delta = after.total_blocks.saturating_sub(baseline.total_blocks);
    let byte_delta = after.total_bytes.saturating_sub(baseline.total_bytes);

    assert_eq!(
        block_delta, 0,
        "tick loop allocated {block_delta} blocks ({byte_delta} bytes) after warmup; \
         PLAN §14 requires zero per-tick allocations"
    );
}
