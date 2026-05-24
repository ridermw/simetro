//! Tool specifications for LLM backends.
//!
//! Each action the engine accepts is exposed to the model as a single
//! tool with a strict JSON Schema. The bridge sends these to the
//! backend; the model responds with one or more `ToolCall`s and the
//! bridge parses them back into `Action`s.
//!
//! ```text
//!   bridge ── tools ──▶ backend (LLM)
//!     ▲                     │
//!     │                     ▼
//!   Action  ◀── parse ── ToolCall
//! ```
//!
//! Schema format: JSON Schema Draft-07, kept inline as plain strings so
//! we don't pull in a schema crate. Backends that expect a different
//! shape (e.g. OpenAI's "function" wrapping) wrap these themselves.

use crate::backend::ToolSpec;

/// Names of the five tools the bridge exposes. Stable identifiers —
/// callers (LLMs, tests) match on these.
pub mod names {
    pub const NO_OP: &str = "no_op";
    pub const SET_SPEED: &str = "set_speed";
    pub const PLACE_PIECE: &str = "place_piece";
    pub const CONNECT_PIECES: &str = "connect_pieces";
    pub const REMOVE_PIECE: &str = "remove_piece";
}

const NO_OP_SCHEMA: &str = r#"{
  "type": "object",
  "title": "no_op",
  "description": "Do nothing this turn.",
  "properties": {},
  "additionalProperties": false
}"#;

const SET_SPEED_SCHEMA: &str = r#"{
  "type": "object",
  "title": "set_speed",
  "description": "Set a mover's speed multiplier. Speed must be finite and 0.0..=100.0.",
  "properties": {
    "mover": { "type": "integer", "minimum": 0 },
    "speed": { "type": "number", "minimum": 0.0, "maximum": 100.0 }
  },
  "required": ["mover", "speed"],
  "additionalProperties": false
}"#;

const PLACE_PIECE_SCHEMA: &str = r#"{
  "type": "object",
  "title": "place_piece",
  "description": "(P2) Place a new piece in the world.",
  "properties": {
    "piece_kind": { "type": "string", "maxLength": 64 },
    "pos": {
      "type": "array",
      "items": { "type": "number" },
      "minItems": 2,
      "maxItems": 2
    }
  },
  "required": ["piece_kind", "pos"],
  "additionalProperties": false
}"#;

const CONNECT_PIECES_SCHEMA: &str = r#"{
  "type": "object",
  "title": "connect_pieces",
  "description": "(P2) Connect two pieces with a new path.",
  "properties": {
    "from": { "type": "integer", "minimum": 0 },
    "to":   { "type": "integer", "minimum": 0 }
  },
  "required": ["from", "to"],
  "additionalProperties": false
}"#;

const REMOVE_PIECE_SCHEMA: &str = r#"{
  "type": "object",
  "title": "remove_piece",
  "description": "(P2) Remove a piece by id.",
  "properties": {
    "id": { "type": "integer", "minimum": 0 }
  },
  "required": ["id"],
  "additionalProperties": false
}"#;

/// The full set of tool specs the bridge sends to the model. Author
/// tools (PlacePiece / ConnectPieces / RemovePiece) are exposed in P1
/// so the model learns the shape; the engine rejects them with a
/// typed `Warning::InvalidAction` until P2 enables them.
#[must_use]
pub fn action_tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: names::NO_OP.into(),
            json_schema: NO_OP_SCHEMA.into(),
        },
        ToolSpec {
            name: names::SET_SPEED.into(),
            json_schema: SET_SPEED_SCHEMA.into(),
        },
        ToolSpec {
            name: names::PLACE_PIECE.into(),
            json_schema: PLACE_PIECE_SCHEMA.into(),
        },
        ToolSpec {
            name: names::CONNECT_PIECES.into(),
            json_schema: CONNECT_PIECES_SCHEMA.into(),
        },
        ToolSpec {
            name: names::REMOVE_PIECE.into(),
            json_schema: REMOVE_PIECE_SCHEMA.into(),
        },
    ]
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn five_tool_specs_in_canonical_order() {
        let specs = action_tool_specs();
        assert_eq!(specs.len(), 5);
        assert_eq!(specs[0].name, names::NO_OP);
        assert_eq!(specs[1].name, names::SET_SPEED);
        assert_eq!(specs[2].name, names::PLACE_PIECE);
        assert_eq!(specs[3].name, names::CONNECT_PIECES);
        assert_eq!(specs[4].name, names::REMOVE_PIECE);
    }

    #[test]
    fn every_schema_is_valid_json() {
        for s in action_tool_specs() {
            let _v: serde_json::Value =
                serde_json::from_str(&s.json_schema).expect("schemas must be valid JSON");
        }
    }

    #[test]
    fn set_speed_schema_declares_required_fields() {
        let specs = action_tool_specs();
        let v: serde_json::Value = serde_json::from_str(&specs[1].json_schema).unwrap();
        let req = v.get("required").and_then(|r| r.as_array()).unwrap();
        let names: Vec<&str> = req.iter().filter_map(|s| s.as_str()).collect();
        assert!(names.contains(&"mover"));
        assert!(names.contains(&"speed"));
    }
}
