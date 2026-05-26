# UI Gallery & Scene Visibility Design

**Date:** 2026-05-25
**Status:** Approved (Mega Plan Review completed 2026-05-25)

## Problem

simetro has 52 scenes (21 SL1 scenarios, 31 transit loops) but the UI
doesn't surface them well. All 21 SL1 scenes are marked `draft`. The
scene browser is a flat 320px button list — no categorization, no visual
previews, no discovery. The app auto-loads `demo-paths` without showing
what else exists.

## Design Thesis

**Correct rendering of each scene is the #1 priority.** The gallery and
thumbnails exist to prove that every scene loads and renders correctly.
Playwright E2E captures verify each scene before any gallery polish.
No human interaction should be required for validation.

## Architecture

### Two-view app

1. **Gallery View** — full-page grid shown on app launch.
   - Sections: "SL1 Scenarios" (21) then "Transit Loops" (31).
   - Sorted by difficulty within each section (intro → easy → medium → hard).
   - Filter bar: All / SL1 / Transit chips; difficulty dropdown.
   - Cards show thumbnail rendered from pre-built StaticPayload JSON.
   - Thumbnails lazy-loaded via IntersectionObserver (only visible cards render).
   - Empty filter results show "No scenes match" text.

2. **Simulation View** — current canvas + HUD + compact floating switcher.
   - Compact pill in top-right (scene title + prev/next + gallery button).
   - Auto-hides after 3s inactivity; shows on mouse near top-right.

### Navigation

- App launches into Gallery View (replaces auto-load of `demo-paths`).
- URL param `?scene=X` bypasses gallery and auto-loads scene X directly.
- Click card → transition to Simulation View, load that scene.
- Escape key or gallery button → return to Gallery View.
- Compact switcher prev/next arrows cycle through scenes without returning to gallery.
- A `transitioning` flag prevents clicks during pending transitions (5s safety timeout).

### Transport Lifecycle

- **Gallery view:** Transport is DISCONNECTED. No tick loop, no renderer activity.
- **Simulation view:** Transport connects on entry, disconnects on exit to gallery.
- Thumbnail canvases release GPU memory (`canvas.width = 0`) when entering sim view.
- On return to gallery, thumbnails re-render lazily.

## Thumbnail Architecture (Critical Constraint)

The frontend renderer only understands protocol-level `StaticPayload` and
`SnapshotPayload` messages — it does NOT parse scene JSON directly. Therefore:

**Build step:** A Rust CLI (`simetro-headless --emit-static <scene_id>`) loads
each scene, runs the engine for 1 tick, and serializes the resulting `StaticPayload`
to JSON. These files are committed/generated into `frontend/public/static-payloads/`.

**Runtime:** Gallery cards fetch `/static-payloads/{scene_id}.json`, parse it,
and render into a mini-canvas using the existing renderer path.

**Fallback:** If fetch returns 404 or JSON is malformed or renderer throws,
the card shows a palette swatch (colored rectangle from scene metadata). No crash.

## Priority Order

Implementation is ordered by what builds confidence in correctness first:

1. **Promote SL1 scenes to `ready`** — data change only.
2. **Build `simetro-headless --emit-static`** — Rust CLI generates StaticPayload
   JSON for all 52 scenes into `frontend/public/static-payloads/`.
3. **Playwright E2E per-scene rendering verification** — prove every scene
   loads and paints frames without error. Capture a screenshot per scene.
   This is the gate: no scene ships as `ready` unless Playwright confirms it renders.
4. **Gallery View component** — full-page grid with sections, cards, filter bar,
   lazy-loading via IntersectionObserver.
5. **Scene card with thumbnail** — fetches static payload, renders into mini-canvas.
6. **View routing in main.ts** — ViewRouter class (~40 lines) managing gallery ↔ sim.
7. **Compact scene switcher** — replaces old scene_browser.ts.
8. **Visual polish** — hover effects, transitions, responsive grid.

## Data Changes

- `frontend/src/catalog/scenes.ts`: All 21 SL1 scenes promoted from
  `status: "draft"` to `status: "ready"`.
- No new npm dependencies. Pure TypeScript/DOM.

## New Files

