// One-shot screenshot capture for PR 15a. Not part of CI — invoked
// via `npx playwright test --config scripts/screenshot/pw.config.ts`
// from inside the frontend directory.
//
// The non-Tauri preview build does NOT load scene JSON when you select
// a scene (MockTransport feeds a fixed demo regardless of selection),
// and no shipped frontend PR draws sl1_places / sl1_links on the
// canvas yet. So the only visually-differentiated surface per scene
// is the catalog card itself (title + description + status + tags).
// We capture a tight crop of each card rather than a full page to
// avoid four near-identical full-page PNGs.

import { test } from "@playwright/test";
import * as path from "path";
import { fileURLToPath } from "url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const OUT = process.env.SHOWCASE_OUT ?? path.resolve(HERE, "../../../docs/showcase/pr15a");

const SHOWCASE_SCENES = [
  { id: "sandwich-shop" },
  { id: "theme-park-day" },
  { id: "school-lunch-line" },
  { id: "coffee-roastery" },
  { id: "library-checkout" },
  { id: "farmers-market" },
  { id: "bicycle-repair-shop" },
] as const;

test("scene browser overview", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");
  await page.locator("#simetro-scene-list").waitFor();
  await page.locator("#simetro-scene-bicycle-repair-shop").scrollIntoViewIfNeeded();
  await page.screenshot({
    path: path.join(OUT, "scene-browser-overview.png"),
    fullPage: true,
  });
});

test("scene catalog cards (cropped)", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.goto("/");
  await page.locator("#simetro-scene-list").waitFor();
  for (const scene of SHOWCASE_SCENES) {
    const button = page.locator(`#simetro-scene-${scene.id}`);
    await button.scrollIntoViewIfNeeded();
    // small settle for scroll
    await page.waitForTimeout(80);
    await button.screenshot({
      path: path.join(OUT, `card-${scene.id}.png`),
    });
  }
});

test("sl1 hud demo", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?sl1demo=1");
  await page.waitForTimeout(1500);
  await page.screenshot({
    path: path.join(OUT, "sl1-hud-demo.png"),
    fullPage: true,
  });
});
