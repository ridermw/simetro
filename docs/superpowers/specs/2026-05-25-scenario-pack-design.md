# 40-scene scenario pack design

## Goal

Replace the visible gallery's simple transit-loop worlds with exactly 40
new local scenarios so the app presents complex systems-game content by
default. The new pack is balanced by difficulty:

| Difficulty | New scenes |
| --- | ---: |
| intro | 10 |
| easy | 10 |
| medium | 10 |
| hard | 10 |

Each difficulty contributes five `scenario_language_v1` systems-game
drafts and five legacy-rendered systems worlds. The final visible pack
therefore contains 20 SL1 scenes and 20 legacy-rendered scenes.

## Architecture

Each visible scene is a first-class `games/*.json` file with:

- `schema_version: 1`
- a `catalog` block that satisfies the world-quality checklist
- a stable kebab-case slug matching the filename
- matching entries in `frontend/src/catalog/scenes.ts`
- matching visible-scene entries in `src-tauri/src/scene_registry.rs`

Existing transit-loop scenes must be removed from the visible frontend
catalog and from the Tauri registry's user-selectable scene list.
`demo-paths` is the exception: keep the file available as an
internal/default/test fixture until the engine, driver tests, and
determinism baselines no longer depend on it. It must not appear in the
visible gallery after this change.

The initial desktop scene must still resolve through the backend scene
registry, never from a frontend-supplied path. If `demo-paths` remains
the default during this pack, the frontend selection state must not
pretend it is one of the visible catalog scenarios.

## Scene types

### SL1 systems-game drafts

The 20 SL1 scenes use `scenario_language_v1` primitives rather than
legacy movers as their gameplay language. They model systems-game
domains such as operations centers, supply chains, datacenters,
hospitals, climate response, factories, research labs, security
operations, and incident command.

SL1 scenes must use the currently supported loader grammar:

- places
- links
- things
- transforms
- demand
- pressure
- objectives
- failure conditions
- victory conditions
- observability, when accepted by the current schema

Because the SL1 renderer is still draft, these entries must be marked
`status: "draft"` in the frontend catalog unless a later renderer change
makes them visually complete.

### Legacy-rendered worlds

The 20 legacy scenes use the current nodes/paths/movers renderer so they
are visible immediately. They must avoid transit framing in titles,
subtitles, metadata, and visual language. Good domains include circuit
boards, ecosystems, kitchens, logistics yards, constellation rituals,
robotics cells, defense grids, lab benches, archives, and other spatial
systems.

These scenes still use path-following movers internally because that is
the available renderer, but the catalog copy and JSON metadata must frame
them as operational systems rather than passenger/ferry/metro/courier
transit loops. They must be marked `status: "ready"` when they load
and render through the current UI.

## Difficulty model

Difficulty is catalog-facing and must reflect conceptual/system
complexity, not player input complexity.

- `intro`: small, legible systems with few entities and forgiving
  pressure.
- `easy`: broader systems with more visible roles, still simple to read.
- `medium`: multi-stage pipelines, competing pressure, and observable
  tradeoffs.
- `hard`: dense operational systems with multiple failure modes and
  higher stakes.

The 40 new scenes must be exactly balanced: 10 intro, 10 easy, 10
medium, and 10 hard.

## Data flow

1. Authored JSON lives in `games/<slug>.json`.
2. Frontend metadata in `SCENE_CATALOG` exposes only the 40 complex
   visible scenes in the browser.
3. Tauri `SceneRegistry` resolves visible scene ids plus any internal
   default/test fixtures to local `games/*.json` paths; the frontend
   never sends a file path.
4. Existing scene switching loads by registry-backed scene id.

## Error handling and safety

- Catalog and registry entries must stay aligned with `games/*.json`.
- Scene ids stay stable, local, and kebab-case.
- Visible catalog entries must exclude old transit-loop scenes.
- SL1 behavior-bearing fields must use the strict schema accepted by the
  current loader.
- Invalid scenes must fail visibly through existing load-error paths.
- User-facing catalog strings continue to render as text, not HTML.

## Validation

Use the smallest relevant checks for the content pack:

```bash
cargo test -p simetro-engine --test world_quality_checklist -- --nocapture
cd frontend && npm test -- --run catalog scene_browser
cd frontend && npm run typecheck
cd frontend && npm run build
```

If Tauri disk space allows it, also run:

```bash
cd src-tauri && cargo test --locked scene_registry -- --nocapture
```

## Implementation notes

The diff will be large because 40 JSON files plus catalog and registry
entries are required. Keep it reviewable by using consistent scene
templates, stable ordering grouped by difficulty, and no unrelated
refactors.
