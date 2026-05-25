//! `scenario_language_v1` Transforms integration tests (PR 4).
//!
//! Exercises:
//!   - Every new `Sl1LoadError::Transform*` variant via minimal scenes.
//!   - Cross-validation against places, things, and place capacity buckets.
//!   - Runtime state machine: Idle → Running → Idle on success,
//!     Idle → Starved on missing inputs, Idle → Blocked on capacity/output
//!     overflow, Running → Late → Failed (Drop) or Late → retry → Idle
//!     (RetryThenWarn at max_attempts).
//!   - Capacity contention: stable id order wins.
//!   - SlotMissed emitted once when cadence fires mid-Running.
//!   - `DegradeQuality` rejected until PR 8.
//!   - The `sl1-transforms.json` fixture ticks deterministically against
//!     `tests/baselines/sl1-transforms.hash`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_engine::{
    encode_snapshot, encode_static, hash_run, load_scene_str, FreshnessState, LoadError,
    Sl1LoadError, Sl1TransformState, TickRunner,
};
use simetro_protocol::{SimMessage, Sl1TransformWarningKind, SnapshotPayload, WarningPayload};

const TRANSFORMS_SCENE: &str = include_str!("fixtures/sl1-transforms.json");
const TRANSFORMS_BASELINE: &str = include_str!("../../../tests/baselines/sl1-transforms.hash");
const TICKS: u64 = 200;

fn scene_with_transforms(transforms_json: &str) -> String {
    scene_with("[]", "[]", transforms_json)
}

fn scene_with(places_json: &str, things_json: &str, transforms_json: &str) -> String {
    format!(
        r##"{{
            "schema_version": 1,
            "name": "sl1-transforms-test",
            "theme": {{
                "palette": ["#000000"],
                "background_index": 0,
                "font": "system-ui"
            }},
            "pieces": {{ "nodes": [], "paths": [], "movers": [] }},
            "scenario_language_v1": {{
                "places": {places_json},
                "things": {things_json},
                "transforms": {transforms_json}
            }}
        }}"##
    )
}

fn expect_sl1_err(json: String) -> Sl1LoadError {
    match load_scene_str(&json, 0).expect_err("expected load failure") {
        LoadError::Sl1(e) => e,
        other => panic!("expected LoadError::Sl1, got {other:?}"),
    }
}

fn default_places() -> &'static str {
    r#"[
        {
            "id": "factory",
            "role": "producer",
            "pos": [0.0, 0.0],
            "capacity": { "machine_hours": 4 },
            "storage": {
                "raw": { "capacity": 100, "initial": 50 },
                "widget": { "capacity": 50, "initial": 0 }
            },
            "accepts": ["raw"],
            "produces": ["widget"]
        }
    ]"#
}

fn default_things() -> &'static str {
    r#"[
        {"id": "raw", "kind": "input", "tags": []},
        {"id": "widget", "kind": "output", "tags": [], "freshness_budget_ticks": 100}
    ]"#
}

// -------------------------------------------------------------------
// Per-variant Transform* load error coverage
// -------------------------------------------------------------------

#[test]
fn transform_invalid_id_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{
            "id": "BAD ID",
            "type": "x",
            "runs_on": "factory",
            "outputs": [{"thing": "widget", "amount": 1}],
            "cadence_ticks": 5,
            "duration_ticks": 2,
            "deadline_ticks": 5,
            "failure_policy": "drop"
        }]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformInvalidId { ref id } if id == "BAD ID"),
        "{err:?}"
    );
}

#[test]
fn transform_duplicate_id_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[
            {"id": "dup", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "drop"},
            {"id": "dup", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "drop"}
        ]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformDuplicateId { ref id } if id == "dup"),
        "{err:?}"
    );
}

#[test]
fn transform_empty_type_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "drop"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformEmptyType { .. }),
        "{err:?}"
    );
}

#[test]
fn transform_unknown_place_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "ghost",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "drop"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformUnknownPlace { ref place, .. } if place == "ghost"),
        "{err:?}"
    );
}

#[test]
fn transform_empty_outputs_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "drop"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformEmptyOutputs { .. }),
        "{err:?}"
    );
}

#[test]
fn transform_unknown_thing_in_inputs_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "inputs": [{"thing": "ghost", "amount": 1}],
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "drop"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformUnknownThing { ref value, .. } if value == "ghost"),
        "{err:?}"
    );
}

