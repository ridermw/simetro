// One-shot screenshot capture for PR 15a. Not part of CI — invoked
// via `npx playwright test --config scripts/screenshot/pw.config.ts`
// from inside the frontend directory.

import { test } from "@playwright/test";
import * as path from "path";
import { fileURLToPath } from "url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const OUT = process.env.SHOWCASE_OUT ?? path.resolve(HERE, "../../../docs/showcase/pr15a");

const SHOWCASE_SCENES = [
  { id: "sandwich-shop", title: "Sandwich Shop" },
  { id: "theme-park-day", title: "Theme Park Day" },
  { id: "coffee-roastery", title: "Coffee Roastery" },
  { id: "library-checkout", title: "Library Checkout" },
] as const;

test("scene browser overview", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/");
  await page.locator("#simetro-scene-list").waitFor();
  await page.locator("#simetro-scene-bicycle-repair-shop").scrollIntoViewIfNeeded();
  await page.screenshot({
    path: path.join(OUT, "scene-browser-overview.png"),
    fullPage: true,
  });
});

for (const scene of SHOWCASE_SCENES) {
  test(`scene browser — ${scene.id} selected`, async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/");
    const button = page.locator(`#simetro-scene-${scene.id}`);
    await button.scrollIntoViewIfNeeded();
    await button.click();
    await page.waitForTimeout(150);
    await page.screenshot({
      path: path.join(OUT, `scene-${scene.id}.png`),
      fullPage: true,
    });
  });
}

test("sl1 hud demo", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto("/?sl1demo=1");
  await page.waitForTimeout(1500);
  await page.screenshot({
    path: path.join(OUT, "sl1-hud-demo.png"),
    fullPage: true,
  });
});
