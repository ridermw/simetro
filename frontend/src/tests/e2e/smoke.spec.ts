// frontend/src/tests/e2e/smoke.spec.ts
//
// end-to-end smoke coverage — end-to-end smoke. Boots the production
// bundle (vite preview) and verifies the renderer, controls, and
// inspector materialize on screen.

import { test, expect } from "@playwright/test";

test.describe("simetro smoke", () => {
  test("canvas is visible", async ({ page }) => {
    await page.goto("/");
    const canvas = page.locator("#scene");
    await expect(canvas).toBeVisible();
  });

  test("controls bar renders with toolbar role", async ({ page }) => {
    await page.goto("/");
    const bar = page.locator("#simetro-controls");
    await expect(bar).toBeVisible();
    await expect(bar).toHaveAttribute("role", "toolbar");
    await expect(page.locator("#simetro-play-pause")).toBeVisible();
    await expect(page.locator("#simetro-step")).toBeVisible();
    await expect(page.locator("#simetro-reload")).toBeVisible();
  });

  test("inspector panel mounts and is reachable as a region", async ({ page }) => {
    await page.goto("/");
    const panel = page.locator("#simetro-inspector");
    await expect(panel).toBeVisible();
    await expect(panel).toHaveAttribute("role", "region");
  });

  test("perf overlay turns on with ?perf=1", async ({ page }) => {
    await page.goto("/?perf=1");
    const perf = page.locator("#simetro-perf");
    await expect(perf).toBeVisible();
    // Wait for at least one fps sample (>= 500ms window).
    await page.waitForTimeout(700);
    await expect(perf).toContainText("fps");
  });

  test("play/pause button toggles aria-label", async ({ page }) => {
    await page.goto("/");
    const btn = page.locator("#simetro-play-pause");
    await expect(btn).toHaveAttribute("aria-label", "Pause");
    await btn.click();
    await expect(btn).toHaveAttribute("aria-label", "Play");
    await btn.click();
    await expect(btn).toHaveAttribute("aria-label", "Pause");
  });

  test("heartbeat badge mounts", async ({ page }) => {
    await page.goto("/");
    const hb = page.locator("#simetro-heartbeat");
    await expect(hb).toBeVisible();
  });

  test("canvas has rendered content (not blank)", async ({ page }) => {
    await page.goto("/");
    await page.waitForTimeout(200); // Allow first rAF frame.
    // Use a small JS evaluation to sample the canvas pixel buffer.
    const isNotBlack = await page.evaluate(() => {
      const c = document.getElementById("scene") as HTMLCanvasElement | null;
      if (c === null) return false;
      const ctx = c.getContext("2d");
      if (ctx === null) return false;
      // Sample a node we know exists in demo: node id=1 at (200, 200).
      const dpr = window.devicePixelRatio || 1;
      const px = ctx.getImageData(200 * dpr, 200 * dpr, 1, 1).data;
      // px[0..2] = RGB. The background is #0e1116 (14, 17, 22).
      return px[0] !== 14 || px[1] !== 17 || px[2] !== 22;
    });
    expect(isNotBlack).toBe(true);
  });
});
