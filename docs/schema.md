# Scene JSON schema (v1/v2)

simetro reads a single JSON file (a "scene") and instantiates the
engine's world from it. The schema is versioned (`schema_version: 1`
or `2`) and every field is bounds-checked at load time; violations surface as
typed [`LoadError`](../crates/engine/src/error.rs) variants and reach
the renderer as a `Fault::LoadError` overlay (PLAN §5.1, §11.2).

The canonical loader is
[`crates/engine/src/loader.rs`](../crates/engine/src/loader.rs); this
document mirrors what it accepts.

## Top-level shape

```jsonc
{
  "schema_version": 2,
  "name": "demo-paths",
  "theme": {
    "palette": ["#0e1116", "#e8eaed", "#7aa2f7", "#bb9af7", "#9ece6a"],
    "background_index": 0,
    "font": "system-ui",
  },
  "pieces": {
    "nodes": [
      { "id": "a", "pos": [120, 200], "shape": "circle", "color": 2 },
      { "id": "b", "pos": [420, 120], "shape": "square", "color": 3 },
      { "id": "c", "pos": [620, 320], "shape": "triangle", "color": 4 },
    ],
    "paths": [
      { "id": "ab", "from": "a", "to": "b", "color": 3 },
      { "id": "bc", "from": "b", "to": "c", "color": 4 },
      { "id": "ca", "from": "c", "to": "a", "color": 2 },
    ],
    "movers": [{ "id": "m1", "on_path": "ab", "speed": 0.8 }],
  },
  "resources": [{ "id": "ore", "color": 4 }],
  "inventory": [{ "resource": "ore", "amount": 0 }],
  "producers": [
    { "id": "mine", "resource": "ore", "amount": 3, "interval_ticks": 60 },
  ],
  "consumers": [
    { "id": "sink", "resource": "ore", "amount": 2, "interval_ticks": 120 },
  ],
  "goals": [{ "type": "loop_forever" }],
  "agents": [{ "kind": "speed_tuner", "interval_ticks": 30 }],
}
```

## Required fields

| Field            | Type   | Notes                                                          |
| ---------------- | ------ | -------------------------------------------------------------- |
| `schema_version` | u32    | `1` for P1 scenes, `2` for resource-chain scenes.              |
| `name`           | string | ≤200 chars, no control characters.                             |
| `pieces`         | object | Container with `nodes` / `paths` / `movers` arrays (each opt). |

## Optional top-level fields

| Field       | Type   | Default                        | Notes                                         |
| ----------- | ------ | ------------------------------ | --------------------------------------------- |
| `theme`     | object | omitted = default dark palette | See **Theme** below.                          |
| `goals`     | array  | `[]`                           | See **Goals** below.                          |
| `agents`    | array  | `[]`                           | See **Agents** below.                         |
| `resources` | array  | `[]`                           | v2 resource kinds; v1 auto-upgrades to empty. |
| `inventory` | array  | `[]`                           | v2 global starting stock by resource.         |
| `producers` | array  | `[]`                           | v2 deterministic stock sources.               |
| `consumers` | array  | `[]`                           | v2 deterministic stock sinks.                 |

The loader ignores unknown top-level fields. Polished authored worlds
may therefore include a `catalog` object for review metadata while still
remaining `schema_version: 1`; the engine does not read it and no schema
v2 fields are introduced.

Schema v1 scenes are accepted by the v2-capable loader and auto-upgrade with
empty resources, inventory, producers, and consumers.

## Catalog metadata convention

`catalog` is a documentation/review convention for `games/*.json`, not a
simulation input. See [world-quality.md](world-quality.md) for the full
checklist, `docs/world-template.jsonc`, and the catalog/registry validation
gate.

```jsonc
{
  "slug": "demo-paths",
  "title": "Demo Paths",
  "version": "1.0.0",
  "author": "simetro team",
  "description": "A compact triangular loop...",
  "tags": ["demo", "loop", "triangle"],
  "palette_note": "Dark background with three route accents.",
  "layout_note": "Three nodes form a wide triangular silhouette.",
  "node_language_note": "Circle, square, and triangle are route-coded.",
  "mover_path_note": "Each mover starts on a different loop edge.",
  "review_note": "Reviewed for v1 validity and visual readability.",
}
```

## Theme

```jsonc
{
  "palette": ["#0e1116", "#e8eaed", "#7aa2f7"],
  "background_index": 0,
  "font": "system-ui",
}
```

- `palette` — array of `#RRGGBB` strings, ≤32 entries. Index 0 is
  conventionally the background; index 1 is the foreground; later
  indices are accent colors.
- `background_index` — u8, must be `< palette.len()`.
- `font` — string, defaults to `"system-ui"`.

## Pieces

The `pieces` object has three optional arrays. Each array is capped at
**100,000** entries (PLAN §5.1).

### Nodes

```jsonc
{ "id": "a", "pos": [120, 200], "shape": "circle", "color": 2 }
```

| Field   | Type        | Constraint                                                    |
| ------- | ----------- | ------------------------------------------------------------- |
| `id`    | string      | ≤64 chars, `[a-zA-Z0-9_-]+`, unique across all pieces.        |
| `pos`   | `[f32; 2]`  | Finite (no NaN/Inf); within `±1e6`.                           |
| `shape` | enum string | `circle` \| `square` \| `triangle` \| `diamond` \| `hexagon`. |
| `color` | u8          | Index into `theme.palette`; bounds-checked.                   |