#[test]
fn transform_unknown_thing_in_outputs_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "ghost", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "drop"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformUnknownThing { ref value, .. } if value == "ghost"),
        "{err:?}"
    );
}

#[test]
fn transform_duplicate_io_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "inputs": [{"thing": "raw", "amount": 1}, {"thing": "raw", "amount": 2}],
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "drop"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformDuplicateIo { .. }),
        "{err:?}"
    );
}

#[test]
fn transform_io_amount_zero_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 0}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "drop"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformIoAmountZero { .. }),
        "{err:?}"
    );
}

#[test]
fn transform_cadence_zero_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 0, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "drop"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformCadenceZero { .. }),
        "{err:?}"
    );
}

#[test]
fn transform_duration_zero_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 0, "deadline_ticks": 5,
             "failure_policy": "drop"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformDurationZero { .. }),
        "{err:?}"
    );
}

#[test]
fn transform_deadline_zero_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 0,
             "failure_policy": "drop"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformDeadlineZero { .. }),
        "{err:?}"
    );
}

#[test]
fn transform_deadline_less_than_duration_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 10, "deadline_ticks": 3,
             "failure_policy": "drop"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformDeadlineLessThanDuration { .. }),
        "{err:?}"
    );
}

#[test]
fn transform_capacity_cost_empty_key_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "capacity_cost": {"": 1},
             "failure_policy": "drop"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformCapacityCostEmptyKey { .. }),
        "{err:?}"
    );
}

#[test]
fn transform_capacity_cost_zero_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "capacity_cost": {"machine_hours": 0},
             "failure_policy": "drop"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformCapacityCostZero { .. }),
        "{err:?}"
    );
}

#[test]
fn transform_unknown_capacity_key_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "capacity_cost": {"ghost_bucket": 1},
             "failure_policy": "drop"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformUnknownCapacityKey { .. }),
        "{err:?}"
    );
}

#[test]
fn transform_invalid_failure_policy_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "explode"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformInvalidFailurePolicy { ref policy, .. } if policy == "explode"),
        "{err:?}"
    );
}

#[test]
fn transform_degrade_quality_rejected_until_pr8() {
    // PR 4 only ships retry_then_warn and drop; degrade_quality must
    // be rejected until PR 8 adds quality contracts.
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "degrade_quality"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformInvalidFailurePolicy { ref policy, .. } if policy == "degrade_quality"),
        "{err:?}"
    );
}

#[test]
fn transform_max_attempts_zero_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "retry_then_warn",
             "max_attempts": 0}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformMaxAttemptsZero { .. }),
        "{err:?}"
    );
}

#[test]
fn transform_unknown_field_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "drop",
             "mystery_field": 42}]"#,
    ));
    assert!(matches!(err, Sl1LoadError::Parse { .. }), "{err:?}");
}

#[test]
fn transform_io_unknown_field_rejected() {
    let err = expect_sl1_err(scene_with(
        default_places(),
        default_things(),
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1, "extra": 9}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "drop"}]"#,
    ));
    assert!(matches!(err, Sl1LoadError::Parse { .. }), "{err:?}");
}

#[test]
fn transforms_section_alone_rejected() {
    // PRs 5-11 still require an explicit places/things stack — but
    // transforms with no places must fail TransformUnknownPlace.
    let err = expect_sl1_err(scene_with_transforms(
        r#"[{"id": "t", "type": "x", "runs_on": "factory",
             "outputs": [{"thing": "widget", "amount": 1}],
             "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
             "failure_policy": "drop"}]"#,
    ));
    assert!(
        matches!(err, Sl1LoadError::TransformUnknownPlace { .. }),
        "{err:?}"
    );
}

// -------------------------------------------------------------------
// Runtime state machine behavior
// -------------------------------------------------------------------

fn load_and_tick(json: &str, ticks: u64) -> (TickRunner, simetro_engine::World, Vec<SimMessage>) {
    use simetro_engine::World;
    let mut scene = load_scene_str(json, 0).expect("scene loads");
    let mut world: World = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    let mut all_messages: Vec<SimMessage> = Vec::new();
    for _ in 0..ticks {
        runner.tick_once(&mut world);
        for m in runner.messages() {
            all_messages.push(m.clone());
        }
    }
    (runner, world, all_messages)
}

