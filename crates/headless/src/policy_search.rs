//! Autoresearch-style policy-search runner (PR 13 of
//! `scenario_language_v1`).
//!
//! See `docs/superpowers/specs/2026-05-24-scenario_language_v1-plan.md`
//! (sections "Outcome metrics" and "policy-search mode") for the
//! design contract this module implements.
//!
//! Contract recap:
//!
//! * Scene mechanics (places, links, things, transforms, demand,
//!   pressure, objectives, failure/victory conditions, observability,
//!   milestones) and the seed/tick budget are FIXED across trials.
//! * Only the agent heuristic/policy artifact changes per trial.
//! * Each trial is one full deterministic run of the engine; the same
//!   `(scene, policy, seed, ticks)` tuple produces the same final
//!   state hash and the same `policy_score`.
//! * Comparison is lexicographic: terminal outcome class wins first
//!   (`Won` > `InProgress` > `Lost`); within the same class the
//!   numeric weighted score is the tie-breaker. A `Lost` candidate
//!   can never be `kept` over a non-`Lost` baseline.
//! * Outputs: one JSONL row per trial (`type: "trial"`) plus a single
//!   `type: "summary"` row at the end. Rows include
//!   `(trial_id, policy_name, status, score, baseline_score, delta,
//!    outcome, hash, ticks, seed)`.
//!
//! Failure surface:
//!
//! * `blocked` — the policy artifact failed to parse, applied to an
//!   unknown agent id, used an unsupported override key, or produced
//!   a scene that failed `load_scene_str` validation.
//! * `failed` — the trial run itself panicked while ticking (caught
//!   via `catch_unwind` as a last-resort isolation wrapper). The
//!   trial's mutated world is discarded.

use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use simetro_engine::scenario_language_v1::{GameOutcome, Sl1ObjectiveStatus};
use simetro_engine::{hash_run, load_scene_str, LoadError, TickRunner, World};

// ---------------------------------------------------------------------------
// Policy artifact.
// ---------------------------------------------------------------------------

/// Allowed override keys per agent. Any other key is rejected at apply
/// time with [`PolicyApplyError::UnsupportedOverrideKey`].
///
/// PR 13 intentionally keeps this list small. Future PRs may grow it,
/// but every addition must remain strictly within the "agent
/// heuristic" surface — never pressure, demand, observability, or
/// scene topology.
const ALLOWED_AGENT_OVERRIDE_KEYS: &[&str] = &[
    "interval_ticks",
    "cooldown_ticks",
    "max_cost_per_decision",
    "objective_weights",
    "allowed_actions",
];

/// A policy artifact loaded from disk. The artifact only overrides
/// per-agent heuristic fields; it cannot add or remove agents, change
/// pressure, demand, objectives, or any scene topology.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub overrides: PolicyOverrides,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct PolicyOverrides {
    /// Map of agent id → field override map. Field keys must come
    /// from [`ALLOWED_AGENT_OVERRIDE_KEYS`].
    #[serde(default)]
    pub agents: BTreeMap<String, BTreeMap<String, Value>>,
}

/// Errors that can occur while parsing or applying a policy artifact.
/// These map to [`TrialStatus::Blocked`] in JSONL output.
#[derive(Debug, thiserror::Error)]
pub enum PolicyApplyError {
    #[error("policy parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("policy I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("policy references unknown agent id {agent_id:?}")]
    UnknownAgent { agent_id: String },
    #[error(
        "policy override key {key:?} is not in the PR 13 allowlist for agent {agent_id:?} (allowed: {allowed:?})"
    )]
    UnsupportedOverrideKey {
        agent_id: String,
        key: String,
        allowed: Vec<&'static str>,
    },
    #[error("scene file has no scenario_language_v1.agents block to override")]
    NoAgentsBlock,
    #[error("scene file is not a JSON object")]
    SceneNotObject,
    #[error(
        "objective_weights value for objective {objective:?} on agent {agent_id:?} must be a number in [0, 1]"
    )]
    InvalidObjectiveWeight { agent_id: String, objective: String },
    #[error("override value for {key:?} on agent {agent_id:?} has wrong type")]
    InvalidOverrideValueType { agent_id: String, key: String },
}

