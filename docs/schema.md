# Scene JSON schema

simetro reads a single JSON file (a "scene") and instantiates the
engine's world from it. The same shape supports transit, building,
and resource-mining themes — the rules in `interactions` decide
which.

## Top-level shape

```jsonc
{
  "schema_version": 1,
  "scene_name": "demo-paths",
  "theme": {
    "palette": ["#0e1116", "#e8eaed", "#7aa2f7", "#bb9af7", "#9ece6a"],
    "background_index": 0,
    "font": "system-ui"
  },
  "win_conditions": [
    { "kind": "AllMoversArrive", "min_unique_nodes": 3 }
  ],
  "pieces": [
    { "id": "a", "kind": "Node", "pos": [200, 200], "shape": "circle",   "color": 2 },
    { "id": "b", "kind": "Node", "pos": [600, 200], "shape": "square",   "color": 3 },
    { "id": "c", "kind": "Node", "pos": [400, 480], "shape": "triangle", "color": 4 },
    { "id": "ab", "kind": "Path", "from": "a", "to": "b", "color": 2 },
    { "id": "bc", "kind": "Path", "from": "b", "to": "c", "color": 3 },
    { "id": "ca", "kind": "Path", "from": "c", "to": "a", "color": 4 },
    { "id": "m1", "kind": "Mover", "on_path": "ab", "speed": 1.0 }
  ],
  "interactions": [
    { "trigger": "MoverArrived", "effect": "Pulse", "scope": "Node" }
  ]
}
```

## Rules the loader enforces (PLAN §5.1)

- `schema_version` must equal the engine's supported version. Mismatch
  is a hard `LoadError`.
- `palette.length` ≤ 32. `background_index` < `palette.length`.
- All piece `id`s unique. `Path.from` / `Path.to` must reference
  existing `Node` ids. `Mover.on_path` must reference an existing
  `Path` id.
- `pos` arrays length 2; coordinates finite. `speed` finite, > 0.
- `shape` ∈ {`circle`, `square`, `triangle`, `diamond`}.
- `color` is an index into `palette`; bounds-checked.
- Maximum piece count is 100,000 per scene (PLAN §5.1).

Every violation is reported as a typed `LoadError { field, message }`
with the JSON path to the bad value, and surfaces in the frontend as
a `Fault::LoadError` overlay.

## Winning states

Each `win_conditions` entry is one of:

- `{ "kind": "AllMoversArrive", "min_unique_nodes": N }`
- `{ "kind": "AccumulateResource", "resource": "shape:circle", "amount": N }`
- `{ "kind": "ConnectAll" }`

(P2 will add a small expression language to combine them.)

## Theme

The `theme` block is the single source of truth for color: every
renderer call reads palette indices, never literal colors. Swap a
palette in JSON to reskin the whole app.
