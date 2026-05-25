# Complex Scenario Pack Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add 40 complex local scenarios, exactly 10 per difficulty, using only current `main` capabilities while leaving agent and visibility-model upgrades to the separate worktree.

**Architecture:** Generate and commit 40 new `games/*.json` files, 20 SL1 draft scenarios and 20 legacy-rendered systems worlds. Register every new scene in `frontend/src/catalog/scenes.ts` and `src-tauri/src/scene_registry.rs` so the existing world-quality set-equality invariant continues to pass. Add catalog tests that enforce the new pack's 40-scene, 10-per-difficulty, and 20/20 SL1-vs-legacy guarantees without changing the visibility model in this worktree.

**Tech Stack:** Rust engine loader/tests, Tauri scene registry, TypeScript frontend catalog/Vitest, JSON scene files.

---

## Scope decisions

- Do not remove or hide existing scenes in this worktree; the visibility-model worktree owns that.
- Do not upgrade `SpeedTuner` or add new agent actions here; the agent worktree owns that.
- Make legacy-rendered scenes mechanically as rich as current nodes/paths/movers allow: branching-looking topologies, multiple subsystems, varied mover speeds, dense silhouettes, and non-transit system framing.
- Make SL1 scenes as rich as the current strict schema allows: 5+ places, 5+ links, 5+ things, 4+ transforms, 2+ demand entries, 2+ pressure entries, observability/objectives/failure/victory where supported by current loader.

## File structure

- Create 40 files under `games/`.
- Modify `frontend/src/catalog/scenes.ts` to append matching `defineScene(...)` entries.
- Modify `frontend/src/tests/unit/catalog.test.ts` to enforce pack invariants.
- Modify `src-tauri/src/scene_registry.rs` to append matching `scene_entry!(...)` entries.
- Modify `docs/superpowers/specs/2026-05-25-scenario-pack-design.md` to record the latest scope: content in this worktree, visibility and agent upgrades elsewhere.

## Scenario inventory

Use these exact slugs and metadata categories:

| Difficulty | SL1 draft slugs | Legacy ready slugs |
| --- | --- | --- |
| intro | `clinic-triage-desk`, `greenhouse-water-watch`, `library-reshelving-clock`, `microgrid-starter`, `sensor-calibration-lab` | `circuit-garden`, `kitchen-prep-board`, `archive-index-table`, `reef-nursery`, `robot-arm-workbench` |
| easy | `stormwater-pump-room`, `bakery-oven-shift`, `warehouse-cold-chain`, `observatory-night-queue`, `recycling-sort-floor` | `forge-heat-map`, `seed-bank-vault`, `drone-repair-bay`, `weather-balloon-yard`, `crystal-growth-rig` |
| medium | `datacenter-cooling-surge`, `hospital-bed-command`, `food-bank-allocation`, `security-alert-fusion`, `satellite-downlink-window` | `bioreactor-balance`, `disaster-supply-staging`, `fabric-dye-lab`, `museum-conservation-bench`, `wildfire-watch-grid` |
| hard | `chip-fab-yield-crisis`, `regional-blackstart`, `airport-ground-stop`, `pandemic-supply-web`, `fusion-shot-campaign` | `quantum-control-room`, `deep-sea-habitat-grid`, `city-budget-war-room`, `planetary-defense-array`, `autonomous-farm-season` |

---

## Task 1: Update spec to current worktree scope

**Files:**
- Modify: `docs/superpowers/specs/2026-05-25-scenario-pack-design.md`

- [ ] **Step 1: Verify the spec says visibility and agent upgrades are out of this worktree**

Run:

```bash
rg -n "separate worktree|Existing transit-loop scenes remain|agent and visibility" docs/superpowers/specs/2026-05-25-scenario-pack-design.md
```

Expected: output includes the goal paragraph saying the agent and visibility upgrades are in a separate worktree, and the architecture section saying existing transit-loop scenes remain here.

- [ ] **Step 2: Commit the scope update**

