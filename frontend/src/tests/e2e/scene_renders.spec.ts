// frontend/src/tests/e2e/scene_renders.spec.ts
//
// Per-scene render verification.
//
// For each scene in the catalog with status="ready", load the scene via
// ?scene=<id>, verify the canvas mounts, no fault overlay appears, and
// the renderer produces at least one non-background pixel.
//
// This catches:
//   - Missing static payloads (emit-static failed for the scene)
//   - Schema mismatches (envelope version drift)
//   - Renderer regressions on specific scene geometry

import { test, expect } from "@playwright/test";

// All scene IDs that should render. Kept in sync with SCENE_CATALOG
// (status="ready" entries). The catalog test verifies all 59 are ready.
const SCENE_IDS = [
  "airport-ground-stop",
  "archive-index-table",
  "autonomous-farm-season",
  "bakery-oven-shift",
  "bicycle-repair-shop",
  "bioreactor-balance",
  "cargo-loom",
  "chip-fab-yield-crisis",
  "circuit-garden",
  "city-budget-war-room",
  "clinic-triage-desk",
  "coffee-roastery",
  "crystal-growth-rig",
  "data-packet-city",
  "datacenter-cooling-surge",
  "deep-sea-habitat-grid",
  "demo-paths",
  "disaster-supply-staging",
  "drone-repair-bay",
  "emergency-dispatch",
  "fabric-dye-lab",
  "factory-line-seeds",
  "farmers-market",
  "food-bank-allocation",
  "forge-heat-map",
  "fusion-shot-campaign",
  "garden-pollinators",
  "gpu-launch-week",
  "greenhouse-water-watch",
  "hospital-bed-command",
  "kitchen-prep-board",
  "library-checkout",
  "library-reshelving-clock",
  "metro-pulse",
  "microgrid-starter",
  "museum-conservation-bench",
  "night-market-runners",
  "observatory-night-queue",
  "orbital-transfers",
  "pandemic-supply-web",
  "planetary-defense-array",
  "power-grid-balancer",
  "quantum-control-room",
  "recycling-sort-floor",
  "reef-nursery",
  "regional-blackstart",
  "river-ferries",
  "robot-arm-workbench",
  "sandwich-shop",
  "satellite-downlink-window",
  "school-lunch-line",
  "security-alert-fusion",
  "seed-bank-vault",
  "sensor-calibration-lab",
  "stormwater-pump-room",
  "theme-park-day",
  "warehouse-cold-chain",
  "weather-balloon-yard",
  "wildfire-watch-grid",
];

test.describe("per-scene render verification", () => {
  for (const sceneId of SCENE_IDS) {
    test(`scene ${sceneId} renders without fault`, async ({ page }) => {
      const consoleErrors: string[] = [];
      page.on("console", (msg) => {
        if (msg.type() === "error") consoleErrors.push(msg.text());
      });

      await page.goto(`/?scene=${sceneId}`);

      // Canvas must be visible.
      await expect(page.locator("#scene")).toBeVisible();

      // Allow time for static payload fetch + first snapshot + rAF frame.
      await page.waitForTimeout(500);

      // Fault overlay must NOT be visible — catches missing payloads,
      // schema mismatches, and other load errors.
      const faultText = await page.evaluate(() => {
        const el = document.getElementById("simetro-fault");
        if (el === null || el.style.display === "none") return null;
        return el.textContent;
      });
      expect(faultText, `scene ${sceneId} produced fault: ${faultText}`).toBeNull();

      // Canvas must have at least some non-background pixels —
      // catches blank-canvas regressions where the static payload
      // loaded but rendering produced nothing.
      const hasContent = await page.evaluate(() => {
        const c = document.getElementById("scene") as HTMLCanvasElement | null;
        if (c === null) return false;
        const ctx = c.getContext("2d");
        if (ctx === null) return false;
        const data = ctx.getImageData(0, 0, c.width, c.height).data;
        // Background is #0e1116 (14, 17, 22). Sample sparsely for speed.
        for (let i = 0; i < data.length; i += 64) {
          const r = data[i] ?? 0;
          const g = data[i + 1] ?? 0;
          const b = data[i + 2] ?? 0;
          if (r !== 14 || g !== 17 || b !== 22) return true;
        }
        return false;
      });
      expect(hasContent, `scene ${sceneId} produced blank canvas`).toBe(true);

      // No console errors during scene load (catches schema mismatches,
      // fetch failures, missing payloads).
      const relevantErrors = consoleErrors.filter(
        (e) => e.includes("simetro:") || e.includes("static payload")
      );
      expect(
        relevantErrors,
        `scene ${sceneId} logged errors: ${relevantErrors.join("; ")}`
      ).toEqual([]);
    });
  }
});