fn count_warnings(
    messages: &[SimMessage],
    transform_id: &str,
    event: Sl1TransformWarningKind,
) -> usize {
    messages
        .iter()
        .filter(|m| {
            matches!(
                m,
                SimMessage::Warning(WarningPayload::Sl1Transform {
                    transform_id: tid,
                    event: e,
                    ..
                }) if tid == transform_id && *e == event
            )
        })
        .count()
}

#[test]
fn transform_runs_and_produces_output() {
    let json = scene_with(
        r#"[{
            "id": "p", "role": "x", "pos": [0,0],
            "capacity": {"m": 4},
            "storage": {
                "raw": {"capacity": 100, "initial": 50},
                "widget": {"capacity": 50, "initial": 0}
            },
            "accepts": ["raw"], "produces": ["widget"]
        }]"#,
        r#"[
            {"id": "raw", "kind": "i", "tags": []},
            {"id": "widget", "kind": "o", "tags": [], "freshness_budget_ticks": 1000}
        ]"#,
        r#"[{
            "id": "t", "type": "x", "runs_on": "p",
            "inputs": [{"thing": "raw", "amount": 1}],
            "outputs": [{"thing": "widget", "amount": 1}],
            "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
            "capacity_cost": {"m": 2},
            "failure_policy": "drop"
        }]"#,
    );
    let (_, world, _) = load_and_tick(&json, 50);
    let runtime = world.sl1_runtime.as_ref().expect("runtime present");
    let widget_count = runtime
        .inventories
        .get("p")
        .and_then(|m| m.get("widget"))
        .copied()
        .unwrap_or(0);
    assert!(
        widget_count > 0,
        "expected widgets produced, got {widget_count}"
    );
}

#[test]
fn transform_starves_with_missing_inputs() {
    // Same transform but raw initial = 0.
    let json = scene_with(
        r#"[{
            "id": "p", "role": "x", "pos": [0,0],
            "capacity": {"m": 4},
            "storage": {
                "raw": {"capacity": 100, "initial": 0},
                "widget": {"capacity": 50, "initial": 0}
            },
            "accepts": ["raw"], "produces": ["widget"]
        }]"#,
        r#"[
            {"id": "raw", "kind": "i", "tags": []},
            {"id": "widget", "kind": "o", "tags": []}
        ]"#,
        r#"[{
            "id": "t", "type": "x", "runs_on": "p",
            "inputs": [{"thing": "raw", "amount": 1}],
            "outputs": [{"thing": "widget", "amount": 1}],
            "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
            "failure_policy": "retry_then_warn", "max_attempts": 3
        }]"#,
    );
    let (_, _, messages) = load_and_tick(&json, 20);
    let starved = count_warnings(&messages, "t", Sl1TransformWarningKind::Starved);
    assert!(starved >= 1, "expected ≥1 Starved warning, got {starved}");
}

#[test]
fn transform_blocked_on_output_storage_overflow() {
    // Output storage cap = 1, initial = 1 → second cadence cannot fit.
    let json = scene_with(
        r#"[{
            "id": "p", "role": "x", "pos": [0,0],
            "capacity": {"m": 4},
            "storage": {
                "raw": {"capacity": 100, "initial": 100},
                "widget": {"capacity": 1, "initial": 1}
            },
            "accepts": ["raw"], "produces": ["widget"]
        }]"#,
        r#"[
            {"id": "raw", "kind": "i", "tags": []},
            {"id": "widget", "kind": "o", "tags": []}
        ]"#,
        r#"[{
            "id": "t", "type": "x", "runs_on": "p",
            "inputs": [{"thing": "raw", "amount": 1}],
            "outputs": [{"thing": "widget", "amount": 1}],
            "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
            "failure_policy": "drop"
        }]"#,
    );
    let (_, _, messages) = load_and_tick(&json, 10);
    let blocked = count_warnings(&messages, "t", Sl1TransformWarningKind::Blocked);
    assert!(blocked >= 1, "expected ≥1 Blocked warning, got {blocked}");
}