Run:

```bash
git add docs/superpowers/specs/2026-05-25-scenario-pack-design.md
git commit -m "docs: scope scenario pack to content" -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

Expected: commit succeeds.

---

## Task 2: Add pack-invariant catalog tests first

**Files:**
- Modify: `frontend/src/tests/unit/catalog.test.ts`

- [ ] **Step 1: Add constants for the 40 new slugs**

Insert this after `REQUIRED_STRING_FIELDS`:

```ts
const NEW_SCENARIO_PACK_IDS = [
  "clinic-triage-desk",
  "greenhouse-water-watch",
  "library-reshelving-clock",
  "microgrid-starter",
  "sensor-calibration-lab",
  "circuit-garden",
  "kitchen-prep-board",
  "archive-index-table",
  "reef-nursery",
  "robot-arm-workbench",
  "stormwater-pump-room",
  "bakery-oven-shift",
  "warehouse-cold-chain",
  "observatory-night-queue",
  "recycling-sort-floor",
  "forge-heat-map",
  "seed-bank-vault",
  "drone-repair-bay",
  "weather-balloon-yard",
  "crystal-growth-rig",
  "datacenter-cooling-surge",
  "hospital-bed-command",
  "food-bank-allocation",
  "security-alert-fusion",
  "satellite-downlink-window",
  "bioreactor-balance",
  "disaster-supply-staging",
  "fabric-dye-lab",
  "museum-conservation-bench",
  "wildfire-watch-grid",
  "chip-fab-yield-crisis",
  "regional-blackstart",
  "airport-ground-stop",
  "pandemic-supply-web",
  "fusion-shot-campaign",
  "quantum-control-room",
  "deep-sea-habitat-grid",
  "city-budget-war-room",
  "planetary-defense-array",
  "autonomous-farm-season",
] as const;
```

- [ ] **Step 2: Add a failing test for the pack shape**

Insert this test before `"describes screenshot capture without adding a live provider dependency"`:

```ts
  it("adds the 40-scene complex scenario pack with balanced difficulty and scene kinds", () => {
    const pack = NEW_SCENARIO_PACK_IDS.map((id) => {
      const scene = findSceneById(id);
      expect(scene, `${id} should be registered in SCENE_CATALOG`).toBeDefined();
      return scene;
    }).filter((scene): scene is SceneCatalogEntry => scene !== undefined);

    expect(pack).toHaveLength(40);
    expect(new Set(pack.map((scene) => scene.id)).size).toBe(40);
    expect(pack.filter((scene) => scene.world_kind === "sl1_scenario")).toHaveLength(20);
    expect(pack.filter((scene) => scene.world_kind === "transit_loop")).toHaveLength(20);
    expect(pack.filter((scene) => scene.status === "draft")).toHaveLength(20);
    expect(pack.filter((scene) => scene.status === "ready")).toHaveLength(20);

    for (const difficulty of ["intro", "easy", "medium", "hard"] as const) {
      expect(pack.filter((scene) => scene.difficulty === difficulty)).toHaveLength(10);
    }
  });
