# UI Gallery & Scene Visibility Implementation Plan

> **Status: HISTORICAL / AS-PLANNED.** This document captures the
> plan that drove PR #47. The final implementation diverged in
> small ways during execution (file paths normalized to
> `frontend/src/tests/unit/`, fault overlay uses
> `#simetro-fault` element check rather than `[data-fault]`
> attribute, scene count is 59 not 52 — 7 new scenes from PR #45
> landed mid-implementation). The merged code is the source of
> truth; this plan is preserved for design-rationale traceability.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make all 52 scenes (especially 21 SL1 scenarios) visible and discoverable in the UI via a gallery-first experience with pre-rendered thumbnails.

**Architecture:** Gallery-first two-view app. Rust CLI (`simetro-headless --emit-static`) pre-generates versioned `StaticPayload` JSON files for all scenes. Frontend fetches these to render mini-canvas thumbnails. Mock transport extended to load per-scene static payloads so browser-mode E2E tests render real scene geometry. ViewRouter in main.ts manages gallery ↔ simulation transitions with monotonic transition tokens to prevent stale-message races.

**Tech Stack:** Rust (clap CLI, engine, protocol), TypeScript (pure DOM, Canvas2D, Vitest, Playwright), Vite (static public assets)

---

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `frontend/src/ui/gallery_view.ts` | Full-page gallery grid: sections, cards, filter bar, IntersectionObserver lazy-load |
| `frontend/src/ui/gallery_card.ts` | Individual card: thumbnail canvas + palette swatch fallback |
| `frontend/src/ui/scene_switcher.ts` | Compact floating pill for simulation view (replaces scene_browser.ts) |
| `frontend/src/ui/thumbnail_renderer.ts` | Renders a `StaticPayload` into a mini offscreen canvas |
| `frontend/src/tests/gallery.test.ts` | Vitest unit tests: filter, router, thumbnail fallback |
| `frontend/src/tests/e2e/gallery.spec.ts` | Playwright E2E: gallery renders, card→sim, sim→gallery |
| `frontend/src/tests/e2e/scene_renders.spec.ts` | Playwright E2E: all 52 scenes render real geometry without errors |
| `frontend/public/static-payloads/.gitkeep` | Placeholder; actual JSON generated at build time |

### Modified Files

| File | Change |
|------|--------|
| `crates/headless/src/main.rs` | Add `EmitStatic` subcommand (~80 lines) |
| `crates/headless/Cargo.toml` | No dep changes needed (already has engine+protocol+serde_json) |
| `frontend/src/main.ts` | Add ViewRouter class, `?scene=X` URL param, remove SceneBrowser import |
| `frontend/src/transport/mock.ts` | Add `loadStaticPayload(sceneId)` for per-scene browser rendering |
| `frontend/src/catalog/scenes.ts` | 21 SL1 entries: `status: "draft"` → `status: "ready"` |
| `frontend/src/tests/e2e/smoke.spec.ts` | Update URLs to use `?scene=demo-paths` |
| `frontend/src/tests/e2e/sl1_hud.spec.ts` | Update URLs to use `?scene=demo-paths&sl1demo=1` |

### Deleted Files

| File | Reason |
|------|--------|
| `frontend/src/ui/scene_browser.ts` | Replaced by gallery_view.ts + scene_switcher.ts |

---

## PR / Task Ordering (revised per rubber-duck review)

```
PR 1: ?scene=X URL param + mock transport per-scene loading
PR 2: simetro-headless --emit-static (Rust build step)
PR 3: Playwright per-scene render verification (quality gate)
PR 4: Promote SL1 scenes to ready (ONLY after render verification green)
PR 5: Gallery View + Thumbnail Renderer + View Routing + Scene Switcher
```

**Rationale:** Scenes are NOT promoted to `ready` until automated rendering
proof exists. The `?scene=X` param + mock transport extension must land first
so Playwright can actually render each scene's real geometry (not just demo-paths).

---

## Task 1: `?scene=X` URL Param + Mock Transport Per-Scene Loading

**Files:**
- Modify: `frontend/src/main.ts:340-360` (createTransport), `frontend/src/main.ts:370-442` (boot)
- Modify: `frontend/src/transport/mock.ts` (add static payload loading)
- Modify: `frontend/src/tests/e2e/smoke.spec.ts`
- Modify: `frontend/src/tests/e2e/sl1_hud.spec.ts`
- Test: `frontend/src/tests/e2e/smoke.spec.ts`

### Context

Currently `createTransport()` always creates a MockTransport with hardcoded demo-paths geometry.
We need: (1) URL param `?scene=X` to specify which scene to load, (2) MockTransport that can
fetch a pre-built StaticPayload for that scene from `/static-payloads/{id}.json`.

- [ ] **Step 1: Add `?scene=X` param parsing to boot()**

In `frontend/src/main.ts`, add param reading before transport creation:

```typescript
// Add after line 359 (end of createTransport), before boot():
function sceneFromLocation(): string | null {
  if (typeof window === "undefined") return null;
  const params = new URLSearchParams(window.location.search);
  return params.get("scene");
}
```

- [ ] **Step 2: Extend MockTransport to accept an initial scene id**

In `frontend/src/transport/mock.ts`, add a `sceneId` option and a method to load external static payload:

```typescript
// Add to MockTransportOptions interface (after sl1Mode):
export interface MockTransportOptions {
  sl1Mode?: boolean;
  /** When set, mock fetches /static-payloads/{sceneId}.json instead of using DEMO_STATIC. */
  sceneId?: string;
}
```

Add a private method to the MockTransport class:

```typescript
private async loadExternalStatic(sceneId: string, handler: MessageHandler): Promise<void> {
  try {
    const resp = await fetch(`/static-payloads/${sceneId}.json`);
    if (!resp.ok) {
      console.warn(`simetro: static payload fetch failed for ${sceneId} (${resp.status}), using demo`);
      handler(DEMO_STATIC);
      return;
    }
    const envelope = await resp.json() as { schema_version: number; payload: StaticPayload };
    if (envelope.schema_version !== SCHEMA_VERSION) {
      console.warn(`simetro: schema mismatch for ${sceneId} (got ${envelope.schema_version}, want ${SCHEMA_VERSION}), using demo`);
      handler(DEMO_STATIC);
      return;
    }
    const msg: SimMessage = { kind: "static", payload: envelope.payload };
    handler(msg);
  } catch (e) {
    console.warn(`simetro: failed to load static for ${sceneId}`, e);
    handler(DEMO_STATIC);
  }
}
```

Modify `connect()` to use this when `sceneId` is set:

```typescript
connect(handler: MessageHandler): void {
  // ... existing interval setup ...
  if (this.sceneId) {
    void this.loadExternalStatic(this.sceneId, handler);
  } else {
    // existing: handler(staticMsg);
  }
  // snapshot loop still uses demo movers for animation (no snapshot data in static-only mode)
}
```

- [ ] **Step 3: Wire `?scene=X` into createTransport**

In `frontend/src/main.ts`, modify `createTransport()` and `boot()`:

```typescript
function createTransport(sceneId: string | null): Transport {
  if (isTauri()) {
    return new TauriTransport();
  }
  const search =
    typeof window !== "undefined" && window.location !== undefined
      ? window.location.search
      : undefined;
  return new MockTransport({
    sl1Mode: sl1ModeFromLocation(search),
    sceneId: sceneId ?? undefined,
  });
}
```

In `boot()`, change line 432:

```typescript
const requestedScene = sceneFromLocation();
const transport: Transport = createTransport(requestedScene);
```

- [ ] **Step 4: Handle invalid `?scene=X` values**

Add validation in boot — if `?scene=X` is provided but not in the catalog, show the gallery (for now, log error since gallery doesn't exist yet):

```typescript
const requestedScene = sceneFromLocation();
if (requestedScene !== null && findSceneById(requestedScene) === undefined) {
  console.error(`simetro: unknown scene "${requestedScene}" in URL param, ignoring`);
  // For now, fall through to default behavior. Gallery will handle this later.
}
const validScene = requestedScene !== null && findSceneById(requestedScene) !== undefined
  ? requestedScene
  : null;
const transport: Transport = createTransport(validScene);
```

- [ ] **Step 5: Update existing E2E tests for explicit scene param**

`frontend/src/tests/e2e/smoke.spec.ts` — change all `page.goto("/")` to `page.goto("/?scene=demo-paths")`:

```typescript
test("canvas is visible", async ({ page }) => {
  await page.goto("/?scene=demo-paths");
  const canvas = page.locator("#scene");
  await expect(canvas).toBeVisible();
});
```

Apply same change to all tests in smoke.spec.ts and sl1_hud.spec.ts (sl1 uses `/?scene=demo-paths&sl1demo=1`).

- [ ] **Step 6: Run existing E2E tests to verify no regression**

Run: `cd frontend && npx playwright test`
Expected: All existing tests pass (they now explicitly request demo-paths, which loads the hardcoded geometry as before).

- [ ] **Step 7: Commit**

```bash
git add frontend/src/main.ts frontend/src/transport/mock.ts frontend/src/tests/e2e/smoke.spec.ts frontend/src/tests/e2e/sl1_hud.spec.ts
git commit -m "feat: add ?scene=X URL param + mock transport per-scene static loading

- Parse ?scene=X from URL, validate against catalog
- MockTransport fetches /static-payloads/{id}.json when sceneId specified
- Versioned envelope check (schema_version must match SCHEMA_VERSION)
- Falls back to demo-paths geometry on fetch failure
- Existing E2E tests updated to use ?scene=demo-paths explicitly"
```

---

## Task 2: `simetro-headless --emit-static` Build Step

**Files:**
- Modify: `crates/headless/src/main.rs` (add EmitStatic subcommand)
- Create: `frontend/public/static-payloads/.gitkeep`
- Test: integration test in `crates/headless/src/main.rs` (or separate test file)

### Context

`encode_static()` in `crates/engine/src/snapshot.rs` already builds a `StaticPayload` from a `LoadedScene`. The headless CLI already has `load()` which returns `LoadedScene`. We just need to serialize and write the output.

The static payload files must include `schema_version` so the frontend can detect drift. We wrap each in an envelope: `{ "schema_version": N, "payload": { ...StaticPayload... } }`.

- [ ] **Step 1: Add EmitStatic subcommand to Cmd enum**

In `crates/headless/src/main.rs`, add after the PolicySearch variant:

```rust
/// Generate StaticPayload JSON for all scenes in a directory.
/// Used as a build step for frontend gallery thumbnails.
EmitStatic {
    /// Directory containing games/*.json scene files.
    #[arg(long, default_value = "games")]
    scenes_dir: PathBuf,
    /// Output directory for static payload JSON files.
    #[arg(long, default_value = "frontend/public/static-payloads")]
    output_dir: PathBuf,
},
```

- [ ] **Step 2: Add the handler in main()**

```rust
Cmd::EmitStatic { scenes_dir, output_dir } => cmd_emit_static(&scenes_dir, &output_dir),
```

- [ ] **Step 3: Implement cmd_emit_static**

```rust
#[derive(Serialize)]
struct StaticEnvelope {
    schema_version: u32,
    payload: simetro_protocol::StaticPayload,
}

fn cmd_emit_static(scenes_dir: &std::path::Path, output_dir: &std::path::Path) -> i32 {
    use simetro_engine::snapshot::encode_static;

    // Create output directory if it doesn't exist.
    if let Err(e) = std::fs::create_dir_all(output_dir) {
        eprintln!("emit-static: failed to create output dir {}: {e}", output_dir.display());
        return 3;
    }

    // Discover all *.json files in scenes_dir.
    let entries = match std::fs::read_dir(scenes_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("emit-static: failed to read scenes dir {}: {e}", scenes_dir.display());
            return 3;
        }
    };

    let mut total = 0u32;
    let mut failed = 0u32;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let scene_id = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        total += 1;
        eprintln!("emit-static: generating {scene_id}...");

        let loaded = match load(&path, DEFAULT_SEED) {
            Ok(v) => v,
            Err(_) => {
                eprintln!("emit-static: FAILED {scene_id}");
                failed += 1;
                continue;
            }
        };

        let static_payload = encode_static(&loaded);
        let envelope = StaticEnvelope {
            schema_version: simetro_protocol::SCHEMA_VERSION,
            payload: static_payload,
        };

        let out_path = output_dir.join(format!("{scene_id}.json"));
        let json = match serde_json::to_string(&envelope) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("emit-static: serialize failed for {scene_id}: {e}");
                failed += 1;
                continue;
            }
        };

        if let Err(e) = std::fs::write(&out_path, json) {
            eprintln!("emit-static: write failed for {scene_id}: {e}");
            failed += 1;
            continue;
        }
    }

    eprintln!("emit-static: {}/{total} payloads generated ({failed} failed)", total - failed);
    if failed > 0 { 1 } else { 0 }
}
```

- [ ] **Step 4: Ensure `encode_static` is publicly exported from engine**

Check `crates/engine/src/lib.rs` exports `snapshot::encode_static`. If not, add:

```rust
pub use snapshot::encode_static;
```

- [ ] **Step 5: Create the .gitkeep and add static-payloads/ to .gitignore**

```bash
mkdir -p frontend/public/static-payloads
touch frontend/public/static-payloads/.gitkeep
echo "frontend/public/static-payloads/*.json" >> .gitignore
```

The `.gitkeep` ensures the directory exists in git. The `*.json` ignore ensures generated payloads are never committed.

- [ ] **Step 6: Build and run emit-static locally**

```bash
cargo build -p simetro-headless
./target/debug/simetro-headless emit-static --scenes-dir games --output-dir frontend/public/static-payloads
```

Expected output:
```
emit-static: generating demo-paths...
emit-static: generating metro-pulse...
...
emit-static: 52/52 payloads generated (0 failed)
```

- [ ] **Step 7: Verify output format**

```bash
cat frontend/public/static-payloads/demo-paths.json | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['schema_version'], d['payload']['name'], len(d['payload']['nodes']))"
```

Expected: `1 demo-paths 3`

- [ ] **Step 8: Add integration test**

In `crates/headless/src/main.rs` (at bottom, in `#[cfg(test)]` module):

```rust
#[cfg(test)]
mod emit_static_tests {
    use super::*;
    use std::fs;

    #[test]
    fn emit_static_generates_valid_payloads() {
        let tmp = tempfile::tempdir().unwrap();
        let out_dir = tmp.path().join("out");
        let code = cmd_emit_static(std::path::Path::new("../../games"), &out_dir);
        assert_eq!(code, 0, "emit-static should succeed for all scenes");

        // Check at least one known scene exists.
        let demo = out_dir.join("demo-paths.json");
        assert!(demo.exists(), "demo-paths.json should be generated");

        let content = fs::read_to_string(&demo).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(envelope["schema_version"], simetro_protocol::SCHEMA_VERSION);
        assert_eq!(envelope["payload"]["name"], "demo-paths");
        assert!(envelope["payload"]["nodes"].as_array().unwrap().len() > 0);
    }
}
```

- [ ] **Step 9: Run tests**

```bash
cargo test -p simetro-headless -- emit_static
```

Expected: PASS

- [ ] **Step 10: Commit**

```bash
git add crates/headless/src/main.rs frontend/public/static-payloads/.gitkeep .gitignore
git commit -m "feat: add simetro-headless --emit-static for gallery thumbnails

Generates versioned StaticPayload JSON for all scenes in games/ into
frontend/public/static-payloads/. Each file wraps the payload in an
envelope with schema_version for frontend drift detection.

Exit code 1 if any scene fails to load (CI gate).
Generated files are gitignored — CI builds them fresh."
```

---

## Task 3: Playwright Per-Scene Render Verification

**Files:**
- Create: `frontend/src/tests/e2e/scene_renders.spec.ts`
- Modify: `frontend/playwright.config.ts` (add webServer build step or ensure static payloads exist)

### Context

Now that Task 1 gave MockTransport per-scene static loading and Task 2 generates the payloads,
Playwright can verify each scene renders real geometry. The test uses `?scene=X` to load each scene.
MockTransport will fetch `/static-payloads/{id}.json` and render that scene's real nodes/paths.

Note: Snapshot-driven movers won't animate (static-only mode), but the static geometry (nodes + paths)
will render correctly — which is what we need to verify scenes load without error.

- [ ] **Step 1: Generate static payloads before tests run**

Add a CI step / local workflow. For local dev, document:

```bash
cargo build -p simetro-headless && ./target/debug/simetro-headless emit-static
cd frontend && npm run build  # vite build bundles public/ assets
```

- [ ] **Step 2: Create scene_renders.spec.ts**

```typescript
// frontend/src/tests/e2e/scene_renders.spec.ts
//
// Per-scene render verification. Quality gate: every scene in the catalog
// must load its real StaticPayload and render without fault overlays.
// Requires: simetro-headless --emit-static has been run before npm run build.

import { test, expect } from "@playwright/test";

// Import catalog scene ids at test-generation time.
// Playwright runs against the built bundle, so we hardcode the list.
const SCENE_IDS = [
  "demo-paths",
  "metro-pulse",
  "cargo-loom",
  "factory-line-seeds",
  "garden-pollinators",
  "data-packet-city",
  "emergency-dispatch",
  "power-grid-balancer",
  "river-ferries",
  "night-market-runners",
  "orbital-transfers",
  "gpu-launch-week",
  "airport-ground-stop",
  "archive-index-table",
  "autonomous-farm-season",
  "bakery-oven-shift",
  "bioreactor-balance",
  "chip-fab-yield-crisis",
  "circuit-garden",
  "city-budget-war-room",
  "clinic-triage-desk",
  "crystal-growth-rig",
  "datacenter-cooling-surge",
  "deep-sea-habitat-grid",
  "disaster-supply-staging",
  "drone-repair-bay",
  "fabric-dye-lab",
  "food-bank-allocation",
  "forge-heat-map",
  "fusion-shot-campaign",
  "greenhouse-water-watch",
  "hospital-bed-command",
  "kitchen-prep-board",
  "library-reshelving-clock",
  "microgrid-starter",
  "museum-conservation-bench",
  "observatory-night-queue",
  "pandemic-supply-web",
  "planetary-defense-array",
  "quantum-control-room",
  "recycling-sort-floor",
  "reef-nursery",
  "regional-blackstart",
  "robot-arm-workbench",
  "satellite-downlink-window",
  "security-alert-fusion",
  "seed-bank-vault",
  "sensor-calibration-lab",
  "stormwater-pump-room",
  "warehouse-cold-chain",
  "weather-balloon-yard",
  "wildfire-watch-grid",
];

test.describe("per-scene render verification", () => {
  for (const sceneId of SCENE_IDS) {
    test(`scene "${sceneId}" renders without error`, async ({ page }) => {
      await page.goto(`/?scene=${sceneId}`);
      const canvas = page.locator("#scene");
      await expect(canvas).toBeVisible({ timeout: 5000 });

      // Wait for the mock transport to emit static + first frame to paint.
      await page.waitForTimeout(300);

      // No fault overlay should be visible.
      // (Note: actual implementation uses `#simetro-fault` element
      // with `style.display === "none"` when hidden — see
      // `frontend/src/tests/e2e/scene_renders.spec.ts`.)
      const faultCount = await page.locator("[data-fault]").count();
      expect(faultCount).toBe(0);

      // Canvas should have rendered content (not just background).
      const hasContent = await page.evaluate(() => {
        const c = document.getElementById("scene") as HTMLCanvasElement | null;
        if (!c) return false;
        const ctx = c.getContext("2d");
        if (!ctx) return false;
        // Sample multiple points. If ALL are background color, scene didn't render.
        const dpr = window.devicePixelRatio || 1;
        const w = c.width / dpr;
        const h = c.height / dpr;
        // Sample 5 evenly spaced points across canvas.
        const points = [
          [w * 0.25, h * 0.25],
          [w * 0.5, h * 0.5],
          [w * 0.75, h * 0.25],
          [w * 0.25, h * 0.75],
          [w * 0.75, h * 0.75],
        ];
        let nonBgCount = 0;
        for (const [x, y] of points) {
          const px = ctx.getImageData(Math.floor(x * dpr), Math.floor(y * dpr), 1, 1).data;
          // Background is typically dark (#0e1116 = 14, 17, 22).
          if (px[0] !== 14 || px[1] !== 17 || px[2] !== 22) {
            nonBgCount++;
          }
        }
        return nonBgCount > 0;
      });
      expect(hasContent).toBe(true);
    });
  }
});
```

- [ ] **Step 3: Run the per-scene tests**

```bash
cd frontend && npx playwright test scene_renders
```

Expected: All 52 tests pass (each scene's static payload loads and renders nodes/paths).

- [ ] **Step 4: Commit**

```bash
git add frontend/src/tests/e2e/scene_renders.spec.ts
git commit -m "test: add Playwright per-scene render verification for all 52 scenes

