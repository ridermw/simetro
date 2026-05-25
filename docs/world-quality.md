# World quality checklist

Use this checklist before calling a current `games/*.json` gallery world
polished. It is an authoring gate for legacy visual scenes, not a
`scenario_language_v1` schema: worlds must keep
`"schema_version": 1`, and the engine still treats `catalog` as
non-simulation metadata.

## Required checklist

- **Valid v1 JSON:** `cargo test -p simetro-engine --test world_quality_checklist`
  must pass, and the world must load through `load_scene_str`.
- **Distinct palette:** include a background, foreground, and at least
  three separated accent colors. Avoid duplicate or near-duplicate
  `#RRGGBB` values.
- **Distinct layout silhouette:** node positions should create a
  recognizable footprint, not a line or tiny cluster.
- **Intentional node language:** use at least three node shapes and at
  least three node colors; document what the shape/color choices mean.
- **Meaningful mover starts:** every mover starts on a visible,
  non-zero-length path that is part of the world’s readable route
  structure.
- **Complete catalog metadata:** add a top-level `catalog` object with
  title, slug, version, author, description, tags, and short notes for
  palette, layout, node language, and mover path choices.
- **Screenshot/review evidence:** provide either a screenshot reference
  in `catalog.screenshot` or a concise `catalog.review_note` explaining
  what was reviewed.

## Validation approach

The executable gate lives in
`crates/engine/tests/world_quality_checklist.rs` and scans every
`games/*.json` file. It verifies loader compatibility plus objective
quality signals: palette separation, non-degenerate layout silhouette,
three or more node shapes/colors, visible mover home paths, and complete
catalog/review metadata. It also checks that every polished world is
registered in the frontend catalog and the Tauri scene registry.

Human review covers the subjective part: the palette should feel
coherent, the silhouette should be recognizable in a screenshot, and the
catalog notes should explain the design rather than merely restating the
field names.

## Mechanical one-world PR recipe

1. Copy `docs/world-template.jsonc` to `games/<slug>.json`, remove comments,
   keep `"schema_version": 1`, and set `name` plus `catalog.slug` to `<slug>`.
2. Fill the required catalog notes with reviewable design intent, not field
   labels. Use lowercase ids so the filename, `catalog.slug`, frontend id,
   and Tauri registry id all match.
3. Add one frontend entry with `defineScene({ id: "<slug>", ... })` in
   `frontend/src/catalog/scenes.ts`; `scene_path` and the default screenshot
   target are derived by helper.
4. Add one Tauri registry line, `scene_entry!("<slug>")`, in
   `src-tauri/src/scene_registry.rs`.
5. Run:

   ```bash
   cargo test -p simetro-engine --test world_quality_checklist
   cd frontend && npm test -- --run catalog scene_browser scene_commands
   ```

For paired logic/world delivery, keep the logic PR separate when possible and
put only the JSON/catalog/registry/baseline churn in the mechanical world PR.