#[test]
fn transform_capacity_contention_lower_id_wins() {
    // Two transforms each cost 3 of "m" out of 4 total: only one can
    // run at a time. With cadence=10, duration=8, deadline=3, when `a`
    // takes the slot, `b` is Blocked and breaches its deadline before
    // `a` releases — `b` fails every cycle, `a` produces every cycle.
    let json = scene_with(
        r#"[{
            "id": "p", "role": "x", "pos": [0,0],
            "capacity": {"m": 4},
            "storage": {
                "raw": {"capacity": 1000, "initial": 1000},
                "wa": {"capacity": 100, "initial": 0},
                "wb": {"capacity": 100, "initial": 0}
            },
            "accepts": ["raw"], "produces": ["wa", "wb"]
        }]"#,
        r#"[
            {"id": "raw", "kind": "i", "tags": []},
            {"id": "wa", "kind": "o", "tags": []},
            {"id": "wb", "kind": "o", "tags": []}
        ]"#,
        r#"[
            {"id": "a", "type": "x", "runs_on": "p",
             "inputs": [{"thing": "raw", "amount": 1}],
             "outputs": [{"thing": "wa", "amount": 1}],
             "cadence_ticks": 10, "duration_ticks": 8, "deadline_ticks": 9,
             "capacity_cost": {"m": 3},
             "failure_policy": "drop"},
            {"id": "b", "type": "x", "runs_on": "p",
             "inputs": [{"thing": "raw", "amount": 1}],
             "outputs": [{"thing": "wb", "amount": 1}],
             "cadence_ticks": 10, "duration_ticks": 2, "deadline_ticks": 2,
             "capacity_cost": {"m": 3},
             "failure_policy": "drop"}
        ]"#,
    );
    let (_, world, messages) = load_and_tick(&json, 100);
    let runtime = world.sl1_runtime.as_ref().expect("runtime present");
    let wa = runtime
        .inventories
        .get("p")
        .and_then(|m| m.get("wa"))
        .copied()
        .unwrap_or(0);
    let wb = runtime
        .inventories
        .get("p")
        .and_then(|m| m.get("wb"))
        .copied()
        .unwrap_or(0);
    assert!(wa > 0, "lower id `a` should produce, got wa={wa}");
    assert_eq!(
        wb, 0,
        "higher id `b` should be starved by contention, got wb={wb}"
    );
    let b_blocked = count_warnings(&messages, "b", Sl1TransformWarningKind::Blocked);
    assert!(b_blocked > 0, "expected `b` to emit Blocked at least once");
}

#[test]
fn transform_late_with_drop_policy_fails_immediately() {
    // Duration = deadline = 5; with raw=0 the transform starves, and
    // at tick > scheduled+deadline, drop policy emits Failed and resets.
    let json = scene_with(
        r#"[{
            "id": "p", "role": "x", "pos": [0,0],
            "capacity": {"m": 4},
            "storage": {
                "raw": {"capacity": 100, "initial": 0},
                "widget": {"capacity": 50, "initial": 0}
            },
            "accepts": ["raw"], "produces": ["widget"]
        }]"#,
        r#"[
            {"id": "raw", "kind": "i", "tags": []},
            {"id": "widget", "kind": "o", "tags": []}
        ]"#,
        r#"[{
            "id": "t", "type": "x", "runs_on": "p",
            "inputs": [{"thing": "raw", "amount": 1}],
            "outputs": [{"thing": "widget", "amount": 1}],
            "cadence_ticks": 10, "duration_ticks": 2, "deadline_ticks": 3,
            "failure_policy": "drop"
        }]"#,
    );
    let (_, _, messages) = load_and_tick(&json, 30);
    let failed = count_warnings(&messages, "t", Sl1TransformWarningKind::Failed);
    assert!(
        failed >= 1,
        "expected ≥1 Failed warning under drop, got {failed}"
    );
}

#[test]
fn transform_late_with_retry_then_warn_caps_at_max_attempts() {
    let json = scene_with(
        r#"[{
            "id": "p", "role": "x", "pos": [0,0],
            "capacity": {"m": 4},
            "storage": {
                "raw": {"capacity": 100, "initial": 0},
                "widget": {"capacity": 50, "initial": 0}
            },
            "accepts": ["raw"], "produces": ["widget"]
        }]"#,
        r#"[
            {"id": "raw", "kind": "i", "tags": []},
            {"id": "widget", "kind": "o", "tags": []}
        ]"#,
        r#"[{
            "id": "t", "type": "x", "runs_on": "p",
            "inputs": [{"thing": "raw", "amount": 1}],
            "outputs": [{"thing": "widget", "amount": 1}],
            "cadence_ticks": 50, "duration_ticks": 2, "deadline_ticks": 3,
            "failure_policy": "retry_then_warn", "max_attempts": 2
        }]"#,
    );
    let (_, _, messages) = load_and_tick(&json, 100);
    let failed = count_warnings(&messages, "t", Sl1TransformWarningKind::Failed);
    assert!(
        failed >= 1,
        "expected ≥1 Failed warning at max_attempts, got {failed}"
    );
}

