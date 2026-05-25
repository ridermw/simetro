//! JSON scene loader.
//!
//! ```text
//!     json (&str) ──▶ serde_json ──▶ RawScene ──▶ validate ──▶ LoadedScene
//!                       │                            │
//!                       │ Parse{line,col}            │ LoadError::*
//!                       ▼                            ▼
//!                  LoadError::Parse           field-typed errors
//! ```
//!
//! Loader is split in two halves: deserialization (serde) and validation
//! (this file). Validation enforces every row in scene loader contract. Each field's
//! violation maps to a specific [`LoadError`] variant so the canvas
//! overlay can point at the source location.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer};

use crate::components::{
    Consumer, ConsumerId, Mover, MoverId, Node, NodeId, NodeShape, Path, PathId, Producer,
    ProducerId, Resource, ResourceId,
};
use crate::error::LoadError;
use crate::scenario_language_v1::{self, Sl1Scene};
use crate::world::World;

const MIN_SCHEMA_VERSION: u32 = 1;
const SUPPORTED_SCHEMA_VERSION: u32 = 2;
const MAX_NAME_LEN: usize = 200;
const MAX_PALETTE_ENTRIES: usize = 32;
const MAX_PIECES_PER_SECTION: usize = 100_000;
const MAX_ID_LEN: usize = 64;
const COORD_LIMIT: f32 = 1.0e6;
const SPEED_MIN: f32 = 0.0;
const SPEED_MAX: f32 = 100.0;
const INTERVAL_MIN: u32 = 1;
const INTERVAL_MAX: u32 = 10_000;
const AMOUNT_MIN: u64 = 1;
const AMOUNT_MAX: u64 = 1_000_000;

/// Specification for an agent the engine should instantiate at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSpec {
    pub kind: String,
    pub interval_ticks: u32,
    /// Per lifecycle invariant: ticks the bridge has to reply before an LLM
    /// request is expired and (possibly) re-issued. Only meaningful
    /// for `kind: "llm"`; ignored otherwise. Default 60 (≈1 s @ 60 Hz)
    /// applied when the JSON omits it.
    pub deadline_ticks: u32,
}

/// Win/end condition for legacy scenes. `scenario_language_v1` will add
/// explicit objectives, failure conditions, and `GameOutcome`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goal {
    LoopForever,
}

/// Theme metadata pulled from the JSON. Carried by `StaticPayload` to
/// the renderer.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub palette: Vec<String>,
    pub background_index: u8,
    pub font: String,
}

/// Bidirectional mapping between JSON string ids and runtime numeric ids.
/// Numeric ids are dense indices `0..n` per section.
#[derive(Debug, Default, Clone)]
pub struct IdMap {
    pub nodes_by_name: BTreeMap<String, NodeId>,
    pub paths_by_name: BTreeMap<String, PathId>,
    pub movers_by_name: BTreeMap<String, MoverId>,
    pub resources_by_name: BTreeMap<String, ResourceId>,
    pub producers_by_name: BTreeMap<String, ProducerId>,
    pub consumers_by_name: BTreeMap<String, ConsumerId>,
    pub node_names: BTreeMap<NodeId, String>,
    pub path_names: BTreeMap<PathId, String>,
    pub mover_names: BTreeMap<MoverId, String>,
    pub resource_names: BTreeMap<ResourceId, String>,
    pub producer_names: BTreeMap<ProducerId, String>,
    pub consumer_names: BTreeMap<ConsumerId, String>,
}

/// Successful result of loading a scene.
#[derive(Debug)]
pub struct LoadedScene {
    pub name: String,
    pub theme: Theme,
    pub goals: Vec<Goal>,
    pub agents: Vec<AgentSpec>,
    pub id_map: IdMap,
    pub world: World,
    /// Validated `scenario_language_v1` block, if the scene JSON
    /// included one. `None` for legacy v1/v2 scenes.
    pub sl1: Option<Sl1Scene>,
}

