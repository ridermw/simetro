// frontend/src/tests/e2e/showcase_daily_life.spec.ts
//
// PR 15a — Daily-life delights showcase. End-to-end coverage that the
// 7 new SL1 scenes added in this PR are surfaced through the live
// UI: they appear in the gallery with their human-readable titles,
// are selectable without script execution (XSS-safe text), and never
// leak their backend `games/*.json` path into the DOM. This validates
// the catalog → GalleryView → user-visible affordance chain end-to-end
// against the production-built bundle (Vite preview).
//
// The HUD/snapshot path for these scenes is exercised by
// `crates/engine/tests/showcase_daily_life.rs` (engine + protocol
// determinism + GameOutcome reach-Won). The Tauri scene-switch path
// is exercised by `src-tauri/src/scene_registry.rs` unit tests. The
// browser does not actually load the scene JSON outside Tauri, so
// the per-scene HUD render is intentionally not in scope here.

import { test, expect } from "@playwright/test";

const SHOWCASE_SCENES: ReadonlyArray<{
  readonly id: string;
  readonly title: string;
}> = [
  { id: "sandwich-shop", title: "Sandwich Shop" },
  { id: "theme-park-day", title: "Theme Park Day" },
  { id: "school-lunch-line", title: "School Lunch Line" },
  { id: "coffee-roastery", title: "Coffee Roastery" },
  { id: "library-checkout", title: "Library Checkout" },
  { id: "farmers-market", title: "Farmers Market Saturday" },
  { id: "bicycle-repair-shop", title: "Bicycle Repair Shop" },
];

test.describe("SL1 showcase — daily-life delights", () => {
  test("every showcase scene mounts in the gallery with safe text", async ({
    page,
  }) => {
    await page.goto("/");

    const gallery = page.locator("#simetro-gallery");
    await expect(gallery).toBeVisible();

    for (const scene of SHOWCASE_SCENES) {
      const card = page.locator(`button[data-scene-id="${scene.id}"]`);
      await expect(card).toBeVisible();
      await expect(card).toContainText(scene.title);
      const html = await card.evaluate((el) => el.innerHTML);
      expect(html).not.toContain(`games/${scene.id}.json`);
      expect(html).not.toContain("<script");
    }
  });

  test("clicking a showcase scene enters sim view and updates the URL", async ({ page }) => {
    await page.goto("/");

    for (const scene of SHOWCASE_SCENES) {
      await page.goto("/");
      const card = page.locator(`button[data-scene-id="${scene.id}"]`);
      await card.click();
      await expect(page.locator("#scene")).toBeVisible();
      await expect(page.locator("#simetro-switcher")).toBeVisible();
      await expect(page).toHaveURL(new RegExp(`\\?scene=${scene.id}$`));
    }
  });

  test("gallery exposes showcase cards with stable scene ids", async ({ page }) => {
    await page.goto("/");
    const gallery = page.locator("#simetro-gallery");
    for (const scene of SHOWCASE_SCENES) {
      const card = gallery.locator(`button[data-scene-id="${scene.id}"]`);
      await expect(card).toHaveCount(1);
    }
  });
});