#[test]
fn freshness_updates_on_transform_completion() {
    let json = scene_with(
        r#"[{
            "id": "p", "role": "x", "pos": [0,0],
            "capacity": {"m": 4},
            "storage": {
                "raw": {"capacity": 100, "initial": 50},
                "widget": {"capacity": 50, "initial": 0}
            },
            "accepts": ["raw"], "produces": ["widget"]
        }]"#,
        r#"[
            {"id": "raw", "kind": "i", "tags": []},
            {"id": "widget", "kind": "o", "tags": [], "freshness_budget_ticks": 1000}
        ]"#,
        r#"[{
            "id": "t", "type": "x", "runs_on": "p",
            "inputs": [{"thing": "raw", "amount": 1}],
            "outputs": [{"thing": "widget", "amount": 1}],
            "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
            "failure_policy": "drop"
        }]"#,
    );
    let (_, world, _) = load_and_tick(&json, 30);
    let runtime = world.sl1_runtime.as_ref().expect("runtime present");
    let fresh = runtime
        .freshness
        .get(&("p".to_string(), "widget".to_string()))
        .copied()
        .expect("freshness key present");
    assert!(
        matches!(fresh, FreshnessState::Ok { .. }),
        "expected widget freshness Ok, got {fresh:?}"
    );
}

#[test]
fn transform_idle_when_no_cadence_match() {
    let json = scene_with(
        r#"[{
            "id": "p", "role": "x", "pos": [0,0],
            "capacity": {"m": 4},
            "storage": {"widget": {"capacity": 50, "initial": 0}},
            "accepts": [], "produces": ["widget"]
        }]"#,
        r#"[{"id": "widget", "kind": "o", "tags": []}]"#,
        r#"[{
            "id": "t", "type": "x", "runs_on": "p",
            "outputs": [{"thing": "widget", "amount": 1}],
            "cadence_ticks": 1000, "duration_ticks": 2, "deadline_ticks": 5,
            "failure_policy": "drop"
        }]"#,
    );
    let (_, world, _) = load_and_tick(&json, 50);
    let runtime = world.sl1_runtime.as_ref().expect("runtime present");
    let state = runtime.transforms.get("t").expect("transform state");
    assert_eq!(
        *state,
        Sl1TransformState::Idle,
        "expected Idle, got {state:?}"
    );
}

#[test]
fn transform_snapshot_carries_runtime_state() {
    let json = scene_with(
        r#"[{
            "id": "p", "role": "x", "pos": [0,0],
            "capacity": {"m": 4},
            "storage": {
                "raw": {"capacity": 100, "initial": 50},
                "widget": {"capacity": 50, "initial": 0}
            },
            "accepts": ["raw"], "produces": ["widget"]
        }]"#,
        r#"[
            {"id": "raw", "kind": "i", "tags": []},
            {"id": "widget", "kind": "o", "tags": []}
        ]"#,
        r#"[{
            "id": "t", "type": "x", "runs_on": "p",
            "inputs": [{"thing": "raw", "amount": 1}],
            "outputs": [{"thing": "widget", "amount": 1}],
            "cadence_ticks": 5, "duration_ticks": 2, "deadline_ticks": 5,
            "failure_policy": "drop"
        }]"#,
    );
    let scene = load_scene_str(&json, 0).expect("scene loads");
    let static_payload = encode_static(&scene);
    assert_eq!(static_payload.sl1_transforms.len(), 1);
    assert_eq!(static_payload.sl1_transforms[0].id, "t");

    let mut snap = SnapshotPayload::default();
    encode_snapshot(&scene.world, &mut snap);
    assert_eq!(snap.sl1_transform_states.len(), 1);
    assert_eq!(snap.sl1_transform_states[0].transform_id, "t");
}