```

- [ ] **Step 3: Run the test and verify it fails**

Run:

```bash
cd frontend && npm test -- --run catalog
```

Expected: FAIL because the first new slug, `clinic-triage-desk`, is not registered yet.

---

## Task 3: Generate the 40 scene JSON files

**Files:**
- Create: `games/clinic-triage-desk.json`
- Create: `games/greenhouse-water-watch.json`
- Create: `games/library-reshelving-clock.json`
- Create: `games/microgrid-starter.json`
- Create: `games/sensor-calibration-lab.json`
- Create: `games/circuit-garden.json`
- Create: `games/kitchen-prep-board.json`
- Create: `games/archive-index-table.json`
- Create: `games/reef-nursery.json`
- Create: `games/robot-arm-workbench.json`
- Create: `games/stormwater-pump-room.json`
- Create: `games/bakery-oven-shift.json`
- Create: `games/warehouse-cold-chain.json`
- Create: `games/observatory-night-queue.json`
- Create: `games/recycling-sort-floor.json`
- Create: `games/forge-heat-map.json`
- Create: `games/seed-bank-vault.json`
- Create: `games/drone-repair-bay.json`
- Create: `games/weather-balloon-yard.json`
- Create: `games/crystal-growth-rig.json`
- Create: `games/datacenter-cooling-surge.json`
- Create: `games/hospital-bed-command.json`
- Create: `games/food-bank-allocation.json`
- Create: `games/security-alert-fusion.json`
- Create: `games/satellite-downlink-window.json`
- Create: `games/bioreactor-balance.json`
- Create: `games/disaster-supply-staging.json`
- Create: `games/fabric-dye-lab.json`
- Create: `games/museum-conservation-bench.json`
- Create: `games/wildfire-watch-grid.json`
- Create: `games/chip-fab-yield-crisis.json`
- Create: `games/regional-blackstart.json`
- Create: `games/airport-ground-stop.json`
- Create: `games/pandemic-supply-web.json`
- Create: `games/fusion-shot-campaign.json`
- Create: `games/quantum-control-room.json`
- Create: `games/deep-sea-habitat-grid.json`
- Create: `games/city-budget-war-room.json`
- Create: `games/planetary-defense-array.json`
- Create: `games/autonomous-farm-season.json`

- [ ] **Step 1: Create a temporary generator outside the repo**

Create `/Users/mattheww/.copilot/session-state/a22b859e-6493-4aa7-851d-025e568673f6/files/generate_scenario_pack.py` with code that:

1. writes the exact 40 JSON files listed above;
2. gives every JSON file `schema_version: 1`, `name`, `theme`, `catalog`, `pieces`, `goals`, `agents`;
3. gives SL1 scenes empty legacy `pieces` arrays and a `scenario_language_v1` block with at least five places, five links, five things, four transforms, two demand entries, and two pressure entries;
4. gives legacy scenes at least eight nodes, ten paths, five movers, three shapes, and three colors;
5. uses `json.dump(..., indent=2)` and appends a trailing newline.

Use the exact `SCENES` table from the Scenario inventory section above as the generator input. Use these five palettes cyclically:

```python
PALETTES = [
    ["#0b1020", "#e8eaed", "#7aa2f7", "#f7768e", "#9ece6a", "#e0af68", "#bb9af7"],
    ["#101820", "#f2f7f5", "#00a8e8", "#ff6b6b", "#84dcc6", "#ffd166", "#9b5de5"],
    ["#15120f", "#f5ead6", "#2ec4b6", "#e71d36", "#ff9f1c", "#a8dadc", "#b56576"],
    ["#061826", "#f8f9fa", "#4cc9f0", "#f72585", "#80ed99", "#f9c74f", "#b5179e"],
    ["#1a1423", "#fff7ed", "#06d6a0", "#ef476f", "#118ab2", "#ffd166", "#c77dff"],
]
```

Legacy scenes should use this node shape cycle:

```python
SHAPES = ["circle", "square", "triangle", "diamond", "hexagon"]
```

SL1 scenes should use these role families by difficulty:

```python
ROLE_FAMILIES = {
    "intro": ["source", "buffer", "processor", "dashboard", "operator"],
    "easy": ["intake", "normalizer", "planner", "quality_gate", "operator"],
    "medium": ["source", "scheduler", "storage", "observability", "incident_command"],
    "hard": ["demand_front", "optimizer", "constraint_manager", "risk_board", "commander"],
}
```

- [ ] **Step 2: Run the generator**

Run:

```bash
python /Users/mattheww/.copilot/session-state/a22b859e-6493-4aa7-851d-025e568673f6/files/generate_scenario_pack.py
```

Expected: it prints `wrote 40 scenarios`.

- [ ] **Step 3: Confirm the file count**

Run:

```bash
python - <<'PY'
from pathlib import Path
slugs = [
  "clinic-triage-desk","greenhouse-water-watch","library-reshelving-clock","microgrid-starter","sensor-calibration-lab",
  "circuit-garden","kitchen-prep-board","archive-index-table","reef-nursery","robot-arm-workbench",
  "stormwater-pump-room","bakery-oven-shift","warehouse-cold-chain","observatory-night-queue","recycling-sort-floor",
  "forge-heat-map","seed-bank-vault","drone-repair-bay","weather-balloon-yard","crystal-growth-rig",
  "datacenter-cooling-surge","hospital-bed-command","food-bank-allocation","security-alert-fusion","satellite-downlink-window",
  "bioreactor-balance","disaster-supply-staging","fabric-dye-lab","museum-conservation-bench","wildfire-watch-grid",
  "chip-fab-yield-crisis","regional-blackstart","airport-ground-stop","pandemic-supply-web","fusion-shot-campaign",
  "quantum-control-room","deep-sea-habitat-grid","city-budget-war-room","planetary-defense-array","autonomous-farm-season",
]
missing = [slug for slug in slugs if not Path("games", f"{slug}.json").is_file()]
assert not missing, missing
print(f"confirmed {len(slugs)} scenario files")
PY
```

Expected: `confirmed 40 scenario files`.

---

## Task 4: Register the 40 scenes in the frontend catalog

**Files:**
- Modify: `frontend/src/catalog/scenes.ts`

- [ ] **Step 1: Append catalog entries after `gpu-launch-week`**

Append one `defineScene({ ... })` entry per new slug before the closing `] as const`. Each entry must use:

```ts
world_kind: "sl1_scenario"
status: "draft"
```

for the 20 SL1 scenes, and:

```ts
world_kind: "transit_loop"
status: "ready"
```

for the 20 legacy-rendered scenes.

For every new entry, set `difficulty` from the Scenario inventory table, set `palette_name` to the slug with hyphens converted to underscores, and include exactly three `rules_summary` strings and exactly three `visual_notes` strings. The first rules summary for each SL1 scene must mention `scenario_language_v1`; the first rules summary for each legacy scene must mention `current renderer`.

- [ ] **Step 2: Run the catalog test**

Run:

```bash
cd frontend && npm test -- --run catalog
```

Expected: if dependencies are installed, the new pack-shape test passes its catalog assertions but world-quality may still fail until Tauri registry entries are added.

---

## Task 5: Register the 40 scenes in the Tauri registry

**Files:**
- Modify: `src-tauri/src/scene_registry.rs`

- [ ] **Step 1: Append `scene_entry!` lines**

Append these lines to `SCENE_ENTRIES` after `scene_entry!("gpu-launch-week"),`:

```rust
    scene_entry!("clinic-triage-desk"),
    scene_entry!("greenhouse-water-watch"),
    scene_entry!("library-reshelving-clock"),
    scene_entry!("microgrid-starter"),
    scene_entry!("sensor-calibration-lab"),
    scene_entry!("circuit-garden"),
    scene_entry!("kitchen-prep-board"),
    scene_entry!("archive-index-table"),
    scene_entry!("reef-nursery"),
    scene_entry!("robot-arm-workbench"),
    scene_entry!("stormwater-pump-room"),
    scene_entry!("bakery-oven-shift"),
    scene_entry!("warehouse-cold-chain"),
    scene_entry!("observatory-night-queue"),
    scene_entry!("recycling-sort-floor"),
    scene_entry!("forge-heat-map"),
    scene_entry!("seed-bank-vault"),
    scene_entry!("drone-repair-bay"),
    scene_entry!("weather-balloon-yard"),
    scene_entry!("crystal-growth-rig"),
    scene_entry!("datacenter-cooling-surge"),
    scene_entry!("hospital-bed-command"),
    scene_entry!("food-bank-allocation"),
    scene_entry!("security-alert-fusion"),
    scene_entry!("satellite-downlink-window"),
    scene_entry!("bioreactor-balance"),
    scene_entry!("disaster-supply-staging"),
    scene_entry!("fabric-dye-lab"),
    scene_entry!("museum-conservation-bench"),
    scene_entry!("wildfire-watch-grid"),
    scene_entry!("chip-fab-yield-crisis"),
    scene_entry!("regional-blackstart"),
    scene_entry!("airport-ground-stop"),
    scene_entry!("pandemic-supply-web"),
    scene_entry!("fusion-shot-campaign"),
    scene_entry!("quantum-control-room"),
    scene_entry!("deep-sea-habitat-grid"),
    scene_entry!("city-budget-war-room"),
    scene_entry!("planetary-defense-array"),
    scene_entry!("autonomous-farm-season"),