| File | Purpose |
|------|---------|
| `frontend/src/ui/gallery_view.ts` | Full-page gallery (sections, grid, filter, lazy-load) |
| `frontend/src/ui/gallery_card.ts` | Individual card with thumbnail canvas + fallback |
| `frontend/src/ui/scene_switcher.ts` | Compact floating pill (replaces scene_browser.ts) |
| `frontend/src/ui/thumbnail_renderer.ts` | Renders StaticPayload into mini-canvas |
| `frontend/src/tests/gallery.test.ts` | Unit tests for gallery/filter/router logic |
| `frontend/src/tests/e2e/gallery.spec.ts` | E2E: gallery renders, card→sim, sim→gallery |
| `frontend/src/tests/e2e/scene_renders.spec.ts` | E2E: every scene loads + renders correctly |
| `frontend/public/static-payloads/*.json` | Pre-built StaticPayload per scene (52 files) |

## Modified Files

| File | Change |
|------|--------|
| `frontend/src/main.ts` | Add ViewRouter class, `?scene=X` param, remove scene_browser |
| `frontend/src/catalog/scenes.ts` | Promote 21 SL1 scenes to `ready` |
| `frontend/index.html` | Add `#gallery` container alongside `#app` |
| `crates/headless/src/main.rs` | Add `--emit-static` subcommand |

## Deleted Files

| File | Reason |
|------|--------|
| `frontend/src/ui/scene_browser.ts` | Replaced by gallery_view + scene_switcher |

## Component Interfaces

```typescript
// gallery_view.ts
export interface SceneSelectIntent {
  readonly kind: "SelectScene";
  readonly scene_id: string;
}

export type GalleryFilter = {
  world_kind: "all" | "sl1_scenario" | "transit_loop";
  difficulty: "all" | SceneDifficulty;
};

export class GalleryView {
  constructor(
    container: HTMLElement,
    scenes: readonly SceneCatalogEntry[],
    onSelect: (scene_id: string) => void
  );
  show(): void;
  hide(): void;
  setFilter(filter: GalleryFilter): void;
}

// scene_switcher.ts
export class SceneSwitcher {
  constructor(
    parent: HTMLElement,
    scenes: readonly SceneCatalogEntry[],
    handler: SceneSelectHandler
  );
  setSelected(scene_id: string): void;
  show(): void;
  hide(): void;
}

// thumbnail_renderer.ts — renders StaticPayload into mini-canvas
export class ThumbnailRenderer {
  render(payload: StaticPayload, width: number, height: number): HTMLCanvasElement;
  dispose(): void;
}
```

## View Routing State Machine

```
AppState.view: "gallery" | "simulation"
AppState.transitioning: boolean

Launch (no ?scene param) → gallery
Launch (?scene=X) → simulation (direct load, skip gallery)

Gallery
  ├── user clicks card (transitioning=false) → set transitioning=true → simulation
  ├── user clicks card (transitioning=true) → IGNORED
  ├── filter/search → stays in gallery
  └── transition failure → stays in gallery + error toast

Simulation
  ├── Escape / gallery button → gallery (disconnect transport)
  ├── prev/next arrows → simulation (loads adjacent scene)
  └── scene loads → stays in simulation, transitioning=false
```

## Error Handling

| Error | Rescue | User Sees |
|-------|--------|-----------|
| Thumbnail fetch 404 | Palette swatch fallback | Card with colors, no preview |
| Thumbnail JSON malformed | Palette swatch fallback | Card with colors, no preview |
| Thumbnail renderer throws | Palette swatch fallback | Card with colors, no preview |
| Canvas getContext returns null | Palette swatch fallback | Card with colors, no preview |
| Scene not in catalog | FaultOverlay message | "Unknown scene" error |
| Tauri set_scene fails | Return to gallery + toast | Error message, stays on gallery |
| Transport connect fails | Return to gallery + toast | "Connection failed" message |
| Transition timeout (5s) | Clear transitioning flag | Re-enables interaction |

## Build Step: emit-static

```rust
// crates/headless/src/main.rs (new subcommand)
// simetro-headless --emit-static [--output-dir frontend/public/static-payloads]
//
// For each scene in the registry:
//   1. Load scene JSON
//   2. Initialize engine, run 1 tick
//   3. Serialize StaticPayload to JSON
//   4. Write to {output_dir}/{scene_id}.json
//
// On scene load failure: log error to stderr, skip scene, continue.
// Exit code: 0 if all scenes succeed, 1 if any failed (CI gate).
```

This reuses the existing engine + protocol crates. No new dependencies.

## Gallery Visual Design