// ---------------------------------------------------------------------------
// Raw (post-serde, pre-validation) shape.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawScene {
    schema_version: u32,
    name: String,
    #[serde(default)]
    theme: Option<RawTheme>,
    pieces: RawPieces,
    #[serde(default)]
    goals: Vec<RawGoal>,
    #[serde(default)]
    agents: Vec<RawAgent>,
    #[serde(default)]
    resources: Vec<RawResource>,
    #[serde(default)]
    inventory: Vec<RawInventory>,
    #[serde(default)]
    producers: Vec<RawProducer>,
    #[serde(default)]
    consumers: Vec<RawConsumer>,
    /// Optional `scenario_language_v1` block. Sibling of `pieces` so
    /// legacy v1/v2 scenes are unaffected. Captured as a raw JSON
    /// `Value` here so that strict-schema deserialization runs through
    /// `scenario_language_v1::load_value` in validate(), producing
    /// typed `LoadError::Sl1` errors (e.g. `UnknownField`) instead of
    /// being swallowed into the outer scene parse error.
    ///
    /// `deserialize_with = "deserialize_some"` is used so we can
    /// distinguish a key that is absent (None) from a key that is
    /// explicitly `null` (Some(Value::Null)) — the latter must be
    /// rejected so a scene cannot bypass SL1 validation by writing
    /// `"scenario_language_v1": null`.
    #[serde(
        default,
        rename = "scenario_language_v1",
        deserialize_with = "deserialize_some"
    )]
    sl1: Option<serde_json::Value>,
    /// Any top-level key not matched by a named field lands here.
    /// We use this to reject SL1 grammar primitive names that were
    /// authored at the scene's top level rather than inside the
    /// `scenario_language_v1` block — a common error mode that would
    /// otherwise silently no-op. Legacy unknown keys (anything not in
    /// the reserved list) remain accepted to preserve v1/v2 behavior.
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct RawTheme {
    palette: Vec<String>,
    #[serde(default)]
    background_index: u8,
    #[serde(default = "default_font")]
    font: String,
}

fn default_font() -> String {
    "system-ui".to_string()
}

/// `#[serde(deserialize_with = "deserialize_some")]` helper.
///
/// Lets a field distinguish "key absent" (resolved by `#[serde(default)]`
/// to `None`) from "key explicitly null" (becomes `Some(T)` where `T`
/// is `serde_json::Value::Null`). The standard `Option<T>` deserializer
/// collapses both into `None`. We need the distinction so explicit
/// `"scenario_language_v1": null` can be rejected.
fn deserialize_some<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize)]
struct RawPieces {
    #[serde(default)]
    nodes: Vec<RawNode>,
    #[serde(default)]
    paths: Vec<RawPath>,
    #[serde(default)]
    movers: Vec<RawMover>,
}

#[derive(Debug, Deserialize)]
struct RawNode {
    id: String,
    pos: [f32; 2],
    shape: NodeShape,
    color: u8,
}

#[derive(Debug, Deserialize)]
struct RawPath {
    id: String,
    from: String,
    to: String,
    color: u8,
}

