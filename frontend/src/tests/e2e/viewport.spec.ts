// frontend/src/tests/e2e/viewport.spec.ts
//
// Renderer viewport E2E — verifies that the auto-fit, drag-pan,
// wheel-zoom, and double-click-reset viewport behaviors work in a
// real browser against the Vite preview build.
//
// Viewport state is read via `window.__simetroRenderer.viewportForTest`
// exposed by main.ts in non-Tauri builds. Pixel sampling via
// ctx.getImageData() is used to confirm rendered content is on-screen.

import { test, expect } from "@playwright/test";

// -----------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------

/** Read the current viewport transform from the renderer. */
async function readViewport(
  page: import("@playwright/test").Page
): Promise<{ scale: number; offsetX: number; offsetY: number } | null> {
  return page.evaluate(() => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const r = (window as any).__simetroRenderer;
    if (r?.viewportForTest === undefined) return null;
    const vp = r.viewportForTest as { scale: number; offsetX: number; offsetY: number };
    return { scale: vp.scale, offsetX: vp.offsetX, offsetY: vp.offsetY };
  });
}

/** Hash the full canvas pixel buffer for change detection. */
async function canvasHash(page: import("@playwright/test").Page): Promise<number> {
  return page.evaluate(() => {
    const c = document.getElementById("scene") as HTMLCanvasElement | null;
    if (c === null) return 0;
    const ctx = c.getContext("2d");
    if (ctx === null) return 0;
    const data = ctx.getImageData(0, 0, c.width, c.height).data;
    let h = 0;
    for (let i = 0; i < data.length; i += 16) {
      h = ((h << 5) - h + (data[i] ?? 0) + (data[i + 1] ?? 0) + (data[i + 2] ?? 0)) | 0;
    }
    return h;
  });
}

/** Return the canvas bounding rect centre in page coordinates. */
async function canvasCentre(
  page: import("@playwright/test").Page
): Promise<{ x: number; y: number }> {
  return page.evaluate(() => {
    const c = document.getElementById("scene") as HTMLCanvasElement | null;
    if (c === null) return { x: 400, y: 300 };
    const r = c.getBoundingClientRect();
    return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
  });
}

// -----------------------------------------------------------------
// Tests
// -----------------------------------------------------------------

test.describe("renderer viewport", () => {
  test("scene is visibly drawn on canvas after load (auto-fit)", async ({ page }) => {
    await page.goto("/");
    // Wait for MockTransport's first snapshot + rAF frame.
    await page.waitForTimeout(400);

    const vp = await readViewport(page);
    expect(vp).not.toBeNull();

    // With the demo scene (nodes span ~400×280 world units on an 800×600 canvas)
    // auto-fit produces a scale above 1. Regardless, the viewport must be set.
    expect(vp!.scale).toBeGreaterThan(0);

    // Confirm node id=1 (world 200,200) is rendered at a non-background pixel.
    const isVisible = await page.evaluate(
      ({ scale, offsetX, offsetY }) => {
        const c = document.getElementById("scene") as HTMLCanvasElement | null;
        if (c === null) return false;
        const ctx = c.getContext("2d");
        if (ctx === null) return false;
        const dpr = window.devicePixelRatio || 1;
        const sx = Math.round((200 * scale + offsetX) * dpr);
        const sy = Math.round((200 * scale + offsetY) * dpr);
        // Clamp to canvas bounds.
        if (sx < 0 || sy < 0 || sx >= c.width || sy >= c.height) return false;
        const px = ctx.getImageData(sx, sy, 1, 1).data;
        // Background is #0e1116 (14, 17, 22).
        return px[0] !== 14 || px[1] !== 17 || px[2] !== 22;
      },
      { scale: vp!.scale, offsetX: vp!.offsetX, offsetY: vp!.offsetY }
    );
    expect(isVisible).toBe(true);
  });

  test("wheel zoom changes viewport scale and canvas pixels", async ({ page }) => {
    await page.goto("/");
    await page.waitForTimeout(400);

    const vpBefore = await readViewport(page);
    expect(vpBefore).not.toBeNull();
    const hashBefore = await canvasHash(page);

    const centre = await canvasCentre(page);
    // Scroll up = zoom in (deltaY < 0 → factor 1.1 per tick).
    await page.mouse.move(centre.x, centre.y);
    await page.mouse.wheel(0, -150);
    // Allow rAF to redraw.
    await page.waitForTimeout(100);

    const vpAfter = await readViewport(page);
    const hashAfter = await canvasHash(page);

    expect(vpAfter).not.toBeNull();
    // Scale must have increased (zoomed in).
    expect(vpAfter!.scale).toBeGreaterThan(vpBefore!.scale);
    // Canvas pixels must have changed.
    expect(hashAfter).not.toBe(hashBefore);
  });

  test("drag pan shifts viewport offsets and canvas pixels", async ({ page }) => {
    await page.goto("/");
    await page.waitForTimeout(400);

    const vpBefore = await readViewport(page);
    expect(vpBefore).not.toBeNull();
    const hashBefore = await canvasHash(page);

    const centre = await canvasCentre(page);
    // Drag 80px right and 40px down.
    await page.mouse.move(centre.x, centre.y);
    await page.mouse.down();
    await page.mouse.move(centre.x + 80, centre.y + 40, { steps: 5 });
    await page.mouse.up();
    await page.waitForTimeout(100);

    const vpAfter = await readViewport(page);
    const hashAfter = await canvasHash(page);

    expect(vpAfter).not.toBeNull();
    // Offsets must have shifted by approximately the drag delta.
    expect(vpAfter!.offsetX).toBeGreaterThan(vpBefore!.offsetX + 40);
    expect(vpAfter!.offsetY).toBeGreaterThan(vpBefore!.offsetY + 10);
    // Canvas pixels must have changed.
    expect(hashAfter).not.toBe(hashBefore);
  });

  test("double-click reset returns viewport to the scene fit", async ({ page }) => {
    await page.goto("/");
    await page.waitForTimeout(400);

    // Capture the auto-fit viewport.
    const fitVp = await readViewport(page);
    expect(fitVp).not.toBeNull();

    // Mutate viewport: pan and zoom away from fit.
    const centre = await canvasCentre(page);
    await page.mouse.move(centre.x, centre.y);
    await page.mouse.wheel(0, -300); // zoom in
    await page.waitForTimeout(50);
    await page.mouse.down();
    await page.mouse.move(centre.x + 120, centre.y + 80, { steps: 6 });
    await page.mouse.up();
    await page.waitForTimeout(50);

    const mutatedVp = await readViewport(page);
    expect(mutatedVp!.scale).not.toBeCloseTo(fitVp!.scale, 1);

    // Double-click to reset.
    await page.dblclick("#scene");
    await page.waitForTimeout(100);

    const resetVp = await readViewport(page);
    expect(resetVp).not.toBeNull();
    expect(resetVp!.scale).toBeCloseTo(fitVp!.scale, 4);
    expect(resetVp!.offsetX).toBeCloseTo(fitVp!.offsetX, 2);
    expect(resetVp!.offsetY).toBeCloseTo(fitVp!.offsetY, 2);
  });
});