- Full-page dark background (`#0e1116`), content centered, max-width 1200px.
- Header: "simetro" + filter chips (All / SL1 / Transit) + difficulty dropdown.
- Section headers: category name + scene count badge.
- CSS Grid: 3 columns wide, 2 medium, 1 narrow (responsive).
- Cards: 16:9 thumbnail, title (bold), subtitle, difficulty pill (green/blue/amber/red).
- Hover: slight scale-up, brighter border, "Launch →" overlay on thumbnail.

## Compact Switcher Design

- Small floating pill, top-right corner of simulation view.
- Content: `◀ [Scene Title] ▶  ⊞`
- Auto-hides after 3s of inactivity; reappears on mouse near top-right.
- Minimal footprint — doesn't obscure simulation.

## Playwright E2E Verification

**This is the quality gate for the entire feature.**

```typescript
// frontend/src/tests/e2e/scene_renders.spec.ts
import { test, expect } from "@playwright/test";
import { SCENE_CATALOG } from "../src/catalog/scenes";

for (const scene of SCENE_CATALOG) {
  test(`scene "${scene.id}" renders without error`, async ({ page }) => {
    await page.goto(`/?scene=${scene.id}`);
    // Wait for first frame to paint
    await page.waitForSelector("#scene", { state: "visible" });
    // No error overlays
    await expect(page.locator("[data-fault]")).toHaveCount(0);
    // Capture screenshot for visual regression
    await page.screenshot({
      path: `e2e-results/scenes/${scene.id}.png`,
      clip: { x: 0, y: 0, width: 960, height: 540 },
    });
  });
}
```

**Note:** The `?scene=X` URL param bypasses the gallery and loads the scene
directly. This serves both backward-compat for existing smoke tests and
efficient per-scene E2E testing.

## Testing Strategy

1. **Playwright E2E (highest priority):**
   - Every scene loads without errors (`scene_renders.spec.ts`).
   - Gallery view renders all sections and cards (`gallery.spec.ts`).
   - Clicking a card navigates to simulation view.
   - Simulation → gallery returns correctly.
   - Compact switcher prev/next cycles correctly.
   - Rapid-click stress test (10 cards in <1s → no crash, deterministic end state).

2. **Vitest unit tests:**
   - Gallery filter logic (category + difficulty).
   - Scene sorting (type → difficulty).
   - ViewRouter state machine (all transitions + transitioning guard).
   - Catalog promotion (no `draft` SL1 scenes remain).
   - Thumbnail fallback (bad input → palette swatch).

3. **Integration tests (Rust):**
   - `emit-static` produces valid StaticPayload JSON for all 52 scenes.
   - `emit-static` with invalid scene → error exit code.

4. **Existing tests must pass:**
   - `frontend/npm run typecheck && npm run lint && npm test -- --run`
   - Existing smoke/sl1_hud/animates E2E tests (use `?scene=demo-paths`).
   - Scene registry tests in `src-tauri/`.

## Safety Invariants (from project instructions)

- All text rendered via `textContent`, never `innerHTML`.
- Scene selection still goes through registry by `scene_id` — no frontend-supplied paths.
- `cmd_get_scene_json` uses the registry, rejecting unknown ids.
- Failed scene loads preserve previous scene or show gallery.
- No new npm dependencies.

## Non-Goals

- Animated card transitions on first release.
- Full-text search of scene content (filter by type/difficulty is enough).
- Scene favoriting or history tracking.
- Editable scene metadata from the gallery.
- Live thumbnails that re-render on scene changes (build-step is sufficient).
- User-editable custom scenes folder.

## Mega Plan Review Decisions (2026-05-25)

| # | Issue | Decision |
|---|-------|----------|
| 1 | Thumbnail data source | Rust CLI generates StaticPayload (not frontend scene parse) |
| 2 | Transport in gallery | Disconnect in gallery, reconnect on scene select |
| 6 | Gallery→sim failure (no prev scene) | Return to gallery + error toast |
| 7 | Click during pending transition | Transitioning flag + disable clicks + 5s timeout |
| 8 | Thumbnail lazy loading | IntersectionObserver (zero deps, only visible cards) |
| 9 | Empty filter results | "No scenes match" text in grid area |
| 10 | ViewRouter location | Class in main.ts (~40 lines) |
| 11 | SceneSelectIntent home | Exported from gallery_view.ts |
| 12 | Existing smoke test compat | ?scene=X URL param bypasses gallery |
| 13 | Thumbnail canvas memory | Release on sim transition, re-render on gallery return |
| 14 | Static payload file location | frontend/public/static-payloads/{scene_id}.json |
