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

/// Names of the tools the bridge exposes. Stable identifiers —
/// callers (LLMs, tests) match on these.
pub mod names {
    pub const NO_OP: &str = "no_op";
    pub const SET_SPEED: &str = "set_speed";
    pub const PLACE_PIECE: &str = "place_piece";
    pub const CONNECT_PIECES: &str = "connect_pieces";
    pub const REMOVE_PIECE: &str = "remove_piece";

    // Author tools (P2.A task 9).
    pub const DEFINE_RESOURCE: &str = "define_resource";
    pub const ADD_PRODUCER: &str = "add_producer";
    pub const ADD_CONSUMER: &str = "add_consumer";
    pub const SET_GOAL: &str = "set_goal";
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
  "description": "Place a new node in the world. piece_kind may be node or a node shape.",
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
  "description": "Connect two nodes with a new directed path.",
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
  "description": "Remove a safe node by id.",
  "properties": {
    "id": { "type": "integer", "minimum": 0 }
  },
  "required": ["id"],
  "additionalProperties": false
}"#;

const DEFINE_RESOURCE_SCHEMA: &str = r#"{
  "type": "object",
  "title": "define_resource",
  "description": "Author tool: create a new resource kind addressable by name. Name must match [A-Za-z0-9_-]+ and be ≤64 chars.",
  "properties": {
    "name": { "type": "string", "minLength": 1, "maxLength": 64 },
    "color": { "type": "integer", "minimum": 0, "maximum": 255 }
  },
  "required": ["name", "color"],
  "additionalProperties": false
}"#;

const ADD_PRODUCER_SCHEMA: &str = r#"{
  "type": "object",
  "title": "add_producer",
  "description": "Author tool: add a producer that emits `amount` of `resource` (by name) every `interval_ticks`.",
  "properties": {
    "resource": { "type": "string", "minLength": 1, "maxLength": 64 },
    "amount": { "type": "integer", "minimum": 1, "maximum": 1000000 },
    "interval_ticks": { "type": "integer", "minimum": 1, "maximum": 10000 }
  },
  "required": ["resource", "amount", "interval_ticks"],
  "additionalProperties": false
}"#;

const ADD_CONSUMER_SCHEMA: &str = r#"{
  "type": "object",
  "title": "add_consumer",
  "description": "Author tool: add a consumer that drains `amount` of `resource` (by name) every `interval_ticks` when inventory is sufficient.",
  "properties": {
    "resource": { "type": "string", "minLength": 1, "maxLength": 64 },
    "amount": { "type": "integer", "minimum": 1, "maximum": 1000000 },
    "interval_ticks": { "type": "integer", "minimum": 1, "maximum": 10000 }
  },
  "required": ["resource", "amount", "interval_ticks"],
  "additionalProperties": false
}"#;

const SET_GOAL_SCHEMA: &str = r#"{
  "type": "object",
  "title": "set_goal",
  "description": "Author tool: set the scene's win/end condition. Currently only `loop_forever` is accepted.",
  "properties": {
    "goal": { "type": "string", "enum": ["loop_forever"] }
  },
  "required": ["goal"],
  "additionalProperties": false
}"#;

/// The full set of tool specs the bridge sends to the model. Author
/// tools (PlacePiece / ConnectPieces / RemovePiece, plus the P2.A
/// task 9 graph tools DefineResource / AddProducer / AddConsumer /
/// SetGoal) are engine-validated: valid requests mutate the world,
/// invalid ones surface as typed `Warning::InvalidAction` messages.
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
        ToolSpec {
            name: names::DEFINE_RESOURCE.into(),
            json_schema: DEFINE_RESOURCE_SCHEMA.into(),
        },
        ToolSpec {
            name: names::ADD_PRODUCER.into(),
            json_schema: ADD_PRODUCER_SCHEMA.into(),
        },
        ToolSpec {
            name: names::ADD_CONSUMER.into(),
            json_schema: ADD_CONSUMER_SCHEMA.into(),
        },
        ToolSpec {
            name: names::SET_GOAL.into(),
            json_schema: SET_GOAL_SCHEMA.into(),
        },
    ]
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn nine_tool_specs_in_canonical_order() {
        let specs = action_tool_specs();
        assert_eq!(specs.len(), 9);
        assert_eq!(specs[0].name, names::NO_OP);
        assert_eq!(specs[1].name, names::SET_SPEED);
        assert_eq!(specs[2].name, names::PLACE_PIECE);
        assert_eq!(specs[3].name, names::CONNECT_PIECES);
        assert_eq!(specs[4].name, names::REMOVE_PIECE);
        assert_eq!(specs[5].name, names::DEFINE_RESOURCE);
        assert_eq!(specs[6].name, names::ADD_PRODUCER);
        assert_eq!(specs[7].name, names::ADD_CONSUMER);
        assert_eq!(specs[8].name, names::SET_GOAL);
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