#[derive(Debug, Deserialize)]
struct RawMover {
    id: String,
    on_path: String,
    speed: f32,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RawGoal {
    LoopForever,
}

#[derive(Debug, Deserialize)]
struct RawAgent {
    kind: String,
    #[serde(default)]
    interval_ticks: u32,
    /// Optional; defaults to [`DEFAULT_LLM_DEADLINE_TICKS`] when
    /// loading an `llm` agent that doesn't specify it.
    #[serde(default)]
    deadline_ticks: Option<u32>,
}

/// Default per-agent deadline (in ticks) for `kind: "llm"` agents
/// when the scene JSON does not specify one. ≈1 second at 60 Hz.
pub const DEFAULT_LLM_DEADLINE_TICKS: u32 = 60;

/// Agent kinds the loader recognizes today. `"llm"` is gated behind
/// the `llm-live` Cargo feature.
const KNOWN_AGENT_KINDS: &[&str] = &["speed_tuner", "llm"];

/// Agent kinds that require the `llm-live` feature.
const FEATURE_GATED_AGENT_KINDS: &[&str] = &["llm"];

#[derive(Debug, Deserialize)]
struct RawResource {
    id: String,
    #[serde(default)]
    color: u8,
}

#[derive(Debug, Deserialize)]
struct RawInventory {
    resource: String,
    amount: u64,
}

#[derive(Debug, Deserialize)]
struct RawProducer {
    id: String,
    resource: String,
    amount: u64,
    interval_ticks: u32,
}

#[derive(Debug, Deserialize)]
struct RawConsumer {
    id: String,
    resource: String,
    amount: u64,
    interval_ticks: u32,
}

// ---------------------------------------------------------------------------
// Public entry point.
// ---------------------------------------------------------------------------

/// Parse + validate a scene from a JSON string.
///
/// `seed` is the deterministic RNG seed; it does not come from the JSON
/// (the scene is the puzzle; the seed is the run).
///
/// # Errors
/// Returns a [`LoadError`] for any malformed or out-of-bounds field.
/// Every error in [`LoadError`] is reachable from this function.
pub fn load_scene_str(json: &str, seed: u64) -> Result<LoadedScene, LoadError> {
    let raw: RawScene = serde_json::from_str(json).map_err(|e| LoadError::Parse {
        line: u32::try_from(e.line()).unwrap_or(0),
        col: u32::try_from(e.column()).unwrap_or(0),
        message: e.to_string(),
    })?;
    validate(raw, seed)
}

// ---------------------------------------------------------------------------
// Validation.
// ---------------------------------------------------------------------------

fn validate(raw: RawScene, seed: u64) -> Result<LoadedScene, LoadError> {
    if raw.schema_version < MIN_SCHEMA_VERSION || raw.schema_version > SUPPORTED_SCHEMA_VERSION {
        return Err(LoadError::UnsupportedVersion {
            found: raw.schema_version,
            supported: SUPPORTED_SCHEMA_VERSION,
        });
    }

    // Reject `scenario_language_v1` grammar primitives that were
    // mistakenly placed at the scene's top level rather than inside
    // the `scenario_language_v1` block. Without this guard the SL1
    // section would silently no-op.
    //
    // `agents` is intentionally NOT in this list — it is a legacy
    // top-level key in existing scenes and clashes with SL1's nested
    // `agents`. PR 10 resolves the ambiguity.
    const SL1_RESERVED_TOP_LEVEL_KEYS: &[&str] = &[
        "places",
        "links",
        "things",
        "transforms",
        "demand",
        "pressure",
        "objectives",
        "failure_conditions",
        "observability",
        "milestones",
    ];
    for &reserved in SL1_RESERVED_TOP_LEVEL_KEYS {
        if raw.extra.contains_key(reserved) {
            return Err(LoadError::Sl1ReservedKeyAtTopLevel { name: reserved });
        }
    }

    validate_name(&raw.name)?;

    let theme = validate_theme(raw.theme)?;

    // Section size caps.
    check_section_cap("nodes", raw.pieces.nodes.len())?;
    check_section_cap("paths", raw.pieces.paths.len())?;
    check_section_cap("movers", raw.pieces.movers.len())?;
    check_section_cap("resources", raw.resources.len())?;
    check_section_cap("inventory", raw.inventory.len())?;
    check_section_cap("producers", raw.producers.len())?;
    check_section_cap("consumers", raw.consumers.len())?;

    // Build the id map by interning string ids to dense numeric ids.
    let mut id_map = IdMap::default();
    let mut nodes = BTreeMap::new();
    let mut paths = BTreeMap::new();
    let mut movers = BTreeMap::new();
    let mut resources = BTreeMap::new();
    let mut inventory = BTreeMap::new();
    let mut producers = BTreeMap::new();
    let mut consumers = BTreeMap::new();

    // Nodes first.
    for (i, n) in raw.pieces.nodes.into_iter().enumerate() {
        validate_id("nodes", &n.id)?;
        if id_map.nodes_by_name.contains_key(&n.id) {
            return Err(LoadError::DuplicateId {
                section: "nodes",
                id: n.id,
            });
        }
        validate_coord(&n.id, n.pos)?;
        if (n.color as usize) >= theme.palette.len() {
            return Err(LoadError::PaletteIndexOOB {
                field: format!("pieces.nodes[{i}].color"),
                index: n.color as usize,
                max: theme.palette.len(),
            });
        }
        let nid = NodeId(u32::try_from(i).unwrap_or(u32::MAX));
        id_map.nodes_by_name.insert(n.id.clone(), nid);
        id_map.node_names.insert(nid, n.id);
        nodes.insert(
            nid,
            Node {
                id: nid,
                pos: n.pos,
                shape: n.shape,
                color: n.color,
            },
        );
    }

    // Paths next; they reference nodes.
    for (i, p) in raw.pieces.paths.into_iter().enumerate() {
        validate_id("paths", &p.id)?;
        if id_map.paths_by_name.contains_key(&p.id) {
            return Err(LoadError::DuplicateId {
                section: "paths",
                id: p.id,
            });
        }
        let from = id_map.nodes_by_name.get(&p.from).copied().ok_or_else(|| {
            LoadError::UnknownReference {
                from: format!("pieces.paths[{i}].from"),
                to: p.from.clone(),
            }
        })?;
        let to = id_map.nodes_by_name.get(&p.to).copied().ok_or_else(|| {
            LoadError::UnknownReference {
                from: format!("pieces.paths[{i}].to"),
                to: p.to.clone(),
            }
        })?;
        if (p.color as usize) >= theme.palette.len() {
            return Err(LoadError::PaletteIndexOOB {
                field: format!("pieces.paths[{i}].color"),
                index: p.color as usize,
                max: theme.palette.len(),
            });
        }
        let pid = PathId(u32::try_from(i).unwrap_or(u32::MAX));
        id_map.paths_by_name.insert(p.id.clone(), pid);
        id_map.path_names.insert(pid, p.id);
        paths.insert(
            pid,
            Path {
                id: pid,
                from,
                to,
                color: p.color,
            },
        );
    }

    // Movers last; reference paths.
    for (i, m) in raw.pieces.movers.into_iter().enumerate() {
        validate_id("movers", &m.id)?;
        if id_map.movers_by_name.contains_key(&m.id) {
            return Err(LoadError::DuplicateId {
                section: "movers",
                id: m.id,
            });
        }
        if !m.speed.is_finite() || m.speed < SPEED_MIN || m.speed > SPEED_MAX {
            return Err(LoadError::SpeedOutOfRange {
                id: m.id,
                value: m.speed,
            });
        }
        let on_path = id_map
            .paths_by_name
            .get(&m.on_path)
            .copied()
            .ok_or_else(|| LoadError::UnknownReference {
                from: format!("pieces.movers[{i}].on_path"),
                to: m.on_path.clone(),
            })?;
        let mid = MoverId(u32::try_from(i).unwrap_or(u32::MAX));
        id_map.movers_by_name.insert(m.id.clone(), mid);
        id_map.mover_names.insert(mid, m.id);
        movers.insert(mid, Mover::new(mid, on_path, m.speed));
    }

    // Resources and global inventory are schema v2 additions. Schema v1
    // scenes auto-upgrade by leaving these arrays empty.
    for (i, r) in raw.resources.into_iter().enumerate() {
        validate_id("resources", &r.id)?;
        if id_map.resources_by_name.contains_key(&r.id) {
            return Err(LoadError::DuplicateId {
                section: "resources",
                id: r.id,
            });
        }
        if (r.color as usize) >= theme.palette.len() {
            return Err(LoadError::PaletteIndexOOB {
                field: format!("resources[{i}].color"),
                index: r.color as usize,
                max: theme.palette.len(),
            });
        }
        let rid = ResourceId(u32::try_from(i).unwrap_or(u32::MAX));
        id_map.resources_by_name.insert(r.id.clone(), rid);
        id_map.resource_names.insert(rid, r.id.clone());
        resources.insert(
            rid,
            Resource {
                id: rid,
                name: r.id,
                color: r.color,
            },
        );
        inventory.insert(rid, 0);
    }

    let mut inventory_seen = BTreeMap::<ResourceId, ()>::new();
    for inv in raw.inventory {
        validate_inventory_amount("inventory.amount", inv.amount)?;
        let resource = id_map
            .resources_by_name
            .get(&inv.resource)
            .copied()
            .ok_or_else(|| LoadError::UnknownReference {
                from: "inventory[].resource".to_string(),
                to: inv.resource.clone(),
            })?;
        if inventory_seen.insert(resource, ()).is_some() {
            return Err(LoadError::DuplicateId {
                section: "inventory",
                id: inv.resource,
            });
        }
        inventory.insert(resource, inv.amount);
    }

    for (i, p) in raw.producers.into_iter().enumerate() {
        validate_id("producers", &p.id)?;
        if id_map.producers_by_name.contains_key(&p.id) {
            return Err(LoadError::DuplicateId {
                section: "producers",
                id: p.id,
            });
        }
        validate_amount("producers[].amount", p.amount)?;
        validate_interval("producers", i, p.interval_ticks)?;
        let resource = id_map
            .resources_by_name
            .get(&p.resource)
            .copied()
            .ok_or_else(|| LoadError::UnknownReference {
                from: format!("producers[{i}].resource"),
                to: p.resource.clone(),
            })?;
        let pid = ProducerId(u32::try_from(i).unwrap_or(u32::MAX));
        id_map.producers_by_name.insert(p.id.clone(), pid);
        id_map.producer_names.insert(pid, p.id);
        producers.insert(
            pid,
            Producer {
                id: pid,
                resource,
                amount: p.amount,
                interval_ticks: p.interval_ticks,
            },
        );
    }

    for (i, c) in raw.consumers.into_iter().enumerate() {
        validate_id("consumers", &c.id)?;
        if id_map.consumers_by_name.contains_key(&c.id) {
            return Err(LoadError::DuplicateId {
                section: "consumers",
                id: c.id,
            });
        }
        validate_amount("consumers[].amount", c.amount)?;
        validate_interval("consumers", i, c.interval_ticks)?;
        let resource = id_map
            .resources_by_name
            .get(&c.resource)
            .copied()
            .ok_or_else(|| LoadError::UnknownReference {
                from: format!("consumers[{i}].resource"),
                to: c.resource.clone(),
            })?;
        let cid = ConsumerId(u32::try_from(i).unwrap_or(u32::MAX));
        id_map.consumers_by_name.insert(c.id.clone(), cid);
        id_map.consumer_names.insert(cid, c.id);
        consumers.insert(
            cid,
            Consumer {
                id: cid,
                resource,
                amount: c.amount,
                interval_ticks: c.interval_ticks,
            },
        );
    }

    // Agents.
    let mut agents = Vec::with_capacity(raw.agents.len());
    for (i, a) in raw.agents.into_iter().enumerate() {
        validate_interval("agents", i, a.interval_ticks)?;
        validate_agent_kind(i, &a.kind)?;
        let deadline_ticks = a.deadline_ticks.unwrap_or(DEFAULT_LLM_DEADLINE_TICKS);
        agents.push(AgentSpec {
            kind: a.kind,
            interval_ticks: a.interval_ticks,
            deadline_ticks,
        });
    }

    let goals = raw
        .goals
        .into_iter()
        .map(|g| match g {
            RawGoal::LoopForever => Goal::LoopForever,
        })
        .collect();

    let sl1 = raw.sl1.map(scenario_language_v1::load_value).transpose()?;

    let mut world = World::new(seed);
    world.nodes = nodes;
    world.paths = paths;
    world.movers = movers;
    world.resources = resources;
    world.inventory = inventory;
    world.producers = producers;
    world.consumers = consumers;
    world.sl1 = sl1.clone();
    world.state = crate::world::RunState::Loaded;

    Ok(LoadedScene {
        name: raw.name,
        theme,
        goals,
        agents,
        id_map,
        world,
        sl1,
    })
}

fn validate_name(name: &str) -> Result<(), LoadError> {
    if name.is_empty() {
        return Err(LoadError::InvalidName {
            reason: "empty".to_string(),
        });
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(LoadError::InvalidName {
            reason: format!("longer than {MAX_NAME_LEN} chars"),
        });
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(LoadError::InvalidName {
            reason: "contains control character".to_string(),
        });
    }
    Ok(())
}

fn validate_theme(raw: Option<RawTheme>) -> Result<Theme, LoadError> {
    let raw = raw.ok_or(LoadError::MissingField { field: "theme" })?;
    if raw.palette.len() > MAX_PALETTE_ENTRIES {
        return Err(LoadError::PaletteTooLarge {
            size: raw.palette.len(),
            max: MAX_PALETTE_ENTRIES,
        });
    }
    if raw.palette.is_empty() {
        return Err(LoadError::PaletteTooLarge {
            size: 0,
            max: MAX_PALETTE_ENTRIES,
        });
    }
    for (i, c) in raw.palette.iter().enumerate() {
        if !is_valid_hex_color(c) {
            return Err(LoadError::InvalidColor {
                field: format!("theme.palette[{i}]"),
                value: c.clone(),
            });
        }
    }
    if (raw.background_index as usize) >= raw.palette.len() {
        return Err(LoadError::PaletteIndexOOB {
            field: "theme.background_index".to_string(),
            index: raw.background_index as usize,
            max: raw.palette.len(),
        });
    }
    Ok(Theme {
        palette: raw.palette,
        background_index: raw.background_index,
        font: raw.font,
    })
}

fn is_valid_hex_color(s: &str) -> bool {
    if s.len() != 7 {
        return false;
    }
    let bytes = s.as_bytes();
    if bytes[0] != b'#' {
        return false;
    }
    bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
}

fn check_section_cap(section: &'static str, count: usize) -> Result<(), LoadError> {
    if count > MAX_PIECES_PER_SECTION {
        return Err(LoadError::TooManyPieces {
            section,
            count,
            max: MAX_PIECES_PER_SECTION,
        });
    }
    Ok(())
}

fn validate_id(section: &'static str, id: &str) -> Result<(), LoadError> {
    if id.is_empty()
        || id.len() > MAX_ID_LEN
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(LoadError::InvalidId {
            section,
            id: id.to_string(),
        });
    }
    Ok(())
}