Each scene loads via ?scene=X, mock transport fetches its real
StaticPayload from /static-payloads/, and the test verifies:
- No fault overlay appears
- Canvas has non-background pixels (geometry rendered)

This is the quality gate before promoting SL1 scenes to ready."
```

---

## Task 4: Promote SL1 Scenes to Ready

**Files:**
- Modify: `frontend/src/catalog/scenes.ts` (21 entries)
- Test: `frontend/src/tests/gallery.test.ts` (new unit test)

### Context

Now that Task 3 proves all scenes render correctly, we can safely promote
the 21 SL1 scenarios from `draft` to `ready`.

- [ ] **Step 1: Write the unit test first**

Create `frontend/src/tests/unit/gallery.test.ts` (note: actual path
under `unit/` per vitest config; the implementation uses this path):

```typescript
// frontend/src/tests/unit/gallery.test.ts
import { describe, test, expect } from "vitest";
import { SCENE_CATALOG } from "../../catalog/scenes";

describe("scene catalog", () => {
  test("no SL1 scenes remain in draft status", () => {
    const draftSl1 = SCENE_CATALOG.filter(
      (s) => s.world_kind === "sl1_scenario" && s.status === "draft"
    );
    expect(draftSl1).toEqual([]);
  });

  test("all 52 scenes are present", () => {
    expect(SCENE_CATALOG.length).toBe(52);
  });

  test("all SL1 scenes are ready", () => {
    const sl1Scenes = SCENE_CATALOG.filter((s) => s.world_kind === "sl1_scenario");
    expect(sl1Scenes.length).toBe(21);
    for (const scene of sl1Scenes) {
      expect(scene.status).toBe("ready");
    }
  });
});
```

- [ ] **Step 2: Run test — should FAIL (SL1 scenes still draft)**

```bash
cd frontend && npm test -- --run src/tests/gallery.test.ts
```

Expected: FAIL — "no SL1 scenes remain in draft" fails because 21 are still draft.

- [ ] **Step 3: Promote all 21 SL1 scenes**

In `frontend/src/catalog/scenes.ts`, find every entry with `world_kind: "sl1_scenario"` and change `status: "draft"` to `status: "ready"`. These are the scene ids to change:

```
gpu-launch-week, airport-ground-stop, archive-index-table,
autonomous-farm-season, bakery-oven-shift, bioreactor-balance,
chip-fab-yield-crisis, circuit-garden, city-budget-war-room,
clinic-triage-desk, crystal-growth-rig, datacenter-cooling-surge,
deep-sea-habitat-grid, disaster-supply-staging, drone-repair-bay,
fabric-dye-lab, food-bank-allocation, forge-heat-map,
fusion-shot-campaign, greenhouse-water-watch, hospital-bed-command
```

For each, change:
```typescript
status: "draft",
```
to:
```typescript
status: "ready",
```

- [ ] **Step 4: Run test — should PASS**

```bash
cd frontend && npm test -- --run src/tests/gallery.test.ts
```

Expected: PASS

- [ ] **Step 5: Run full frontend validation**

```bash
cd frontend && npm run typecheck && npm run lint && npm test -- --run
```

Expected: All pass.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/catalog/scenes.ts frontend/src/tests/gallery.test.ts
git commit -m "feat: promote all 21 SL1 scenes from draft to ready

All SL1 scenarios now visible in the UI. This change is gated by
per-scene Playwright render verification proving every scene loads
and renders correctly without human intervention."
```