#[test]
fn transform_retry_then_warn_succeeds_on_attempt_2_after_contention() {
    // Two transforms share `m: 2` capacity bucket. Both fire at tick
    // 1000: "a" wins by stable id order; "b" Blocked. After "a"
    // completes and frees capacity, "b" breaches its original deadline
    // and transitions to Late. The advance_late path tries try_start
    // FIRST with a FRESH scheduled_at, so its retry gets a full
    // deadline budget and completes — producing 1 widget and emitting
    // ZERO Failed warnings.
    let json = scene_with(
        r#"[{
            "id": "p", "role": "x", "pos": [0,0],
            "capacity": {"m": 2},
            "storage": {
                "raw": {"capacity": 100, "initial": 100},
                "widget": {"capacity": 100, "initial": 0}
            },
            "accepts": ["raw"], "produces": ["widget"]
        }]"#,
        r#"[
            {"id": "raw", "kind": "i", "tags": []},
            {"id": "widget", "kind": "o", "tags": []}
        ]"#,
        r#"[
            {
                "id": "a", "type": "x", "runs_on": "p",
                "inputs": [{"thing": "raw", "amount": 1}],
                "outputs": [{"thing": "widget", "amount": 1}],
                "cadence_ticks": 1000, "duration_ticks": 5, "deadline_ticks": 5,
                "capacity_cost": {"m": 2},
                "failure_policy": "drop"
            },
            {
                "id": "b", "type": "x", "runs_on": "p",
                "inputs": [{"thing": "raw", "amount": 1}],
                "outputs": [{"thing": "widget", "amount": 1}],
                "cadence_ticks": 1000, "duration_ticks": 3, "deadline_ticks": 3,
                "capacity_cost": {"m": 2},
                "failure_policy": "retry_then_warn", "max_attempts": 5
            }
        ]"#,
    );
    let (_, world, messages) = load_and_tick(&json, 1100);
    let failed_b = count_warnings(&messages, "b", Sl1TransformWarningKind::Failed);
    assert_eq!(
        failed_b, 0,
        "expected zero Failed warnings for `b` after successful retry, got {failed_b}"
    );
    let runtime = world.sl1_runtime.as_ref().expect("runtime present");
    let widget = runtime
        .inventories
        .get("p")
        .and_then(|inv| inv.get("widget"))
        .copied()
        .unwrap_or(0);
    // Both `a` (first attempt) and `b` (retry attempt 2) should each
    // produce one widget = 2 total.
    assert_eq!(widget, 2, "expected 2 widgets produced total, got {widget}");
}

#[test]
fn transform_drop_does_not_retry_emits_one_failed_per_cadence_slot() {
    // Drop with starved inputs: each cadence slot produces exactly one
    // Failed warning (no spurious retries between slots).
    let json = scene_with(
        r#"[{
            "id": "p", "role": "x", "pos": [0,0],
            "capacity": {"m": 4},
            "storage": {
                "raw": {"capacity": 100, "initial": 0},
                "widget": {"capacity": 50, "initial": 0}
            },
            "accepts": ["raw"], "produces": ["widget"]
        }]"#,
        r#"[
            {"id": "raw", "kind": "i", "tags": []},
            {"id": "widget", "kind": "o", "tags": []}
        ]"#,
        r#"[{
            "id": "t", "type": "x", "runs_on": "p",
            "inputs": [{"thing": "raw", "amount": 1}],
            "outputs": [{"thing": "widget", "amount": 1}],
            "cadence_ticks": 10, "duration_ticks": 2, "deadline_ticks": 3,
            "failure_policy": "drop"
        }]"#,
    );
    // Cadence fires at ticks 10, 20, 30. Each fires → Starved → at tick
    // scheduled+3+1 = breach → Failed → Idle. So 3 Failed warnings.
    let (_, _, messages) = load_and_tick(&json, 35);
    let failed = count_warnings(&messages, "t", Sl1TransformWarningKind::Failed);
    assert_eq!(
        failed, 3,
        "drop policy should emit exactly one Failed per cadence slot, got {failed}"
    );
}

