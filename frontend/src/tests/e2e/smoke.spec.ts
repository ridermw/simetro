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
    await page.waitForTimeout(300); // Allow first rAF frame + mock transport tick.
    // Use the viewport transform to compute where node id=1 (world 200,200)
    // actually lands on screen after auto-fit, then check that pixel is non-background.
    const isNotBlank = await page.evaluate(() => {
      const c = document.getElementById("scene") as HTMLCanvasElement | null;
      if (c === null) return false;
      const ctx = c.getContext("2d");
      if (ctx === null) return false;
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const r = (window as any).__simetroRenderer;
      const dpr = window.devicePixelRatio || 1;
      let sx: number, sy: number;
      if (r?.viewportForTest !== undefined) {
        const vp: { scale: number; offsetX: number; offsetY: number } = r.viewportForTest;
        // Node id=1 is at world position (200, 200) in the demo scene.
        sx = Math.round((200 * vp.scale + vp.offsetX) * dpr);
        sy = Math.round((200 * vp.scale + vp.offsetY) * dpr);
      } else {
        // Fallback if renderer not exposed: sample original demo position.
        sx = Math.round(200 * dpr);
        sy = Math.round(200 * dpr);
      }
      const px = ctx.getImageData(sx, sy, 1, 1).data;
      // Background color is #0e1116 (14, 17, 22).
      return px[0] !== 14 || px[1] !== 17 || px[2] !== 22;
    });
    expect(isNotBlank).toBe(true);
  });
});
