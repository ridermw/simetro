// frontend/src/tests/e2e/animates.spec.ts
//
// animation smoke test — Assert that movers actually animate over time.
// The animated MockTransport continuously advances mover positions,
// so after sufficient elapsed time the canvas content must differ.

import { test, expect } from "@playwright/test";

test.describe("simetro animation", () => {
  test("movers change position over time", async ({ page }) => {
    await page.goto("/");
    // Wait for the first snapshot to render (MockTransport emits at t=0
    // then every 50ms; the rAF loop draws on the next frame).
    await page.waitForTimeout(300);

    // Sample pixel data from the canvas center region where movers transit.
    const sample1 = await sampleCanvasRegion(page);

    // Wait for movers to advance significantly.
    await page.waitForTimeout(600);

    const sample2 = await sampleCanvasRegion(page);

    // Pixel content must have changed — movers moved.
    expect(sample1).not.toEqual(sample2);
  });

  test("heartbeat badge stays green (receiving snapshots)", async ({ page }) => {
    await page.goto("/");
    await page.waitForTimeout(500);
    const hb = page.locator("#simetro-heartbeat");
    await expect(hb).toBeVisible();
    // The heartbeat is a styled dot; state is exposed via aria-label.
    await expect(hb).toHaveAttribute("aria-label", "Engine heartbeat: ok");
  });
});

/** Sample pixel data from the entire canvas and return a hash for
 *  comparison. Any movement anywhere on the canvas will change this. */
async function sampleCanvasRegion(page: import("@playwright/test").Page): Promise<string> {
  return page.evaluate(() => {
    const c = document.getElementById("scene") as HTMLCanvasElement | null;
    if (c === null) return "";
    const ctx = c.getContext("2d");
    if (ctx === null) return "";
    // Sample the full canvas — any mover movement will change pixels.
    const data = ctx.getImageData(0, 0, c.width, c.height).data;
    // Simple hash of pixel data for comparison.
    let hash = 0;
    for (let i = 0; i < data.length; i += 16) {
      hash = ((hash << 5) - hash + data[i]! + data[i + 1]! + data[i + 2]!) | 0;
    }
    return String(hash);
  });
}