---

## Task 5: Gallery View + Thumbnail Renderer

**Files:**
- Create: `frontend/src/ui/gallery_view.ts`
- Create: `frontend/src/ui/gallery_card.ts`
- Create: `frontend/src/ui/thumbnail_renderer.ts`
- Test: `frontend/src/tests/gallery.test.ts` (extend)

### Context

Gallery is a full-page grid of scene cards. Cards render thumbnails from pre-built
StaticPayload files. Uses IntersectionObserver for lazy loading. Palette swatch fallback
on any failure.

- [ ] **Step 1: Create thumbnail_renderer.ts**

```typescript
// frontend/src/ui/thumbnail_renderer.ts
//
// Renders a StaticPayload into a mini offscreen canvas for gallery thumbnails.
// Reuses the same drawing logic as the main Renderer but at thumbnail scale.

import type { StaticPayload, NodeView } from "../protocol/messages";

const THUMB_NODE_RADIUS = 6;
const THUMB_PATH_WIDTH = 2;

export function renderThumbnail(
  payload: StaticPayload,
  width: number,
  height: number
): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (ctx === null) {
    throw new Error("Canvas2D unavailable for thumbnail");
  }

  // Background.
  const bgColor = payload.palette[payload.background_index] ?? "#0e1116";
  ctx.fillStyle = bgColor;
  ctx.fillRect(0, 0, width, height);

  // Compute bounding box of all geometry to fit into thumbnail.
  const positions: [number, number][] = [];
  for (const n of payload.nodes) positions.push(n.pos);
  for (const p of payload.paths) {
    positions.push(p.from_pos);
    positions.push(p.to_pos);
  }

  if (positions.length === 0) return canvas;

  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const [x, y] of positions) {
    if (x < minX) minX = x;
    if (y < minY) minY = y;
    if (x > maxX) maxX = x;
    if (y > maxY) maxY = y;
  }

  const padding = 20;
  const sceneW = maxX - minX || 1;
  const sceneH = maxY - minY || 1;
  const scale = Math.min((width - padding * 2) / sceneW, (height - padding * 2) / sceneH);
  const offsetX = (width - sceneW * scale) / 2 - minX * scale;
  const offsetY = (height - sceneH * scale) / 2 - minY * scale;

  function tx(x: number): number { return x * scale + offsetX; }
  function ty(y: number): number { return y * scale + offsetY; }

  // Draw paths.
  ctx.lineWidth = THUMB_PATH_WIDTH;
  for (const p of payload.paths) {
    ctx.strokeStyle = payload.palette[p.color] ?? "#555";
    ctx.beginPath();
    ctx.moveTo(tx(p.from_pos[0]), ty(p.from_pos[1]));
    ctx.lineTo(tx(p.to_pos[0]), ty(p.to_pos[1]));
    ctx.stroke();
  }

  // Draw nodes.
  for (const n of payload.nodes) {
    ctx.fillStyle = payload.palette[n.color] ?? "#aaa";
    ctx.beginPath();
    ctx.arc(tx(n.pos[0]), ty(n.pos[1]), THUMB_NODE_RADIUS, 0, Math.PI * 2);
    ctx.fill();
  }

  return canvas;
}

/** Create a palette swatch fallback when static payload loading fails. */
export function renderPaletteSwatch(
  palette: string[],
  width: number,
  height: number
): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (ctx === null) return canvas;

  const bgColor = palette[0] ?? "#0e1116";
  ctx.fillStyle = bgColor;
  ctx.fillRect(0, 0, width, height);

  // Draw palette colors as horizontal stripes.
  const stripeH = height / Math.max(palette.length, 1);
  for (let i = 1; i < palette.length; i++) {
    ctx.fillStyle = palette[i];
    ctx.fillRect(width * 0.2, stripeH * i, width * 0.6, stripeH * 0.6);
  }

  return canvas;
}
```

- [ ] **Step 2: Create gallery_card.ts**

