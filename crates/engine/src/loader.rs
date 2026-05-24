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
//! (this file). Validation enforces every row in PLAN §5.1. Each field's
//! violation maps to a specific [`LoadError`] variant so the canvas
//! overlay can point at the source location.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::components::{Mover, MoverId, Node, NodeId, NodeShape, Path, PathId};
use crate::error::LoadError;
use crate::world::World;

const SUPPORTED_SCHEMA_VERSION: u32 = 1;
const MAX_NAME_LEN: usize = 200;
const MAX_PALETTE_ENTRIES: usize = 32;
const MAX_PIECES_PER_SECTION: usize = 100_000;
const MAX_ID_LEN: usize = 64;
const COORD_LIMIT: f32 = 1.0e6;
const SPEED_MIN: f32 = 0.0;
const SPEED_MAX: f32 = 100.0;
const INTERVAL_MIN: u32 = 1;
const INTERVAL_MAX: u32 = 10_000;

/// Specification for an agent the engine should instantiate at startup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSpec {
    pub kind: String,
    pub interval_ticks: u32,
}

/// Win/end condition for the scene. Only `LoopForever` is defined in P1.
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
    pub node_names: BTreeMap<NodeId, String>,
    pub path_names: BTreeMap<PathId, String>,
    pub mover_names: BTreeMap<MoverId, String>,
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
    if raw.schema_version != SUPPORTED_SCHEMA_VERSION {
        return Err(LoadError::UnsupportedVersion {
            found: raw.schema_version,
            supported: SUPPORTED_SCHEMA_VERSION,
        });
    }

    validate_name(&raw.name)?;

    let theme = validate_theme(raw.theme)?;

    // Section size caps.
    check_section_cap("nodes", raw.pieces.nodes.len())?;
    check_section_cap("paths", raw.pieces.paths.len())?;
    check_section_cap("movers", raw.pieces.movers.len())?;

    // Build the id map by interning string ids to dense numeric ids.
    let mut id_map = IdMap::default();
    let mut nodes = BTreeMap::new();
    let mut paths = BTreeMap::new();
    let mut movers = BTreeMap::new();

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

    // Agents.
    let mut agents = Vec::with_capacity(raw.agents.len());
    for (i, a) in raw.agents.into_iter().enumerate() {
        if a.interval_ticks < INTERVAL_MIN || a.interval_ticks > INTERVAL_MAX {
            return Err(LoadError::IntervalOOB {
                agent_index: i,
                value: a.interval_ticks,
            });
        }
        agents.push(AgentSpec {
            kind: a.kind,
            interval_ticks: a.interval_ticks,
        });
    }

    let goals = raw
        .goals
        .into_iter()
        .map(|g| match g {
            RawGoal::LoopForever => Goal::LoopForever,
        })
        .collect();

    let mut world = World::new(seed);
    world.nodes = nodes;
    world.paths = paths;
    world.movers = movers;
    world.state = crate::world::RunState::Loaded;

    Ok(LoadedScene {
        name: raw.name,
        theme,
        goals,
        agents,
        id_map,
        world,
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
        assert_eq!(loaded.agents.len(), 1);
        assert_eq!(loaded.goals, vec![Goal::LoopForever]);
        assert_eq!(loaded.world.state, crate::world::RunState::Loaded);
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
        let s = modify_scene("schema_version", serde_json::json!(2));
        match load_scene_str(&s, 0).unwrap_err() {
            LoadError::UnsupportedVersion { found, supported } => {
                assert_eq!(found, 2);
                assert_eq!(supported, 1);
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
            LoadError::IntervalOOB { agent_index, value } => {
                assert_eq!(agent_index, 0);
                assert_eq!(value, 0);
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