/// Load a policy artifact from a JSON file on disk.
pub fn load_policy(path: &Path) -> Result<Policy, PolicyApplyError> {
    let text = std::fs::read_to_string(path)?;
    let parsed: Policy = serde_json::from_str(&text)?;
    Ok(parsed)
}

/// Apply a policy artifact to a scene JSON value. Returns a NEW
/// `Value` with the overrides spliced into
/// `scenario_language_v1.agents[*]` so the caller can re-feed it
/// through `load_scene_str` for full strict-schema re-validation.
///
/// The strict-schema validation in the loader catches schema-level
/// errors (bad enum values, out-of-range bounds, etc.); this function
/// catches policy-level errors (unknown agent ids, unsupported keys,
/// out-of-range `objective_weights` which are silently coerced by the
/// loader's `f64 → ratio` clamp and so must be checked here too).
pub fn apply_policy(scene: &Value, policy: &Policy) -> Result<Value, PolicyApplyError> {
    let mut scene = scene.clone();
    let root = scene
        .as_object_mut()
        .ok_or(PolicyApplyError::SceneNotObject)?;
    let sl1 = root
        .get_mut("scenario_language_v1")
        .and_then(Value::as_object_mut)
        .ok_or(PolicyApplyError::NoAgentsBlock)?;
    let agents = sl1
        .get_mut("agents")
        .and_then(Value::as_array_mut)
        .ok_or(PolicyApplyError::NoAgentsBlock)?;

    for (agent_id, fields) in &policy.overrides.agents {
        let agent_entry = agents
            .iter_mut()
            .find(|entry| {
                entry
                    .get("id")
                    .and_then(Value::as_str)
                    .map(|s| s == agent_id.as_str())
                    .unwrap_or(false)
            })
            .ok_or_else(|| PolicyApplyError::UnknownAgent {
                agent_id: agent_id.clone(),
            })?;
        let agent_obj = agent_entry.as_object_mut().ok_or_else(|| {
            PolicyApplyError::InvalidOverrideValueType {
                agent_id: agent_id.clone(),
                key: "<agent root>".into(),
            }
        })?;

        for (key, value) in fields {
            if !ALLOWED_AGENT_OVERRIDE_KEYS.contains(&key.as_str()) {
                return Err(PolicyApplyError::UnsupportedOverrideKey {
                    agent_id: agent_id.clone(),
                    key: key.clone(),
                    allowed: ALLOWED_AGENT_OVERRIDE_KEYS.to_vec(),
                });
            }
            apply_one_override(agent_id, agent_obj, key, value)?;
        }
    }

    Ok(scene)
}

fn apply_one_override(
    agent_id: &str,
    agent_obj: &mut Map<String, Value>,
    key: &str,
    value: &Value,
) -> Result<(), PolicyApplyError> {
    match key {
        "interval_ticks" => {
            if !value.is_u64() {
                return Err(PolicyApplyError::InvalidOverrideValueType {
                    agent_id: agent_id.into(),
                    key: key.into(),
                });
            }
            agent_obj.insert("interval_ticks".into(), value.clone());
        }
        "cooldown_ticks" | "max_cost_per_decision" => {
            if !value.is_u64() {
                return Err(PolicyApplyError::InvalidOverrideValueType {
                    agent_id: agent_id.into(),
                    key: key.into(),
                });
            }
            let budgets = agent_obj
                .entry("budgets".to_string())
                .or_insert_with(|| json!({}));
            let budgets = budgets.as_object_mut().ok_or_else(|| {
                PolicyApplyError::InvalidOverrideValueType {
                    agent_id: agent_id.into(),
                    key: "budgets".into(),
                }
            })?;
            budgets.insert(key.to_string(), value.clone());
        }
        "objective_weights" => {
            let map =
                value
                    .as_object()
                    .ok_or_else(|| PolicyApplyError::InvalidOverrideValueType {
                        agent_id: agent_id.into(),
                        key: key.into(),
                    })?;
            for (objective, weight) in map {
                let f =
                    weight
                        .as_f64()
                        .ok_or_else(|| PolicyApplyError::InvalidObjectiveWeight {
                            agent_id: agent_id.into(),
                            objective: objective.clone(),
                        })?;
                if !(0.0..=1.0).contains(&f) {
                    return Err(PolicyApplyError::InvalidObjectiveWeight {
                        agent_id: agent_id.into(),
                        objective: objective.clone(),
                    });
                }
            }
            agent_obj.insert("objective_weights".into(), value.clone());
        }
        "allowed_actions" => {
            if !value.is_array() {
                return Err(PolicyApplyError::InvalidOverrideValueType {
                    agent_id: agent_id.into(),
                    key: key.into(),
                });
            }
            agent_obj.insert("allowed_actions".into(), value.clone());
        }
        _ => unreachable!("guarded by ALLOWED_AGENT_OVERRIDE_KEYS check above"),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Trial outcome / score.
// ---------------------------------------------------------------------------

/// Lexicographic outcome class. `Won > InProgress > Lost`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeClass {
    Lost,
    InProgress,
    Won,
}

impl OutcomeClass {
    fn from_outcome(o: &GameOutcome) -> Self {
        match o {
            GameOutcome::InProgress => Self::InProgress,
            GameOutcome::Won => Self::Won,
            GameOutcome::Lost { .. } => Self::Lost,
        }
    }
}

/// A trial score combines an outcome class with a numeric subscore.
/// Comparison is lexicographic on `(class, weighted)` so a `Lost` run
/// can never beat a non-`Lost` run on raw points.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TrialScore {
    pub class: OutcomeClass,
    pub weighted: f64,
}