```typescript
// frontend/src/ui/gallery_card.ts
//
// Individual scene card for the gallery grid. Renders a thumbnail from
// a pre-built StaticPayload, with palette swatch fallback on failure.

import type { SceneCatalogEntry } from "../catalog/scenes";
import type { StaticPayload } from "../protocol/messages";
import { SCHEMA_VERSION } from "../protocol/messages";
import { renderThumbnail, renderPaletteSwatch } from "./thumbnail_renderer";

const THUMB_WIDTH = 320;
const THUMB_HEIGHT = 180;

const DIFFICULTY_COLORS: Record<string, string> = {
  intro: "#4ade80",
  easy: "#60a5fa",
  medium: "#fbbf24",
  hard: "#f87171",
};

export class GalleryCard {
  readonly element: HTMLElement;
  private thumbContainer: HTMLElement;
  private loaded = false;

  constructor(
    private readonly scene: SceneCatalogEntry,
    private readonly onClick: () => void
  ) {
    this.element = document.createElement("button");
    this.element.type = "button";
    this.element.style.cssText = `
      display: flex; flex-direction: column; border: 1px solid #30363d;
      border-radius: 8px; overflow: hidden; background: #161b22;
      cursor: pointer; padding: 0; text-align: left; width: 100%;
      transition: transform 0.15s, border-color 0.15s;
    `;
    this.element.addEventListener("mouseenter", () => {
      this.element.style.transform = "scale(1.02)";
      this.element.style.borderColor = "#58a6ff";
    });
    this.element.addEventListener("mouseleave", () => {
      this.element.style.transform = "scale(1)";
      this.element.style.borderColor = "#30363d";
    });
    this.element.addEventListener("click", this.onClick);

    // Thumbnail container.
    this.thumbContainer = document.createElement("div");
    this.thumbContainer.style.cssText = `
      width: 100%; aspect-ratio: 16/9; background: #0e1116; position: relative;
    `;
    this.element.appendChild(this.thumbContainer);

    // Text content.
    const info = document.createElement("div");
    info.style.cssText = "padding: 12px;";

    const title = document.createElement("div");
    title.style.cssText = "font-weight: 600; color: #e6edf3; font-size: 14px; margin-bottom: 4px;";
    title.textContent = scene.title;
    info.appendChild(title);

    const subtitle = document.createElement("div");
    subtitle.style.cssText = "color: #8b949e; font-size: 12px; margin-bottom: 8px;";
    subtitle.textContent = scene.subtitle;
    info.appendChild(subtitle);

    // Difficulty pill.
    const pill = document.createElement("span");
    pill.style.cssText = `
      display: inline-block; padding: 2px 8px; border-radius: 12px;
      font-size: 11px; font-weight: 500;
      background: ${DIFFICULTY_COLORS[scene.difficulty] ?? "#555"}22;
      color: ${DIFFICULTY_COLORS[scene.difficulty] ?? "#aaa"};
    `;
    pill.textContent = scene.difficulty;
    info.appendChild(pill);

    this.element.appendChild(info);
  }

  /** Load and render thumbnail. Call when card becomes visible (IntersectionObserver). */
  async loadThumbnail(): Promise<void> {
    if (this.loaded) return;
    this.loaded = true;

    try {
      const resp = await fetch(`/static-payloads/${this.scene.id}.json`);
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      const envelope = await resp.json() as { schema_version: number; payload: StaticPayload };
      if (envelope.schema_version !== SCHEMA_VERSION) {
        throw new Error(`schema mismatch: ${envelope.schema_version}`);
      }
      const canvas = renderThumbnail(envelope.payload, THUMB_WIDTH, THUMB_HEIGHT);
      canvas.style.cssText = "width: 100%; height: 100%; object-fit: cover;";
      this.thumbContainer.appendChild(canvas);
    } catch (e) {
      console.warn(`simetro: thumbnail fallback for ${this.scene.id}:`, e);
      const fallback = renderPaletteSwatch(
        this.scene.palette_name === "simetro_dark"
          ? ["#0e1116", "#e8eaed", "#7aa2f7", "#bb9af7", "#9ece6a"]
          : ["#0e1116", "#e8eaed", "#7aa2f7"],
        THUMB_WIDTH,
        THUMB_HEIGHT
      );
      fallback.style.cssText = "width: 100%; height: 100%;";
      this.thumbContainer.appendChild(fallback);
    }
  }

  /** Release canvas GPU memory (call when leaving gallery view). */
  releaseMemory(): void {
    const canvas = this.thumbContainer.querySelector("canvas");
    if (canvas) {
      canvas.width = 0;
      canvas.height = 0;
    }
    this.loaded = false;
  }
}
```

- [ ] **Step 3: Create gallery_view.ts**

