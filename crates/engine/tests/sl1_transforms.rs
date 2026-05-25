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