#[test]
fn transform_slot_missed_emitted_once_per_overlapping_cadence_tick() {
    // cadence=5, duration=20: transform runs from tick 5 through tick 25.
    // Cadence fires at ticks 10, 15, 20 while the transform is still
    // Running → 3 SlotMissed warnings. At tick 25 the transform's
    // completion runs first in `run()`, so the cadence at tick 25 finds
    // Idle and starts a new attempt (no SlotMissed at tick 25).
    let json = scene_with(
        r#"[{
            "id": "p", "role": "x", "pos": [0,0],
            "capacity": {"m": 4},
            "storage": {
                "raw": {"capacity": 100, "initial": 100},
                "widget": {"capacity": 100, "initial": 0}
            },
            "accepts": ["raw"], "produces": ["widget"]
        }]"#,
        r#"[
            {"id": "raw", "kind": "i", "tags": []},
            {"id": "widget", "kind": "o", "tags": []}
        ]"#,
        r#"[{
            "id": "t", "type": "x", "runs_on": "p",
            "inputs": [{"thing": "raw", "amount": 1}],
            "outputs": [{"thing": "widget", "amount": 1}],
            "cadence_ticks": 5, "duration_ticks": 20, "deadline_ticks": 20,
            "failure_policy": "drop"
        }]"#,
    );
    let (_, _, messages) = load_and_tick(&json, 25);
    let missed = count_warnings(&messages, "t", Sl1TransformWarningKind::SlotMissed);
    assert_eq!(
        missed, 3,
        "expected 3 SlotMissed warnings at ticks 10/15/20, got {missed}"
    );
}

#[test]
fn transform_late_started_does_not_complete_past_deadline() {
    // Codex review (PR #31): a delayed start can overshoot its
    // deadline when completion_tick lands on a tick where
    // `now > deadline_tick`. With contention, transform "b" gets
    // Blocked at tick 1000, starts at tick 1008 once "a" releases
    // capacity → completion_tick=1011, deadline_tick=1010. Without
    // the deadline-first reorder, OLD code completes at tick 1011
    // (since `now >= 1011` evaluates before `now > 1010`).
    // After the fix this must produce zero widgets for `b` and at
    // least one Late warning.
    let json = scene_with(
        r#"[{
            "id": "p", "role": "x", "pos": [0,0],
            "capacity": {"m": 2},
            "storage": {
                "raw": {"capacity": 200, "initial": 100},
                "widget": {"capacity": 100, "initial": 0}
            },
            "accepts": ["raw"], "produces": ["widget"]
        }]"#,
        r#"[
            {"id": "raw", "kind": "i", "tags": []},
            {"id": "widget", "kind": "o", "tags": []}
        ]"#,
        r#"[
            {
                "id": "a", "type": "x", "runs_on": "p",
                "inputs": [{"thing": "raw", "amount": 1}],
                "outputs": [{"thing": "widget", "amount": 1}],
                "cadence_ticks": 1000, "duration_ticks": 8, "deadline_ticks": 8,
                "capacity_cost": {"m": 2},
                "failure_policy": "drop"
            },
            {
                "id": "b", "type": "x", "runs_on": "p",
                "inputs": [{"thing": "raw", "amount": 1}],
                "outputs": [{"thing": "widget", "amount": 1}],
                "cadence_ticks": 1000, "duration_ticks": 3, "deadline_ticks": 10,
                "capacity_cost": {"m": 2},
                "failure_policy": "drop"
            }
        ]"#,
    );
    let (_, world, messages) = load_and_tick(&json, 1020);
    let late_b = count_warnings(&messages, "b", Sl1TransformWarningKind::Late);
    assert!(
        late_b >= 1,
        "expected ≥1 Late warning for `b` after deadline overshoot, got {late_b}"
    );
    let runtime = world.sl1_runtime.as_ref().expect("runtime present");
    // Only `a` should have produced. `b` started at tick 1008 with
    // completion=1013 > deadline=1010, so the deadline-first check
    // must fail it instead of producing a widget.
    let widget = runtime
        .inventories
        .get("p")
        .and_then(|inv| inv.get("widget"))
        .copied()
        .unwrap_or(0);
    assert_eq!(
        widget, 1,
        "only `a` should produce; got widget={widget} (deadline-vs-completion ordering bug returned?)"
    );
}

#[test]
fn transform_cadence_ticks_out_of_range_rejected() {
    let json = scene_with(
        default_places(),
        default_things(),
        &format!(
            r#"[{{
                "id": "t", "type": "x", "runs_on": "factory",
                "inputs": [{{"thing": "raw", "amount": 1}}],
                "outputs": [{{"thing": "widget", "amount": 1}}],
                "cadence_ticks": {over}, "duration_ticks": 1, "deadline_ticks": 1,
                "failure_policy": "drop"
            }}]"#,
            over = simetro_engine::MAX_TRANSFORM_TICKS + 1
        ),
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(
            err,
            Sl1LoadError::TransformTicksOutOfRange { field, .. } if field == "cadence_ticks"
        ),
        "got {err:?}"
    );
}