```typescript
// frontend/src/ui/gallery_view.ts
//
// Full-page gallery view. Sections: "SL1 Scenarios" then "Transit Loops".
// Sorted by difficulty within each section. IntersectionObserver lazy-loads thumbnails.

import type { SceneCatalogEntry, SceneDifficulty, SceneWorldKind } from "../catalog/scenes";
import { GalleryCard } from "./gallery_card";

export interface SceneSelectIntent {
  readonly kind: "SelectScene";
  readonly scene_id: string;
}

export type SceneSelectHandler = (intent: SceneSelectIntent) => void;

export interface GalleryFilter {
  world_kind: "all" | SceneWorldKind;
  difficulty: "all" | SceneDifficulty;
}

const DIFFICULTY_ORDER: Record<string, number> = { intro: 0, easy: 1, medium: 2, hard: 3 };

export class GalleryView {
  private root: HTMLElement;
  private grid: HTMLElement;
  private cards: GalleryCard[] = [];
  private observer: IntersectionObserver;
  private filter: GalleryFilter = { world_kind: "all", difficulty: "all" };
  private scenes: readonly SceneCatalogEntry[];

  constructor(
    container: HTMLElement,
    scenes: readonly SceneCatalogEntry[],
    private readonly onSelect: SceneSelectHandler
  ) {
    this.scenes = scenes;
    this.root = document.createElement("div");
    this.root.id = "simetro-gallery";
    this.root.style.cssText = `
      position: fixed; inset: 0; z-index: 1000; background: #0e1116;
      overflow-y: auto; display: none; padding: 40px;
    `;

    // Header.
    const header = document.createElement("div");
    header.style.cssText = "max-width: 1200px; margin: 0 auto 24px; display: flex; align-items: center; gap: 16px;";
    const title = document.createElement("h1");
    title.style.cssText = "color: #e6edf3; font-size: 24px; margin: 0;";
    title.textContent = "simetro";
    header.appendChild(title);

    // Filter chips.
    const chips = document.createElement("div");
    chips.style.cssText = "display: flex; gap: 8px;";
    for (const kind of ["all", "sl1_scenario", "transit_loop"] as const) {
      const chip = document.createElement("button");
      chip.type = "button";
      chip.style.cssText = `
        padding: 4px 12px; border-radius: 16px; border: 1px solid #30363d;
        background: ${kind === "all" ? "#21262d" : "transparent"};
        color: #e6edf3; font-size: 12px; cursor: pointer;
      `;
      chip.textContent = kind === "all" ? "All" : kind === "sl1_scenario" ? "SL1 Scenarios" : "Transit Loops";
      chip.addEventListener("click", () => {
        this.setFilter({ ...this.filter, world_kind: kind });
        // Update chip styles.
        for (const c of chips.children) {
          (c as HTMLElement).style.background = "transparent";
        }
        chip.style.background = "#21262d";
      });
      chips.appendChild(chip);
    }
    header.appendChild(chips);
    this.root.appendChild(header);

    // Grid.
    this.grid = document.createElement("div");
    this.grid.style.cssText = `
      max-width: 1200px; margin: 0 auto;
      display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
      gap: 16px;
    `;
    this.root.appendChild(this.grid);

    container.appendChild(this.root);

    // IntersectionObserver for lazy thumbnail loading.
    this.observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            const card = this.cards.find((c) => c.element === entry.target);
            if (card) void card.loadThumbnail();
          }
        }
      },
      { root: this.root, rootMargin: "200px" }
    );

    this.render();
  }

  show(): void {
    this.root.style.display = "block";
    // Re-observe cards for lazy loading.
    for (const card of this.cards) {
      this.observer.observe(card.element);
    }
  }

  hide(): void {
    this.root.style.display = "none";
    this.observer.disconnect();
  }

  /** Release all thumbnail canvas memory (call when entering sim view). */
  releaseMemory(): void {
    for (const card of this.cards) card.releaseMemory();
  }

  setFilter(filter: GalleryFilter): void {
    this.filter = filter;
    this.render();
  }

  private render(): void {
    // Clear existing cards.
    this.observer.disconnect();
    this.grid.innerHTML = "";
    this.cards = [];

    const filtered = this.scenes.filter((s) => {
      if (s.status !== "ready") return false;
      if (this.filter.world_kind !== "all" && s.world_kind !== this.filter.world_kind) return false;
      if (this.filter.difficulty !== "all" && s.difficulty !== this.filter.difficulty) return false;
      return true;
    });

    // Sort: SL1 first, then transit; within each group sort by difficulty.
    const sorted = [...filtered].sort((a, b) => {
      const kindOrder = a.world_kind === "sl1_scenario" ? 0 : 1;
      const kindOrderB = b.world_kind === "sl1_scenario" ? 0 : 1;
      if (kindOrder !== kindOrderB) return kindOrder - kindOrderB;
      return (DIFFICULTY_ORDER[a.difficulty] ?? 0) - (DIFFICULTY_ORDER[b.difficulty] ?? 0);
    });

    if (sorted.length === 0) {
      const empty = document.createElement("div");
      empty.style.cssText = "color: #8b949e; text-align: center; padding: 40px; grid-column: 1/-1;";
      empty.textContent = "No scenes match the current filter.";
      this.grid.appendChild(empty);
      return;
    }

    // Section headers.
    let currentKind: string | null = null;
    for (const scene of sorted) {
      if (scene.world_kind !== currentKind && this.filter.world_kind === "all") {
        currentKind = scene.world_kind;
        const sectionHeader = document.createElement("div");
        sectionHeader.style.cssText = "grid-column: 1/-1; color: #8b949e; font-size: 14px; font-weight: 600; margin-top: 16px; padding-bottom: 8px; border-bottom: 1px solid #21262d;";
        sectionHeader.textContent = currentKind === "sl1_scenario" ? "SL1 Scenarios" : "Transit Loops";
        this.grid.appendChild(sectionHeader);
      }

      const card = new GalleryCard(scene, () => {
        this.onSelect({ kind: "SelectScene", scene_id: scene.id });
      });
      this.cards.push(card);
      this.grid.appendChild(card.element);
    }

    // Start observing if visible.
    if (this.root.style.display !== "none") {
      for (const card of this.cards) {
        this.observer.observe(card.element);
      }
    }
  }
}
```

- [ ] **Step 4: Add unit tests for filter and sort logic**

Extend `frontend/src/tests/gallery.test.ts`:

```typescript
import { describe, test, expect } from "vitest";
import { SCENE_CATALOG } from "../catalog/scenes";

describe("scene catalog", () => {
  test("no SL1 scenes remain in draft status", () => {
    const draftSl1 = SCENE_CATALOG.filter(
      (s) => s.world_kind === "sl1_scenario" && s.status === "draft"
    );
    expect(draftSl1).toEqual([]);
  });

  test("all 52 scenes are present", () => {
    expect(SCENE_CATALOG.length).toBe(52);
  });

  test("all SL1 scenes are ready", () => {
    const sl1Scenes = SCENE_CATALOG.filter((s) => s.world_kind === "sl1_scenario");
    expect(sl1Scenes.length).toBe(21);
    for (const scene of sl1Scenes) {
      expect(scene.status).toBe("ready");
    }
  });

  test("ready scenes include both world kinds", () => {
    const ready = SCENE_CATALOG.filter((s) => s.status === "ready");
    const kinds = new Set(ready.map((s) => s.world_kind));
    expect(kinds.has("sl1_scenario")).toBe(true);
    expect(kinds.has("transit_loop")).toBe(true);
  });
});

describe("gallery filter logic", () => {
  const ready = SCENE_CATALOG.filter((s) => s.status === "ready");

  test("filter by sl1_scenario returns only SL1", () => {
    const filtered = ready.filter((s) => s.world_kind === "sl1_scenario");
    expect(filtered.length).toBe(21);
    for (const s of filtered) {
      expect(s.world_kind).toBe("sl1_scenario");
    }
  });

  test("filter by transit_loop returns only transit", () => {
    const filtered = ready.filter((s) => s.world_kind === "transit_loop");
    expect(filtered.length).toBe(31);
  });

  test("filter by difficulty=hard returns subset", () => {
    const filtered = ready.filter((s) => s.difficulty === "hard");
    expect(filtered.length).toBeGreaterThan(0);
    expect(filtered.length).toBeLessThan(ready.length);
  });
});
```

- [ ] **Step 5: Run tests**

```bash
cd frontend && npm run typecheck && npm test -- --run
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add frontend/src/ui/gallery_view.ts frontend/src/ui/gallery_card.ts frontend/src/ui/thumbnail_renderer.ts frontend/src/tests/gallery.test.ts
git commit -m "feat: gallery view with thumbnail renderer and lazy loading

- GalleryView: full-page grid, sections, filter chips, IntersectionObserver
- GalleryCard: thumbnail canvas with palette swatch fallback on error
- ThumbnailRenderer: renders StaticPayload into mini-canvas
- All text rendered via textContent (XSS-safe)
- Unit tests for filter and catalog validation"
```

---

## Task 6: View Routing + Scene Switcher + Delete Scene Browser

**Files:**
- Modify: `frontend/src/main.ts` (ViewRouter class, transition logic, remove SceneBrowser)
- Create: `frontend/src/ui/scene_switcher.ts`
- Delete: `frontend/src/ui/scene_browser.ts`
- Create: `frontend/src/tests/e2e/gallery.spec.ts`

### Context

ViewRouter manages the gallery ↔ simulation state machine. Transition tokens prevent
stale messages from corrupting state. Transport connects on sim entry, disconnects on return.

- [ ] **Step 1: Create scene_switcher.ts**