```

- [ ] **Step 2: Run the world-quality checklist**

Run:

```bash
cargo test -p simetro-engine --test world_quality_checklist -- --nocapture
```

Expected: PASS. If it fails, fix the named JSON file, catalog entry, or registry entry and rerun this same command.

---

## Task 6: Validate frontend and build outputs

**Files:**
- No new files; validates previous tasks.

- [ ] **Step 1: Install frontend dependencies if missing**

Run:

```bash
test -d frontend/node_modules || (cd frontend && npm ci --silent)
```

Expected: exits 0.

- [ ] **Step 2: Run focused frontend tests**

Run:

```bash
cd frontend && npm test -- --run catalog scene_browser
```

Expected: PASS with the new pack-shape test and existing scene browser tests passing.

- [ ] **Step 3: Run typecheck**

Run:

```bash
cd frontend && npm run typecheck
```

Expected: PASS.

- [ ] **Step 4: Run build**

Run:

```bash
cd frontend && npm run build
```

Expected: PASS and Vite writes `frontend/dist`.

---

## Task 7: Optional Tauri registry test if disk permits

**Files:**
- No new files; validates registry behavior.

- [ ] **Step 1: Check available disk**

Run:

```bash
df -h .
```

Expected: at least 3 GiB available before running Tauri tests.

- [ ] **Step 2: Run Tauri scene registry tests**

Run:

```bash
cd src-tauri && cargo test --locked scene_registry -- --nocapture
```

Expected: PASS. If the command fails with `No space left on device`, do not treat it as a code failure; record the environment blocker and rely on `world_quality_checklist` for registry alignment in this worktree.

---

## Task 8: Final review and commit

**Files:**
- All files changed in prior tasks.

- [ ] **Step 1: Review changed files**

Run:

```bash
git --no-pager status --short
git --no-pager diff --stat
```

Expected: 40 new `games/*.json` files, modified `frontend/src/catalog/scenes.ts`, modified `frontend/src/tests/unit/catalog.test.ts`, modified `src-tauri/src/scene_registry.rs`, modified spec, and this plan file.

- [ ] **Step 2: Check for accidental generated artifacts**

Run:

```bash
git --no-pager status --short | rg "frontend/dist|node_modules|target|src-tauri/target" && exit 1 || true
```

Expected: exits 0 with no generated build artifacts staged or unstaged.

- [ ] **Step 3: Commit implementation**

Run:

```bash
git add games frontend/src/catalog/scenes.ts frontend/src/tests/unit/catalog.test.ts src-tauri/src/scene_registry.rs docs/superpowers/specs/2026-05-25-scenario-pack-design.md docs/superpowers/plans/2026-05-25-complex-scenario-pack.md
git commit -m "feat: add complex scenario pack" -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
```

Expected: commit succeeds.

## Self-review

- Spec coverage: covers exactly 40 new scenes, 10 per difficulty, 20 SL1 drafts, 20 legacy ready worlds, catalog/registry alignment, and validation.
- Scope alignment: does not remove or hide existing scenes because the user said agent and visibility model work is happening in a separate worktree.
- Red-flag scan: no incomplete markers or open-ended "add tests" steps remain.
- Type consistency: uses existing `SceneWorldKind`, `SceneDifficulty`, and `SceneStatus` values only.
