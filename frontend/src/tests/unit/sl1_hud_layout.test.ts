import { describe, expect, it } from "vitest";

import { createSl1Hud } from "../../ui/sl1_hud";

describe("createSl1Hud layout stacks", () => {
  it("mounts SL1 HUD panels into left and right vertical flex stacks", () => {
    const parent = document.createElement("div");
    const hud = createSl1Hud(parent);

    const leftStack = parent.querySelector("#simetro-sl1-hud-left") as HTMLDivElement | null;
    const rightStack = parent.querySelector("#simetro-sl1-hud-right") as HTMLDivElement | null;

    expect(leftStack).not.toBeNull();
    expect(rightStack).not.toBeNull();
    expect(Array.from(leftStack?.children ?? [])).toEqual([
      hud.status.__testRoot(),
      hud.objectives.__testRoot(),
      hud.metrics.__testRoot(),
    ]);
    expect(Array.from(rightStack?.children ?? [])).toEqual([
      hud.conditions.__testRoot(),
      hud.dashboards.__testRoot(),
      hud.alerts.__testRoot(),
    ]);

    for (const stack of [leftStack, rightStack]) {
      const css = stack?.style.cssText ?? "";
      expect(css).toContain("display: flex");
      expect(css).toContain("flex-direction: column");
      expect(css).toContain("max-height: calc(100vh - 24px)");
      expect(css).toContain("max-width: calc(50vw - 24px)");
      // Stacks must not block canvas interaction — they're a layout
      // wrapper only; individual panels can re-enable pointer-events
      // if they need interactive controls.
      expect(css).toContain("pointer-events: none");
    }

    // Each refactored panel must NOT carry stale absolute positioning
    // — the parent flex stack owns layout now.
    const refactoredPanels = [
      hud.status.__testRoot(),
      hud.objectives.__testRoot(),
      hud.metrics.__testRoot(),
      hud.conditions.__testRoot(),
      hud.dashboards.__testRoot(),
      hud.alerts.__testRoot(),
    ];
    for (const panel of refactoredPanels) {
      const css = panel.style.cssText;
      expect(css).not.toContain("position: absolute");
      // Top/left/right coordinates would conflict with flex layout.
      expect(css).not.toMatch(/(^|;)\s*top:/);
      expect(css).not.toMatch(/(^|;)\s*left:/);
      expect(css).not.toMatch(/(^|;)\s*right:/);
    }
  });
});