impl TrialScore {
    /// Strict betters-than comparison. Returns true if `self` strictly
    /// beats `other` under lexicographic `(class, weighted)`.
    pub fn beats(&self, other: &TrialScore) -> bool {
        match self.class.cmp(&other.class) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => self.weighted > other.weighted,
        }
    }
}

/// Compute the trusted policy score for a finished trial. Reads the
/// post-run `world.sl1` (scene) for declared weights/penalty/value and
/// the post-run `world.sl1_runtime` for actual counters/statuses.
///
/// The formula combines:
///
/// * **terminal outcome** (lexicographic primary key via [`OutcomeClass`]),
/// * **per-objective contribution:** `weight` if `Met`, `-weight` if
///   `Breached`, `0` otherwise,
/// * **fulfilled demand value:** `fulfilled_count * demand.value`,
/// * **dropped demand penalty:** `dropped_count * demand.penalty.score`,
/// * **failure conditions fired:** `-50` flat per fired condition
///   (declared weights for FCs do not exist in PR 8; this is a small
///   but non-zero signal so authors notice a fired FC even when no
///   objective is breached).
///
/// All arithmetic is in `f64`; the engine's per-tick state is already
/// deterministic so the score is deterministic too.
pub fn compute_policy_score(world: &World) -> TrialScore {
    let runtime = match world.sl1_runtime.as_ref() {
        Some(r) => r,
        None => {
            return TrialScore {
                class: OutcomeClass::InProgress,
                weighted: 0.0,
            }
        }
    };
    let scene = world.sl1.as_ref();

    let class = OutcomeClass::from_outcome(&runtime.game_outcome);
    let mut weighted = 0.0_f64;

    if let Some(scene) = scene {
        for obj in &scene.objectives {
            let status = runtime
                .objectives
                .get(&obj.id)
                .map(|s| s.status)
                .unwrap_or(Sl1ObjectiveStatus::Unknown);
            let w = f64::from(obj.weight);
            match status {
                Sl1ObjectiveStatus::Met => weighted += w,
                Sl1ObjectiveStatus::Breached => weighted -= w,
                Sl1ObjectiveStatus::Unknown | Sl1ObjectiveStatus::Unsupported => {}
            }
        }

        for demand in &scene.demand {
            if let Some(rt) = runtime.demand.get(&demand.id) {
                let value = demand.value as f64;
                weighted += (rt.fulfilled_count as f64) * value;
                let penalty = demand.penalty.score as f64;
                weighted -= (rt.dropped_count as f64) * penalty;
            }
        }
    }

    let fired_fcs = runtime
        .failure_conditions
        .values()
        .filter(|fc| fc.fired_at_tick.is_some())
        .count();
    weighted -= 50.0 * fired_fcs as f64;

    TrialScore { class, weighted }
}

// ---------------------------------------------------------------------------
// Trial runner.
// ---------------------------------------------------------------------------