fn validate_coord(id: &str, pos: [f32; 2]) -> Result<(), LoadError> {
    for v in pos {
        if !v.is_finite() || v.abs() > COORD_LIMIT {
            return Err(LoadError::NonFiniteCoord { id: id.to_string() });
        }
    }
    Ok(())
}

fn validate_interval(
    section: &'static str,
    index: usize,
    interval_ticks: u32,
) -> Result<(), LoadError> {
    if !(INTERVAL_MIN..=INTERVAL_MAX).contains(&interval_ticks) {
        return Err(LoadError::IntervalOOB {
            section,
            index,
            value: interval_ticks,
        });
    }
    Ok(())
}

fn validate_amount(field: &'static str, amount: u64) -> Result<(), LoadError> {
    if !(AMOUNT_MIN..=AMOUNT_MAX).contains(&amount) {
        return Err(LoadError::AmountOOB {
            field,
            value: amount,
        });
    }
    Ok(())
}

/// Per tool-spec round-trip acceptance criteria. Accept only known agent kinds; reject
/// feature-gated kinds when the binary wasn't built with the
/// corresponding feature.
fn validate_agent_kind(index: usize, kind: &str) -> Result<(), LoadError> {
    if !KNOWN_AGENT_KINDS.contains(&kind) {
        return Err(LoadError::UnknownAgentKind {
            index,
            kind: kind.to_string(),
        });
    }
    if FEATURE_GATED_AGENT_KINDS.contains(&kind) && !cfg!(feature = "llm-live") {
        return Err(LoadError::AgentKindRequiresFeature {
            index,
            kind: kind.to_string(),
        });
    }
    Ok(())
}

