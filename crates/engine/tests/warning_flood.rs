//! Characterization test for SL1 transform warning emission volume.
//!
//! User-reported bug (2026-05-26): yellow boxes flood the right side
//! of the canvas in clinic-triage-desk. The frontend `WarningStrip`
//! renders each `WarningPayload` as an amber pill and stacks them.
//!
//! This test characterizes the engine side: the SL1 transform runtime
//! emits a fresh warning every tick a transform fails (Starved /
//! SlotMissed / Failed / Late / Blocked). That is the INTENDED
//! pressure signal for stressed scenes — clinic-triage-desk is
//! authored so transforms repeatedly miss slots under load. The
//! engine is correct; the bug lives in the frontend's lack of
//! display rate-limiting.
//!
//! Purpose of this test:
//! 1. Provides ground-truth numbers for the frontend fix to reason
//!    against: "if engine can emit ~700 warnings over 600 ticks,
//!    WarningStrip must cap or coalesce visible pills."
//! 2. Acts as a regression alarm: if engine warning volume jumps an
//!    order of magnitude (e.g. from ~700 to ~7000 in the same window),
//!    something in the SL1 runtime is mis-firing and the cap below
//!    will trip, prompting investigation.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{load_scene_str, TickRunner};
use simetro_protocol::{SimMessage, WarningPayload};
use std::collections::BTreeMap;

const SCENE: &str = include_str!("../../../games/clinic-triage-desk.json");
const SEED: u64 = 0;
const TICK_BUDGET: u64 = 600;

/// Upper bound on total engine-emitted warnings per 600-tick run.
/// Empirically clinic-triage-desk currently sits around 670; if a
/// future engine change pushes this over 2000 something has gone
/// wrong (e.g., a transform stuck in a per-tick-per-attempt warning
/// loop, or warnings emitted from a tight inner loop instead of once
/// per transition).
const ENGINE_WARNING_CHARACTERIZATION_CAP: usize = 2000;

#[test]
fn clinic_triage_engine_warning_volume_is_characterized() {
    let mut scene = load_scene_str(SCENE, SEED).expect("scene loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut total: usize = 0;

    for _ in 0..TICK_BUDGET {
        runner.tick_once(&mut world);
        for msg in runner.messages() {
            if let SimMessage::Warning(payload) = msg {
                *counts.entry(warning_detail_key(payload)).or_insert(0) += 1;
                total += 1;
            }
        }
    }

    // Diagnostic — visible in `cargo test -- --nocapture`.
    eprintln!("clinic-triage-desk warnings over {TICK_BUDGET} ticks: total={total}");
    for (key, count) in &counts {
        eprintln!("  {key}: {count}");
    }

    // Characterization: engine is allowed to emit many warnings here
    // (scenes are authored to apply pressure). The frontend
    // `WarningStrip` is responsible for not flooding the viewport.
    // This cap exists only to alarm on engine-side regressions.
    assert!(
        total <= ENGINE_WARNING_CHARACTERIZATION_CAP,
        "engine emitted {total} warnings over {TICK_BUDGET} ticks; \
         characterization cap is {ENGINE_WARNING_CHARACTERIZATION_CAP}. \
         This jump suggests an SL1 runtime regression — investigate \
         the per-kind breakdown above."
    );
}

fn warning_detail_key(p: &WarningPayload) -> String {
    match p {
        WarningPayload::Sl1Transform {
            transform_id,
            event,
            ..
        } => {
            format!("Sl1Transform/{transform_id}/{event:?}")
        }
        WarningPayload::Sl1Demand {
            demand_id, event, ..
        } => {
            format!("Sl1Demand/{demand_id}/{event:?}")
        }
        WarningPayload::Sl1Pressure {
            pressure_id, event, ..
        } => {
            format!("Sl1Pressure/{pressure_id}/{event:?}")
        }
        WarningPayload::Sl1Objective {
            objective_id,
            event,
            ..
        } => {
            format!("Sl1Objective/{objective_id}/{event:?}")
        }
        WarningPayload::InvalidAction { .. } => "InvalidAction".to_string(),
        WarningPayload::Behind { .. } => "Behind".to_string(),
        WarningPayload::TickOverBudget { .. } => "TickOverBudget".to_string(),
        WarningPayload::AgentLogSlow => "AgentLogSlow".to_string(),
    }
}