/// Per-trial status emitted to JSONL output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrialStatus {
    Baseline,
    Kept,
    Discarded,
    Failed,
    Blocked,
}

impl TrialStatus {
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Kept => "kept",
            Self::Discarded => "discarded",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
        }
    }
}

/// One JSONL output row for a trial.
#[derive(Debug, Clone, Serialize)]
pub struct TrialRecord {
    #[serde(rename = "type")]
    pub record_type: &'static str,
    pub trial_id: u32,
    pub policy_name: String,
    pub status: TrialStatus,
    pub seed: u64,
    pub ticks: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<TrialScore>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_score: Option<TrialScore>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lost_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// JSONL summary row emitted after all trial rows.
#[derive(Debug, Clone, Serialize)]
pub struct SummaryRecord {
    #[serde(rename = "type")]
    pub record_type: &'static str,
    pub scene: String,
    pub seed: u64,
    pub ticks: u64,
    pub total_trials: u32,
    pub baseline_count: u32,
    pub kept_count: u32,
    pub discarded_count: u32,
    pub failed_count: u32,
    pub blocked_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_policy_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_score: Option<TrialScore>,
}

/// Successful trial output (post-run state).
#[derive(Debug, Clone)]
pub struct TrialRun {
    pub score: TrialScore,
    pub hash: String,
    pub outcome: GameOutcome,
}

#[derive(Debug, thiserror::Error)]
pub enum TrialRunError {
    #[error("scene reload failed: {0}")]
    SceneReload(LoadError),
    #[error("trial panicked during tick loop")]
    Panicked,
}

/// Run a single trial: re-load the (possibly policy-modified) scene
/// JSON, then call `hash_run` to advance the world `ticks` times.
/// `hash_run` both produces the deterministic SHA-256 hex of
/// `(initial_world, per-tick events+messages, final_world)` AND
/// leaves the world in its final state so we can read
/// `world.sl1_runtime` for scoring.
///
/// The `catch_unwind` wrapper is a last-resort isolation: typed
/// failures (invalid scene JSON, invalid policy) surface as
/// `Result::Err` and become `TrialStatus::Blocked` at the call site;
/// only an unexpected engine panic becomes `TrialStatus::Failed`.
pub fn run_trial(scene_json: &str, seed: u64, ticks: u64) -> Result<TrialRun, TrialRunError> {
    let loaded = load_scene_str(scene_json, seed).map_err(TrialRunError::SceneReload)?;
    let mut world = loaded.world;
    let mut runner = TickRunner::new();
    runner.reserve_for(world.movers.len());

    let result = catch_unwind(AssertUnwindSafe(|| {
        let hash = hash_run(&mut world, &mut runner, ticks);
        let score = compute_policy_score(&world);
        let outcome = world.sl1_outcome();
        (hash, score, outcome)
    }));

    match result {
        Ok((hash, score, outcome)) => Ok(TrialRun {
            score,
            hash,
            outcome,
        }),
        Err(_) => Err(TrialRunError::Panicked),
    }
}

fn outcome_label(o: &GameOutcome) -> (String, Option<String>) {
    match o {
        GameOutcome::InProgress => ("in_progress".into(), None),
        GameOutcome::Won => ("won".into(), None),
        GameOutcome::Lost { reason } => ("lost".into(), Some(reason.clone())),
    }
}

/// Orchestrate a baseline run plus N candidate trials. The baseline
/// is emitted with `status: "baseline"`; each candidate is either
/// `kept` (strictly beats the baseline), `discarded` (does not beat),
/// `blocked` (policy did not apply / scene did not load), or
/// `failed` (tick loop panicked).
///
/// Caller is responsible for producing the JSON for `scene_json` from
/// the original scene file. `baseline_policy` is treated as just
/// another policy artifact; passing an empty-overrides policy means
/// "the scene's authored agents".
#[allow(clippy::too_many_arguments)]
pub fn run_policy_search(
    scene_path: &Path,
    scene_json: &str,
    baseline_policy: &Policy,
    candidate_policies: &[Policy],
    seed: u64,
    ticks: u64,
) -> (Vec<TrialRecord>, SummaryRecord) {
    let mut records: Vec<TrialRecord> = Vec::with_capacity(1 + candidate_policies.len());
    let parsed_scene: Result<Value, _> = serde_json::from_str(scene_json);

    let baseline_record = build_record(
        0,
        baseline_policy,
        TrialStatus::Baseline,
        seed,
        ticks,
        &parsed_scene,
        None,
    );
    let baseline_score = baseline_record.score;
    records.push(baseline_record);

    let mut best_score = baseline_score;
    let mut best_name: Option<String> = baseline_score.map(|_| baseline_policy.name.clone());

    for (i, policy) in candidate_policies.iter().enumerate() {
        let trial_id = (i as u32) + 1;
        let mut rec = build_record(
            trial_id,
            policy,
            TrialStatus::Discarded,
            seed,
            ticks,
            &parsed_scene,
            baseline_score,
        );
        match (baseline_score, rec.score) {
            (Some(b), Some(cand)) => {
                rec.status = if cand.beats(&b) {
                    TrialStatus::Kept
                } else {
                    TrialStatus::Discarded
                };
                let update_best = match best_score {
                    Some(curr) => cand.beats(&curr),
                    None => true,
                };
                if update_best {
                    best_score = Some(cand);
                    best_name = Some(policy.name.clone());
                }
            }
            (None, _)
                if rec.status != TrialStatus::Failed && rec.status != TrialStatus::Blocked =>
            {
                // Baseline blocked/failed → candidates can't be compared.
                rec.status = TrialStatus::Blocked;
                rec.error =
                    Some("baseline did not produce a comparable score; candidate skipped".into());
                rec.score = None;
                rec.delta = None;
                rec.hash = None;
            }
            _ => {}
        }
        records.push(rec);
    }

    let mut baseline_count = 0;
    let mut kept_count = 0;
    let mut discarded_count = 0;
    let mut failed_count = 0;
    let mut blocked_count = 0;
    for r in &records {
        match r.status {
            TrialStatus::Baseline => baseline_count += 1,
            TrialStatus::Kept => kept_count += 1,
            TrialStatus::Discarded => discarded_count += 1,
            TrialStatus::Failed => failed_count += 1,
            TrialStatus::Blocked => blocked_count += 1,
        }
    }

    let summary = SummaryRecord {
        record_type: "summary",
        scene: scene_path.display().to_string(),
        seed,
        ticks,
        total_trials: records.len() as u32,
        baseline_count,
        kept_count,
        discarded_count,
        failed_count,
        blocked_count,
        best_policy_name: best_name,
        best_score,
    };

    (records, summary)
}

fn build_record(
    trial_id: u32,
    policy: &Policy,
    initial_status: TrialStatus,
    seed: u64,
    ticks: u64,
    parsed_scene: &Result<Value, serde_json::Error>,
    baseline_score: Option<TrialScore>,
) -> TrialRecord {
    let mut rec = TrialRecord {
        record_type: "trial",
        trial_id,
        policy_name: policy.name.clone(),
        status: initial_status,
        seed,
        ticks,
        score: None,
        baseline_score,
        delta: None,
        outcome: None,
        lost_reason: None,
        hash: None,
        error: None,
    };

    let scene_value = match parsed_scene {
        Ok(v) => v,
        Err(e) => {
            rec.status = TrialStatus::Blocked;
            rec.error = Some(format!("scene file is not valid JSON: {e}"));
            return rec;
        }
    };

    let mutated = match apply_policy(scene_value, policy) {
        Ok(v) => v,
        Err(e) => {
            rec.status = TrialStatus::Blocked;
            rec.error = Some(format!("{e}"));
            return rec;
        }
    };
    let mutated_str = match serde_json::to_string(&mutated) {
        Ok(s) => s,
        Err(e) => {
            rec.status = TrialStatus::Blocked;
            rec.error = Some(format!("policy-overlaid scene failed to serialize: {e}"));
            return rec;
        }
    };

    match run_trial(&mutated_str, seed, ticks) {
        Ok(run) => {
            let (label, lost_reason) = outcome_label(&run.outcome);
            rec.outcome = Some(label);
            rec.lost_reason = lost_reason;
            rec.hash = Some(run.hash);
            rec.score = Some(run.score);
            rec.delta = match baseline_score {
                Some(b) => Some(run.score.weighted - b.weighted),
                None => None,
            };
        }
        Err(TrialRunError::SceneReload(e)) => {
            rec.status = TrialStatus::Blocked;
            rec.error = Some(format!("scene reload failed after policy applied: {e}"));
        }
        Err(TrialRunError::Panicked) => {
            rec.status = TrialStatus::Failed;
            rec.error = Some("trial panicked during tick loop".into());
        }
    }

    rec
}

// ---------------------------------------------------------------------------
// JSONL emit.
// ---------------------------------------------------------------------------

/// Emit one trial record per line plus a summary line. Each record is
/// terminated with `\n` so a JSONL consumer can parse line-by-line
/// without needing to know whether the writer flushed.
pub fn emit_jsonl<W: std::io::Write>(
    out: &mut W,
    records: &[TrialRecord],
    summary: &SummaryRecord,
) -> std::io::Result<()> {
    for rec in records {
        serde_json::to_writer(&mut *out, rec)?;
        writeln!(out)?;
    }
    serde_json::to_writer(&mut *out, summary)?;
    writeln!(out)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// CLI command implementation.
// ---------------------------------------------------------------------------

/// CLI invocation entry point. Returns process exit code.
///
/// * `0` — all trials produced a comparable score (any number may be
///   `discarded`).
/// * `2` — at least one trial was `blocked` (policy artifact error).
/// * `3` — at least one trial was `failed` (tick loop panic).
/// * `4` — scene file not readable / scene file unparseable.
///
/// Output is always written to `out` (stdout if `out` is `-`).
pub fn cmd_policy_search(
    scene: &Path,
    baseline_policy_path: Option<&Path>,
    candidate_paths: &[PathBuf],
    seed: u64,
    ticks: u64,
    out: &Path,
) -> i32 {
    let scene_json = match std::fs::read_to_string(scene) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "policy-search: failed to read scene {scene}: {e}",
                scene = scene.display()
            );
            return 4;
        }
    };

    // Preflight: parse + strict-load the scene before emitting any
    // JSONL. A malformed scene file is a process-level IO/config
    // problem (exit code 4), not a per-trial policy-artifact problem
    // (which would be exit code 2). Catching it here keeps the
    // documented exit-code contract honest.
    if let Err(e) = serde_json::from_str::<Value>(&scene_json) {
        eprintln!(
            "policy-search: scene {scene} is not valid JSON: {e}",
            scene = scene.display()
        );
        return 4;
    }
    if let Err(e) = load_scene_str(&scene_json, seed) {
        eprintln!(
            "policy-search: scene {scene} failed to load: {e}",
            scene = scene.display()
        );
        return 4;
    }

    let baseline_policy = match baseline_policy_path {
        Some(p) => match load_policy(p) {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "policy-search: failed to load baseline policy {}: {e}",
                    p.display()
                );
                return 2;
            }
        },
        None => Policy {
            name: "baseline".into(),
            description: "implicit baseline (no overrides)".into(),
            overrides: PolicyOverrides::default(),
        },
    };

    let mut candidates = Vec::with_capacity(candidate_paths.len());
    for path in candidate_paths {
        match load_policy(path) {
            Ok(p) => candidates.push(p),
            Err(e) => {
                eprintln!(
                    "policy-search: failed to load candidate policy {}: {e}",
                    path.display()
                );
                return 2;
            }
        }
    }

    let (records, summary) = run_policy_search(
        scene,
        &scene_json,
        &baseline_policy,
        &candidates,
        seed,
        ticks,
    );

    let write_to: Box<dyn std::io::Write> = if out == Path::new("-") {
        Box::new(std::io::stdout().lock())
    } else {
        match std::fs::File::create(out) {
            Ok(f) => Box::new(std::io::BufWriter::new(f)),
            Err(e) => {
                eprintln!(
                    "policy-search: failed to open output {}: {e}",
                    out.display()
                );
                return 4;
            }
        }
    };
    let mut write_to = write_to;
    if let Err(e) = emit_jsonl(&mut write_to, &records, &summary) {
        eprintln!("policy-search: failed to write JSONL output: {e}");
        return 4;
    }

    let mut exit = 0;
    for r in &records {
        match r.status {
            TrialStatus::Failed => exit = exit.max(3),
            TrialStatus::Blocked => exit = exit.max(2),
            _ => {}
        }
    }
    exit
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn empty_policy(name: &str) -> Policy {
        Policy {
            name: name.into(),
            description: String::new(),
            overrides: PolicyOverrides::default(),
        }
    }

    #[test]
    fn outcome_class_lexicographic_ordering() {
        assert!(OutcomeClass::Won > OutcomeClass::InProgress);
        assert!(OutcomeClass::InProgress > OutcomeClass::Lost);
    }

    #[test]
    fn trial_score_beats_uses_outcome_class_first() {
        let lost_high = TrialScore {
            class: OutcomeClass::Lost,
            weighted: 1_000_000.0,
        };
        let in_prog_low = TrialScore {
            class: OutcomeClass::InProgress,
            weighted: -100.0,
        };
        let won_zero = TrialScore {
            class: OutcomeClass::Won,
            weighted: 0.0,
        };
        assert!(in_prog_low.beats(&lost_high));
        assert!(won_zero.beats(&in_prog_low));
        assert!(!lost_high.beats(&in_prog_low));
    }

    #[test]
    fn trial_score_beats_uses_weighted_within_same_class() {
        let a = TrialScore {
            class: OutcomeClass::InProgress,
            weighted: 10.0,
        };
        let b = TrialScore {
            class: OutcomeClass::InProgress,
            weighted: 5.0,
        };
        assert!(a.beats(&b));
        assert!(!b.beats(&a));
        assert!(!a.beats(&a));
    }

    #[test]
    fn policy_parses_with_only_required_fields() {
        let json = r#"{ "name": "p", "overrides": { } }"#;
        let p: Policy = serde_json::from_str(json).unwrap();
        assert_eq!(p.name, "p");
        assert!(p.overrides.agents.is_empty());
    }

    #[test]
    fn policy_rejects_unknown_top_level_field() {
        let json = r#"{ "name": "p", "overrides": { }, "extra": 1 }"#;
        let err = serde_json::from_str::<Policy>(json).unwrap_err();
        assert!(err.to_string().contains("extra"), "got: {err}");
    }

    #[test]
    fn apply_policy_rejects_unknown_agent_id() {
        let scene = json!({
            "scenario_language_v1": { "agents": [ { "id": "real-agent" } ] }
        });
        let mut overrides = PolicyOverrides::default();
        overrides.agents.insert(
            "ghost".into(),
            [("interval_ticks".to_string(), json!(10u64))]
                .into_iter()
                .collect(),
        );
        let policy = Policy {
            name: "p".into(),
            description: String::new(),
            overrides,
        };
        let err = apply_policy(&scene, &policy).unwrap_err();
        assert!(
            matches!(err, PolicyApplyError::UnknownAgent { ref agent_id } if agent_id == "ghost")
        );
    }

    #[test]
    fn apply_policy_rejects_unsupported_key() {
        let scene = json!({
            "scenario_language_v1": { "agents": [ { "id": "a" } ] }
        });
        let mut overrides = PolicyOverrides::default();
        overrides.agents.insert(
            "a".into(),
            [("kind".to_string(), json!("llm"))].into_iter().collect(),
        );
        let policy = Policy {
            name: "p".into(),
            description: String::new(),
            overrides,
        };
        let err = apply_policy(&scene, &policy).unwrap_err();
        assert!(
            matches!(err, PolicyApplyError::UnsupportedOverrideKey { ref key, .. } if key == "kind")
        );
    }

    #[test]
    fn apply_policy_rejects_objective_weight_out_of_range() {
        let scene = json!({
            "scenario_language_v1": { "agents": [ { "id": "a" } ] }
        });
        let mut overrides = PolicyOverrides::default();
        overrides.agents.insert(
            "a".into(),
            [("objective_weights".to_string(), json!({ "o1": 2.0 }))]
                .into_iter()
                .collect(),
        );
        let policy = Policy {
            name: "p".into(),
            description: String::new(),
            overrides,
        };
        let err = apply_policy(&scene, &policy).unwrap_err();
        assert!(matches!(
            err,
            PolicyApplyError::InvalidObjectiveWeight { ref objective, .. } if objective == "o1"
        ));
    }

    #[test]
    fn apply_policy_rejects_wrong_type_for_interval_ticks() {
        let scene = json!({
            "scenario_language_v1": { "agents": [ { "id": "a" } ] }
        });
        let mut overrides = PolicyOverrides::default();
        overrides.agents.insert(
            "a".into(),
            [("interval_ticks".to_string(), json!("not a number"))]
                .into_iter()
                .collect(),
        );
        let policy = Policy {
            name: "p".into(),
            description: String::new(),
            overrides,
        };
        let err = apply_policy(&scene, &policy).unwrap_err();
        assert!(matches!(
            err,
            PolicyApplyError::InvalidOverrideValueType { ref key, .. } if key == "interval_ticks"
        ));
    }

    #[test]
    fn apply_policy_sets_nested_budgets_cooldown() {
        let scene = json!({
            "scenario_language_v1": {
                "agents": [ { "id": "a", "budgets": { "max_cost_per_decision": 1 } } ]
            }
        });
        let mut overrides = PolicyOverrides::default();
        overrides.agents.insert(
            "a".into(),
            [("cooldown_ticks".to_string(), json!(120u64))]
                .into_iter()
                .collect(),
        );
        let policy = Policy {
            name: "p".into(),
            description: String::new(),
            overrides,
        };
        let mutated = apply_policy(&scene, &policy).unwrap();
        let v = mutated.pointer("/scenario_language_v1/agents/0/budgets/cooldown_ticks");
        assert_eq!(v, Some(&json!(120u64)));
        // existing field preserved
        let kept = mutated.pointer("/scenario_language_v1/agents/0/budgets/max_cost_per_decision");
        assert_eq!(kept, Some(&json!(1)));
    }

    #[test]
    fn apply_policy_replaces_objective_weights_map() {
        let scene = json!({
            "scenario_language_v1": {
                "agents": [ { "id": "a", "objective_weights": { "x": 0.5 } } ]
            }
        });
        let mut overrides = PolicyOverrides::default();
        overrides.agents.insert(
            "a".into(),
            [(
                "objective_weights".to_string(),
                json!({ "x": 1.0, "y": 0.25 }),
            )]
            .into_iter()
            .collect(),
        );
        let policy = Policy {
            name: "p".into(),
            description: String::new(),
            overrides,
        };
        let mutated = apply_policy(&scene, &policy).unwrap();
        let v = mutated.pointer("/scenario_language_v1/agents/0/objective_weights");
        assert_eq!(v, Some(&json!({ "x": 1.0, "y": 0.25 })));
    }

    #[test]
    fn apply_policy_empty_overrides_returns_equivalent_scene() {
        let scene = json!({
            "scenario_language_v1": { "agents": [ { "id": "a" } ] }
        });
        let policy = empty_policy("baseline");
        let mutated = apply_policy(&scene, &policy).unwrap();
        assert_eq!(scene, mutated);
    }

    #[test]
    fn jsonl_emits_trial_and_summary_with_type_discriminator() {
        let mut buf: Vec<u8> = Vec::new();
        let records = vec![TrialRecord {
            record_type: "trial",
            trial_id: 0,
            policy_name: "p".into(),
            status: TrialStatus::Baseline,
            seed: 1,
            ticks: 10,
            score: Some(TrialScore {
                class: OutcomeClass::InProgress,
                weighted: 1.5,
            }),
            baseline_score: None,
            delta: None,
            outcome: Some("in_progress".into()),
            lost_reason: None,
            hash: Some("deadbeef".into()),
            error: None,
        }];
        let summary = SummaryRecord {
            record_type: "summary",
            scene: "scene.json".into(),
            seed: 1,
            ticks: 10,
            total_trials: 1,
            baseline_count: 1,
            kept_count: 0,
            discarded_count: 0,
            failed_count: 0,
            blocked_count: 0,
            best_policy_name: Some("p".into()),
            best_score: Some(TrialScore {
                class: OutcomeClass::InProgress,
                weighted: 1.5,
            }),
        };
        emit_jsonl(&mut buf, &records, &summary).unwrap();
        let text = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = text.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);
        let trial: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(trial["type"], "trial");
        assert_eq!(trial["status"], "baseline");
        let sum: Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(sum["type"], "summary");
        assert_eq!(sum["baseline_count"], 1);
    }
}