fn validate_inventory_amount(field: &'static str, amount: u64) -> Result<(), LoadError> {
    if amount > AMOUNT_MAX {
        return Err(LoadError::AmountOOB {
            field,
            value: amount,
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    const VALID_SCENE: &str = r##"{
        "schema_version": 1,
        "name": "demo-paths",
        "theme": {
            "palette": ["#0e1116", "#e8eaed", "#7aa2f7", "#bb9af7", "#9ece6a"],
            "background_index": 0,
            "font": "system-ui"
        },
        "pieces": {
            "nodes": [
                { "id": "a", "pos": [100, 100], "shape": "circle", "color": 2 },
                { "id": "b", "pos": [400, 300], "shape": "square", "color": 3 }
            ],
            "paths": [
                { "id": "ab", "from": "a", "to": "b", "color": 3 }
            ],
            "movers": [
                { "id": "m1", "on_path": "ab", "speed": 1.0 }
            ]
        },
        "goals": [ { "type": "loop_forever" } ],
        "agents": [ { "kind": "speed_tuner", "interval_ticks": 30 } ]
    }"##;

    fn modify_scene(field_path: &str, value: serde_json::Value) -> String {
        let mut v: serde_json::Value = serde_json::from_str(VALID_SCENE).unwrap();
        let parts: Vec<&str> = field_path.split('.').collect();
        let mut cursor = &mut v;
        for p in &parts[..parts.len() - 1] {
            cursor = cursor.get_mut(p).expect("path segment exists");
        }
        cursor[parts[parts.len() - 1]] = value;
        serde_json::to_string(&v).unwrap()
    }

    #[test]
    fn happy_path_loads() {
        let loaded = load_scene_str(VALID_SCENE, 42).expect("scene should load");
        assert_eq!(loaded.name, "demo-paths");
        assert_eq!(loaded.theme.palette.len(), 5);
        assert_eq!(loaded.world.nodes.len(), 2);
        assert_eq!(loaded.world.paths.len(), 1);
        assert_eq!(loaded.world.movers.len(), 1);
        assert!(loaded.world.resources.is_empty());
        assert!(loaded.world.inventory.is_empty());
        assert!(loaded.world.producers.is_empty());
        assert!(loaded.world.consumers.is_empty());
        assert_eq!(loaded.agents.len(), 1);
        assert_eq!(loaded.goals, vec![Goal::LoopForever]);
        assert_eq!(loaded.world.state, crate::world::RunState::Loaded);
    }

    #[test]
    fn schema_v2_resources_inventory_and_chains_load() {
        let mut v: serde_json::Value = serde_json::from_str(VALID_SCENE).unwrap();
        v["schema_version"] = serde_json::json!(2);
        v["resources"] = serde_json::json!([
            { "id": "ore", "color": 4 },
            { "id": "plate", "color": 2 }
        ]);
        v["inventory"] = serde_json::json!([
            { "resource": "ore", "amount": 5 },
            { "resource": "plate", "amount": 0 }
        ]);
        v["producers"] = serde_json::json!([
            { "id": "mine", "resource": "ore", "amount": 3, "interval_ticks": 2 }
        ]);
        v["consumers"] = serde_json::json!([
            { "id": "sink", "resource": "ore", "amount": 4, "interval_ticks": 3 }
        ]);
        let s = serde_json::to_string(&v).unwrap();

        let loaded = load_scene_str(&s, 42).expect("v2 scene should load");

        assert_eq!(loaded.world.resources.len(), 2);
        assert_eq!(loaded.world.inventory.get(&ResourceId(0)), Some(&5));
        assert_eq!(loaded.world.inventory.get(&ResourceId(1)), Some(&0));
        assert_eq!(loaded.world.producers.len(), 1);
        assert_eq!(loaded.world.consumers.len(), 1);
        assert_eq!(
            loaded.id_map.resources_by_name.get("ore"),
            Some(&ResourceId(0))
        );
    }

    #[test]
    fn parse_error_carries_line_col() {
        let err = load_scene_str("{ not json", 0).unwrap_err();
        match err {
            LoadError::Parse { line, col, .. } => {
                assert!(line >= 1);
                assert!(col >= 1);
            }
            other => panic!("expected Parse, got {other:?}"),
        }
    }

    #[test]
    fn unsupported_version_rejected() {
        let s = modify_scene("schema_version", serde_json::json!(3));
        match load_scene_str(&s, 0).unwrap_err() {
            LoadError::UnsupportedVersion { found, supported } => {
                assert_eq!(found, 3);
                assert_eq!(supported, 2);
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn empty_name_rejected() {
        let s = modify_scene("name", serde_json::json!(""));
        assert!(matches!(
            load_scene_str(&s, 0).unwrap_err(),
            LoadError::InvalidName { .. }
        ));
    }

    #[test]
    fn name_with_control_char_rejected() {
        let s = modify_scene("name", serde_json::json!("bad\u{0007}name"));
        assert!(matches!(
            load_scene_str(&s, 0).unwrap_err(),
            LoadError::InvalidName { .. }
        ));
    }

    #[test]
    fn palette_too_large_rejected() {
        let big: Vec<String> = (0..40).map(|i| format!("#{:06x}", i)).collect();
        let s = modify_scene("theme.palette", serde_json::json!(big));
        match load_scene_str(&s, 0).unwrap_err() {
            LoadError::PaletteTooLarge { size, max } => {
                assert_eq!(size, 40);
                assert_eq!(max, 32);
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn invalid_color_rejected() {
        let s = modify_scene(
            "theme.palette",
            serde_json::json!(["#0e1116", "not-a-color", "#7aa2f7"]),
        );
        match load_scene_str(&s, 0).unwrap_err() {
            LoadError::InvalidColor { field, value } => {
                assert!(field.contains("theme.palette[1]"));
                assert_eq!(value, "not-a-color");
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn background_index_oob_rejected() {
        let s = modify_scene("theme.background_index", serde_json::json!(99));
        match load_scene_str(&s, 0).unwrap_err() {
            LoadError::PaletteIndexOOB { field, index, max } => {
                assert!(field.contains("background_index"));
                assert_eq!(index, 99);
                assert_eq!(max, 5);
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn duplicate_node_id_rejected() {
        let s = modify_scene(
            "pieces.nodes",
            serde_json::json!([
                { "id": "a", "pos": [0,0], "shape": "circle", "color": 0 },
                { "id": "a", "pos": [1,1], "shape": "square", "color": 0 }
            ]),
        );
        match load_scene_str(&s, 0).unwrap_err() {
            LoadError::DuplicateId { section, id } => {
                assert_eq!(section, "nodes");
                assert_eq!(id, "a");
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn invalid_id_rejected() {
        let s = modify_scene(
            "pieces.nodes",
            serde_json::json!([
                { "id": "bad id!", "pos": [0,0], "shape": "circle", "color": 0 }
            ]),
        );
        match load_scene_str(&s, 0).unwrap_err() {
            LoadError::InvalidId { id, .. } => assert_eq!(id, "bad id!"),
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn non_finite_coord_rejected() {
        // Use a coordinate above the limit.
        let s = modify_scene(
            "pieces.nodes",
            serde_json::json!([
                { "id": "a", "pos": [2.0e7, 0], "shape": "circle", "color": 0 }
            ]),
        );
        assert!(matches!(
            load_scene_str(&s, 0).unwrap_err(),
            LoadError::NonFiniteCoord { .. }
        ));
    }

    #[test]
    fn speed_out_of_range_rejected() {
        let s = modify_scene(
            "pieces.movers",
            serde_json::json!([
                { "id": "m1", "on_path": "ab", "speed": 200.0 }
            ]),
        );
        match load_scene_str(&s, 0).unwrap_err() {
            LoadError::SpeedOutOfRange { id, value } => {
                assert_eq!(id, "m1");
                assert!((value - 200.0).abs() < 1e-6);
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn interval_oob_rejected() {
        let s = modify_scene(
            "agents",
            serde_json::json!([{ "kind": "speed_tuner", "interval_ticks": 0 }]),
        );
        match load_scene_str(&s, 0).unwrap_err() {
            LoadError::IntervalOOB {
                section,
                index,
                value,
            } => {
                assert_eq!(section, "agents");
                assert_eq!(index, 0);
                assert_eq!(value, 0);
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn agent_default_deadline_ticks_is_60() {
        let scene = load_scene_str(VALID_SCENE, 0).unwrap();
        assert_eq!(scene.agents[0].deadline_ticks, DEFAULT_LLM_DEADLINE_TICKS);
        assert_eq!(DEFAULT_LLM_DEADLINE_TICKS, 60);
    }

    #[test]
    fn agent_deadline_ticks_overridable_in_scene_json() {
        let s = modify_scene(
            "agents",
            serde_json::json!([
                { "kind": "speed_tuner", "interval_ticks": 30, "deadline_ticks": 120 }
            ]),
        );
        let scene = load_scene_str(&s, 0).unwrap();
        assert_eq!(scene.agents[0].deadline_ticks, 120);
    }

    #[test]
    fn unknown_agent_kind_rejected() {
        let s = modify_scene(
            "agents",
            serde_json::json!([{ "kind": "bogus_planner", "interval_ticks": 30 }]),
        );
        match load_scene_str(&s, 0).unwrap_err() {
            LoadError::UnknownAgentKind { index, kind } => {
                assert_eq!(index, 0);
                assert_eq!(kind, "bogus_planner");
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    #[cfg(not(feature = "llm-live"))]
    fn llm_agent_kind_rejected_without_feature() {
        let s = modify_scene(
            "agents",
            serde_json::json!([{ "kind": "llm", "interval_ticks": 600 }]),
        );
        match load_scene_str(&s, 0).unwrap_err() {
            LoadError::AgentKindRequiresFeature { index, kind } => {
                assert_eq!(index, 0);
                assert_eq!(kind, "llm");
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    #[cfg(feature = "llm-live")]
    fn llm_agent_kind_accepted_with_feature() {
        let s = modify_scene(
            "agents",
            serde_json::json!([
                { "kind": "llm", "interval_ticks": 600, "deadline_ticks": 90 }
            ]),
        );
        let scene = load_scene_str(&s, 0).unwrap();
        assert_eq!(scene.agents[0].kind, "llm");
        assert_eq!(scene.agents[0].interval_ticks, 600);
        assert_eq!(scene.agents[0].deadline_ticks, 90);
    }

    #[test]
    fn v2_unknown_resource_reference_rejected() {
        let mut v: serde_json::Value = serde_json::from_str(VALID_SCENE).unwrap();
        v["schema_version"] = serde_json::json!(2);
        v["resources"] = serde_json::json!([{ "id": "ore", "color": 2 }]);
        v["producers"] = serde_json::json!([
            { "id": "mine", "resource": "ghost", "amount": 1, "interval_ticks": 1 }
        ]);
        let s = serde_json::to_string(&v).unwrap();

        match load_scene_str(&s, 0).unwrap_err() {
            LoadError::UnknownReference { from, to } => {
                assert_eq!(from, "producers[0].resource");
                assert_eq!(to, "ghost");
            }
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn unknown_reference_rejected() {
        let s = modify_scene(
            "pieces.paths",
            serde_json::json!([{ "id": "ab", "from": "a", "to": "ghost", "color": 0 }]),
        );
        match load_scene_str(&s, 0).unwrap_err() {
            LoadError::UnknownReference { to, .. } => assert_eq!(to, "ghost"),
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn mover_unknown_path_rejected() {
        let s = modify_scene(
            "pieces.movers",
            serde_json::json!([{ "id": "m1", "on_path": "ghost", "speed": 1.0 }]),
        );
        assert!(matches!(
            load_scene_str(&s, 0).unwrap_err(),
            LoadError::UnknownReference { .. }
        ));
    }
}