```typescript
// frontend/src/ui/scene_switcher.ts
//
// Compact floating pill for simulation view. Shows current scene title +
// prev/next arrows + gallery button. Auto-hides after 3s inactivity.

import type { SceneCatalogEntry } from "../catalog/scenes";

export interface SwitcherHandler {
  onPrev(): void;
  onNext(): void;
  onGallery(): void;
}

export class SceneSwitcher {
  private root: HTMLElement;
  private titleEl: HTMLElement;
  private hideTimer: number | null = null;
  private scenes: readonly SceneCatalogEntry[];
  private currentIndex = 0;

  constructor(
    parent: HTMLElement,
    scenes: readonly SceneCatalogEntry[],
    private readonly handler: SwitcherHandler
  ) {
    this.scenes = scenes.filter((s) => s.status === "ready");

    this.root = document.createElement("div");
    this.root.id = "simetro-switcher";
    this.root.style.cssText = `
      position: fixed; top: 12px; right: 12px; z-index: 900;
      display: none; align-items: center; gap: 8px;
      background: #161b22ee; border: 1px solid #30363d;
      border-radius: 20px; padding: 6px 12px;
      font-size: 13px; color: #e6edf3;
      transition: opacity 0.3s;
    `;

    const prevBtn = document.createElement("button");
    prevBtn.type = "button";
    prevBtn.textContent = "◀";
    prevBtn.style.cssText = "background: none; border: none; color: #e6edf3; cursor: pointer; font-size: 14px;";
    prevBtn.addEventListener("click", () => this.handler.onPrev());
    this.root.appendChild(prevBtn);

    this.titleEl = document.createElement("span");
    this.titleEl.style.cssText = "max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;";
    this.root.appendChild(this.titleEl);

    const nextBtn = document.createElement("button");
    nextBtn.type = "button";
    nextBtn.textContent = "▶";
    nextBtn.style.cssText = "background: none; border: none; color: #e6edf3; cursor: pointer; font-size: 14px;";
    nextBtn.addEventListener("click", () => this.handler.onNext());
    this.root.appendChild(nextBtn);

    const galleryBtn = document.createElement("button");
    galleryBtn.type = "button";
    galleryBtn.textContent = "⊞";
    galleryBtn.title = "Gallery";
    galleryBtn.style.cssText = "background: none; border: none; color: #8b949e; cursor: pointer; font-size: 16px; margin-left: 8px;";
    galleryBtn.addEventListener("click", () => this.handler.onGallery());
    this.root.appendChild(galleryBtn);

    parent.appendChild(this.root);

    // Show on mouse near top-right.
    document.addEventListener("mousemove", (e) => {
      if (this.root.style.display === "none") return;
      if (e.clientX > window.innerWidth - 300 && e.clientY < 80) {
        this.root.style.opacity = "1";
        this.resetHideTimer();
      }
    });
  }

  show(): void {
    this.root.style.display = "flex";
    this.root.style.opacity = "1";
    this.resetHideTimer();
  }

  hide(): void {
    this.root.style.display = "none";
    if (this.hideTimer !== null) {
      clearTimeout(this.hideTimer);
      this.hideTimer = null;
    }
  }

  setSelected(sceneId: string): void {
    const idx = this.scenes.findIndex((s) => s.id === sceneId);
    if (idx >= 0) {
      this.currentIndex = idx;
      this.titleEl.textContent = this.scenes[idx].title;
    }
  }

  getAdjacentId(direction: "prev" | "next"): string | null {
    if (this.scenes.length === 0) return null;
    const newIdx = direction === "prev"
      ? (this.currentIndex - 1 + this.scenes.length) % this.scenes.length
      : (this.currentIndex + 1) % this.scenes.length;
    return this.scenes[newIdx].id;
  }

  private resetHideTimer(): void {
    if (this.hideTimer !== null) clearTimeout(this.hideTimer);
    this.hideTimer = window.setTimeout(() => {
      this.root.style.opacity = "0.3";
    }, 3000);
  }
}
```

- [ ] **Step 2: Add ViewRouter class to main.ts**

Add this class inside `main.ts` (before `boot()`):

```typescript
type ViewState = "gallery" | "simulation";

class ViewRouter {
  state: ViewState = "gallery";
  transitioning = false;
  /** Monotonic token. Late messages with a stale token are ignored. */
  private transitionToken = 0;
  private transitionTimeout: number | null = null;

  beginTransition(): number {
    this.transitioning = true;
    this.transitionToken++;
    // Safety timeout: if transition doesn't complete in 5s, clear flag.
    if (this.transitionTimeout !== null) clearTimeout(this.transitionTimeout);
    this.transitionTimeout = window.setTimeout(() => {
      this.transitioning = false;
      this.transitionTimeout = null;
    }, 5000);
    return this.transitionToken;
  }

  completeTransition(token: number): boolean {
    if (token !== this.transitionToken) return false; // Stale.
    this.transitioning = false;
    if (this.transitionTimeout !== null) {
      clearTimeout(this.transitionTimeout);
      this.transitionTimeout = null;
    }
    return true;
  }

  cancelTransition(token: number): void {
    if (token !== this.transitionToken) return;
    this.transitioning = false;
    if (this.transitionTimeout !== null) {
      clearTimeout(this.transitionTimeout);
      this.transitionTimeout = null;
    }
  }
}
```

- [ ] **Step 3: Wire ViewRouter into boot()**

Modify `boot()` in main.ts:

```typescript
function boot(): void {
  const canvas = document.getElementById("scene");
  if (!(canvas instanceof HTMLCanvasElement)) {
    console.error("scene canvas missing");
    return;
  }
  resize(canvas);

  const renderer = new Renderer(canvas);
  renderer.warm(DEFAULT_THEME);
  const state = createAppState();
  const router = new ViewRouter();

  const requestedScene = sceneFromLocation();
  // Validate requested scene.
  const validScene = requestedScene !== null && findSceneById(requestedScene) !== undefined
    ? requestedScene
    : null;

  const appRoot = document.getElementById("app");
  if (appRoot !== null) {
    // ... existing UI setup (inspector, hover, fault, warnings, etc.) ...
    
    // Gallery view (replaces scene browser).
    const gallery = new GalleryView(
      appRoot,
      SCENE_CATALOG,
      (intent) => {
        if (router.transitioning) return; // Ignore during transition.
        transitionToSim(intent.scene_id);
      }
    );

    // Scene switcher (compact pill).
    const readyScenes = SCENE_CATALOG.filter(s => s.status === "ready");
    const switcher = new SceneSwitcher(appRoot, readyScenes, {
      onPrev() {
        const id = switcher.getAdjacentId("prev");
        if (id && !router.transitioning) transitionToSim(id);
      },
      onNext() {
        const id = switcher.getAdjacentId("next");
        if (id && !router.transitioning) transitionToSim(id);
      },
      onGallery() {
        transitionToGallery();
      },
    });

    function transitionToSim(sceneId: string): void {
      const token = router.beginTransition();
      router.state = "simulation";
      gallery.hide();
      gallery.releaseMemory();
      switcher.show();
      switcher.setSelected(sceneId);
      canvas!.style.display = "block";
      state.selectedSceneId = sceneId;

      if (isTauri()) {
        void routeSceneToTauri(sceneId, null, state).then(() => {
          router.completeTransition(token);
        }).catch(() => {
          router.cancelTransition(token);
          transitionToGallery();
          state.fault?.show({
            kind: "load_error",
            message: `Failed to load scene: ${sceneId}`,
            line: null, col: null,
          });
        });
      } else {
        // Browser mode: transport already loaded via ?scene= or mock.
        router.completeTransition(token);
      }
    }

    function transitionToGallery(): void {
      router.state = "gallery";
      router.transitioning = false;
      switcher.hide();
      canvas!.style.display = "none";
      gallery.show();
      console.info("simetro: returning to gallery");
    }

    // Initial state: gallery or simulation?
    if (validScene !== null) {
      // ?scene=X: skip gallery, go straight to sim.
      router.state = "simulation";
      switcher.show();
      switcher.setSelected(validScene);
      state.selectedSceneId = validScene;
    } else {
      // No scene param: show gallery.
      gallery.show();
      canvas!.style.display = "none";
    }

    // Escape key returns to gallery.
    window.addEventListener("keydown", (ev) => {
      if (ev.key === "Escape" && router.state === "simulation") {
        transitionToGallery();
      }
    });
  }

  // ... rest of boot (transport creation, audio, rAF) ...
  
  // Only create transport if we're in sim mode.
  if (router.state === "simulation") {
    const transport: Transport = createTransport(validScene);
    transport.connect((msg) => handleMessage(msg, state, renderer));
    state.rafHandle = requestAnimationFrame(() => frame(state, renderer));
  }
}
```