#[test]
fn transform_duration_ticks_out_of_range_rejected() {
    let json = scene_with(
        default_places(),
        default_things(),
        &format!(
            r#"[{{
                "id": "t", "type": "x", "runs_on": "factory",
                "inputs": [{{"thing": "raw", "amount": 1}}],
                "outputs": [{{"thing": "widget", "amount": 1}}],
                "cadence_ticks": 1, "duration_ticks": {over}, "deadline_ticks": 1,
                "failure_policy": "drop"
            }}]"#,
            over = simetro_engine::MAX_TRANSFORM_TICKS + 1
        ),
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(
            err,
            Sl1LoadError::TransformTicksOutOfRange { field, .. } if field == "duration_ticks"
        ),
        "got {err:?}"
    );
}

#[test]
fn transform_io_amount_out_of_range_rejected() {
    let json = scene_with(
        default_places(),
        default_things(),
        &format!(
            r#"[{{
                "id": "t", "type": "x", "runs_on": "factory",
                "inputs": [{{"thing": "raw", "amount": {over}}}],
                "outputs": [{{"thing": "widget", "amount": 1}}],
                "cadence_ticks": 1, "duration_ticks": 1, "deadline_ticks": 1,
                "failure_policy": "drop"
            }}]"#,
            over = simetro_engine::MAX_TRANSFORM_AMOUNT + 1
        ),
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::TransformIoAmountOutOfRange { .. }),
        "got {err:?}"
    );
}

#[test]
fn transform_capacity_cost_out_of_range_rejected() {
    let json = scene_with(
        default_places(),
        default_things(),
        &format!(
            r#"[{{
                "id": "t", "type": "x", "runs_on": "factory",
                "inputs": [{{"thing": "raw", "amount": 1}}],
                "outputs": [{{"thing": "widget", "amount": 1}}],
                "cadence_ticks": 1, "duration_ticks": 1, "deadline_ticks": 1,
                "capacity_cost": {{"machine_hours": {over}}},
                "failure_policy": "drop"
            }}]"#,
            over = simetro_engine::MAX_TRANSFORM_CAPACITY_COST + 1
        ),
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::TransformCapacityCostOutOfRange { .. }),
        "got {err:?}"
    );
}

#[test]
fn transform_max_attempts_out_of_range_rejected() {
    let json = scene_with(
        default_places(),
        default_things(),
        &format!(
            r#"[{{
                "id": "t", "type": "x", "runs_on": "factory",
                "inputs": [{{"thing": "raw", "amount": 1}}],
                "outputs": [{{"thing": "widget", "amount": 1}}],
                "cadence_ticks": 1, "duration_ticks": 1, "deadline_ticks": 1,
                "failure_policy": "retry_then_warn",
                "max_attempts": {over}
            }}]"#,
            over = simetro_engine::MAX_TRANSFORM_MAX_ATTEMPTS + 1
        ),
    );
    let err = expect_sl1_err(json);
    assert!(
        matches!(err, Sl1LoadError::TransformMaxAttemptsOutOfRange { .. }),
        "got {err:?}"
    );
}

// -------------------------------------------------------------------
// Fixture + deterministic hash baseline
// -------------------------------------------------------------------

#[test]
fn transforms_fixture_loads() {
    let scene = load_scene_str(TRANSFORMS_SCENE, 0).expect("transforms fixture loads");
    let sl1 = scene.world.sl1.as_ref().expect("sl1 present");
    assert_eq!(sl1.transforms.len(), 2);
    let static_payload = encode_static(&scene);
    assert_eq!(static_payload.sl1_transforms.len(), 2);
}

#[test]
fn transforms_fixture_ticks_deterministically_against_baseline() {
    let mut scene = load_scene_str(TRANSFORMS_SCENE, 0).expect("transforms fixture loads");
    let mut world = std::mem::take(&mut scene.world);
    let mut runner = TickRunner::new();
    let hash = hash_run(&mut world, &mut runner, TICKS);
    let baseline = TRANSFORMS_BASELINE.trim();
    assert_eq!(
        hash, baseline,
        "deterministic hash drift detected for sl1-transforms.json\n  baseline: {baseline}\n  current:  {hash}"
    );
}