### Paths

```jsonc
{ "id": "ab", "from": "a", "to": "b", "color": 3 }
```

| Field   | Type   | Constraint                          |
| ------- | ------ | ----------------------------------- |
| `id`    | string | Same rules as node id.              |
| `from`  | string | Must reference an existing node id. |
| `to`    | string | Must reference an existing node id. |
| `color` | u8     | Palette index (bounds-checked).     |

### Movers

```jsonc
{ "id": "m1", "on_path": "ab", "speed": 0.8 }
```

| Field     | Type   | Constraint                          |
| --------- | ------ | ----------------------------------- |
| `id`      | string | Same rules as node id.              |
| `on_path` | string | Must reference an existing path id. |
| `speed`   | f32    | Finite, `0.0..=100.0`.              |

## Goals

P1 supports a single goal kind. The shape is a tagged union with
`type` as the discriminant:

```jsonc
[{ "type": "loop_forever" }]
```

`loop_forever` declares "no terminal condition; run until paused." P2
will add scored goals (`accumulate_resource`, `connect_all`, etc.).

## Agents

```jsonc
[{ "kind": "speed_tuner", "interval_ticks": 30 }]
```

| Field            | Type   | Constraint                                                |
| ---------------- | ------ | --------------------------------------------------------- |
| `kind`           | string | P1 supports `"speed_tuner"`. Unknown kinds → `LoadError`. |
| `interval_ticks` | u32    | `1..=10_000`. How often the engine calls `Agent::act`.    |

## Resources, inventory, producers, consumers (v2)

Resource chains use global inventory. Each tick increments the world tick,
then production runs after movement/interaction: producers fire first in
stable id order, then consumers fire in stable id order. Stock produced on a
tick can therefore be consumed on that same tick.

### Resources

```jsonc
{ "id": "ore", "color": 4 }
```

| Field   | Type   | Constraint                                    |
| ------- | ------ | --------------------------------------------- |
| `id`    | string | Same id rules as pieces; unique in resources. |
| `color` | u8     | Palette index (bounds-checked).               |

### Inventory

```jsonc
{ "resource": "ore", "amount": 0 }
```

`resource` must reference a resource id. `amount` is `0..=1_000_000`.
Resources omitted from `inventory` start at `0`.

### Producers and consumers

```jsonc
{ "id": "mine", "resource": "ore", "amount": 3, "interval_ticks": 60 }
{ "id": "sink", "resource": "ore", "amount": 2, "interval_ticks": 120 }
```

| Field            | Type   | Constraint                                             |
| ---------------- | ------ | ------------------------------------------------------ |
| `id`             | string | Same id rules; unique within producers/consumers.      |
| `resource`       | string | Must reference a resource id.                          |
| `amount`         | u64    | `1..=1_000_000`.                                       |
| `interval_ticks` | u32    | `1..=10_000`; fires when `world.tick % interval == 0`. |

## Identifier interning

String ids are convenient for humans; the engine and wire protocol
use dense numeric ids (`u32`). The loader builds an
[`IdMap`](../crates/engine/src/loader.rs) at load time and the
protocol's [`StaticPayload`](protocol.md#staticpayload) ships
`node_names`, `path_names`, `mover_names` so the Inspector can
translate ids back for display.

## Errors

Every loader error variant is reachable from
[`load_scene_str`](../crates/engine/src/loader.rs):

| `LoadError` variant  | Triggered by                                                                                   |
| -------------------- | ---------------------------------------------------------------------------------------------- |
| `Parse`              | Malformed JSON. Carries `line` / `col` / `message`.                                            |
| `UnsupportedVersion` | `schema_version` outside `1..=2`.                                                              |
| `InvalidName`        | Empty, too long, or control-char in `name`.                                                    |
| `PaletteTooLarge`    | More than 32 palette entries.                                                                  |
| `InvalidColor`       | Palette string does not match `^#[0-9a-fA-F]{6}$`.                                             |
| `PaletteIndexOOB`    | `background_index` or piece `color` out of range.                                              |
| `TooManyPieces`      | Any of `nodes`/`paths`/`movers` exceeds 100,000.                                               |
| `DuplicateId`        | Two pieces in the same section share an id.                                                    |
| `InvalidId`          | Id violates length or character rules.                                                         |
| `NonFiniteCoord`     | A coordinate is NaN/Inf or outside `±1e6`.                                                     |
| `SpeedOutOfRange`    | Mover speed outside `0.0..=100.0`.                                                             |
| `IntervalOOB`        | Agent/producer/consumer `interval_ticks` outside `1..=10_000`.                                 |
| `AmountOOB`          | Inventory/producer/consumer amount outside supported bounds.                                   |
| `UnknownReference`   | `Path.from` / `Path.to` / `Mover.on_path` / resource-chain reference points at a missing item. |
| `UnknownAgentKind`   | `agents[i].kind` is not a registered agent name.                                               |

The frontend renders these as a `Fault::LoadError` overlay; see
[runbook.md](runbook.md) for operator actions.