- [ ] **Step 4: Remove SceneBrowser import and usage**

In `main.ts`:
- Remove `import { SceneBrowser, type SceneSelectIntent } from "./ui/scene_browser";`
- Add `import { GalleryView, type SceneSelectIntent } from "./ui/gallery_view";`
- Add `import { SceneSwitcher } from "./ui/scene_switcher";`
- Remove `state.sceneBrowser = new SceneBrowser(...)` from boot()
- Remove `sceneBrowser: SceneBrowser | null` from AppState interface
- Remove `sceneBrowser: null` from createAppState()

- [ ] **Step 5: Delete scene_browser.ts**

```bash
rm frontend/src/ui/scene_browser.ts
```

- [ ] **Step 6: Add gallery E2E test**

Create `frontend/src/tests/e2e/gallery.spec.ts`:

```typescript
// frontend/src/tests/e2e/gallery.spec.ts
import { test, expect } from "@playwright/test";

test.describe("gallery view", () => {
  test("shows gallery on launch (no ?scene param)", async ({ page }) => {
    await page.goto("/");
    const gallery = page.locator("#simetro-gallery");
    await expect(gallery).toBeVisible({ timeout: 3000 });
    // Canvas should be hidden.
    const canvas = page.locator("#scene");
    await expect(canvas).not.toBeVisible();
  });

  test("clicking a card navigates to simulation", async ({ page }) => {
    await page.goto("/");
    const gallery = page.locator("#simetro-gallery");
    await expect(gallery).toBeVisible();

    // Click the first card.
    const firstCard = gallery.locator("button").first();
    await firstCard.click();

    // Gallery should hide, canvas should show.
    await expect(gallery).not.toBeVisible({ timeout: 3000 });
    const canvas = page.locator("#scene");
    await expect(canvas).toBeVisible();
  });

  test("Escape returns to gallery from simulation", async ({ page }) => {
    await page.goto("/?scene=demo-paths");
    const canvas = page.locator("#scene");
    await expect(canvas).toBeVisible();

    await page.keyboard.press("Escape");

    const gallery = page.locator("#simetro-gallery");
    await expect(gallery).toBeVisible({ timeout: 3000 });
  });

  test("invalid ?scene= shows gallery with error", async ({ page }) => {
    await page.goto("/?scene=nonexistent-scene");
    // Should fall back to gallery (or show error).
    const gallery = page.locator("#simetro-gallery");
    await expect(gallery).toBeVisible({ timeout: 3000 });
  });

  test("switcher pill visible in simulation mode", async ({ page }) => {
    await page.goto("/?scene=demo-paths");
    const switcher = page.locator("#simetro-switcher");
    await expect(switcher).toBeVisible({ timeout: 3000 });
  });
});
```

- [ ] **Step 7: Run all frontend checks**

```bash
cd frontend && npm run typecheck && npm run lint && npm test -- --run && npx playwright test
```

Expected: All pass.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: view routing, gallery integration, scene switcher

- ViewRouter: state machine with monotonic transition tokens
- Transport lifecycle: connect on sim, disconnect on gallery
- Transitioning flag prevents double-clicks (5s safety timeout)
- SceneSwitcher: compact floating pill with prev/next/gallery
- Gallery is default landing (no ?scene param)
- ?scene=X bypasses gallery directly to simulation
- Escape key returns to gallery
- Invalid scene param falls back to gallery
- Delete scene_browser.ts (replaced by gallery + switcher)
- E2E tests for gallery flow"
```

---

## Task 7: Catalog/Registry/Payload Alignment Test

**Files:**
- Modify: `frontend/src/tests/gallery.test.ts` (add alignment test)

### Context

Per rubber-duck review: ensure frontend catalog, Tauri registry, games/ files,
and generated static payloads stay aligned.

- [ ] **Step 1: Add alignment test**

Extend `frontend/src/tests/gallery.test.ts`:

```typescript
import { existsSync } from "fs";
import { resolve } from "path";

describe("catalog/registry/file alignment", () => {
  test("every ready catalog entry has a games/*.json file", () => {
    const ready = SCENE_CATALOG.filter((s) => s.status === "ready");
    for (const scene of ready) {
      const filePath = resolve(__dirname, "../../..", scene.scene_path);
      expect(existsSync(filePath)).toBe(true);
    }
  });

  test("catalog ids are unique", () => {
    const ids = SCENE_CATALOG.map((s) => s.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  test("every catalog entry has scene_path matching id convention", () => {
    for (const scene of SCENE_CATALOG) {
      expect(scene.scene_path).toBe(`games/${scene.id}.json`);
    }
  });
});
```

- [ ] **Step 2: Run test**

```bash
cd frontend && npm test -- --run src/tests/gallery.test.ts
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add frontend/src/tests/gallery.test.ts
git commit -m "test: add catalog/file alignment verification

Ensures every ready catalog entry has a matching games/*.json file,
ids are unique, and scene_path follows the id convention."
```

---

## Validation Checklist (run after all tasks)

```bash
# Rust
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

# Frontend
cd frontend && npm run typecheck && npm run lint && npm test -- --run && npm run build

# E2E (requires emit-static to have been run first)
cd frontend && npx playwright test

# Tauri (if scene registry touched)
cd src-tauri && cargo test --locked
```

---

## Key Safety Constraints (verify during implementation)

- [ ] All text rendered via `textContent`, never `innerHTML`
- [ ] Scene selection via registry `scene_id` only — no frontend-supplied file paths
- [ ] No new npm dependencies
- [ ] Transport never connected in gallery view
- [ ] Every error path visible (palette fallback for thumbs, toast for transitions)
- [ ] `?scene=X` validated against catalog; unknown values fall back to gallery
- [ ] Static payload files include `schema_version` for drift detection
- [ ] Transition token prevents stale messages from corrupting state
- [ ] Generated static payload files are gitignored (CI builds them fresh)
