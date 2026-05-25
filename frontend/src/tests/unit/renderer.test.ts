// frontend/src/tests/unit/renderer.test.ts
import { describe, it, expect, beforeAll } from "vitest";
import { Renderer } from "../../renderer/canvas";
import {
  DEFAULT_THEME,
  paletteColor,
  backgroundColor,
  themeFromStatic,
} from "../../renderer/theme";
import { demoSnapshotEnvelope, demoStaticEnvelope } from "../../transport/mock";
import type { StaticPayload } from "../../protocol/messages";

// jsdom does not implement Canvas2D or Path2D — stub both.
beforeAll(() => {
  type StubCtx = Partial<CanvasRenderingContext2D>;
  const stub: StubCtx = {
    save: () => {},
    restore: () => {},
    setTransform: () => {},
    fillRect: () => {},
    beginPath: () => {},
    moveTo: () => {},
    lineTo: () => {},
    arc: () => {},
    rect: () => {},
    closePath: () => {},
    fill: () => {},
    stroke: () => {},
    translate: () => {},
    scale: () => {},
  };
  const proto = HTMLCanvasElement.prototype;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (proto as any).getContext = () => stub;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (globalThis as any).Path2D = class {
    moveTo(_x: number, _y: number) {}
    lineTo(_x: number, _y: number) {}
  };
});

function makeRenderer(): Renderer {
  const canvas = document.createElement("canvas");
  canvas.width = 800;
  canvas.height = 600;
  return new Renderer(canvas);
}

/** Minimal StaticPayload with no nodes or paths — empty scene. */
function emptyScene(): StaticPayload {
  return {
    name: "empty",
    palette: ["#000000", "#ffffff"],
    background_index: 0,
    nodes: [],
    paths: [],
    node_names: {},
    path_names: {},
    mover_names: {},
  };
}

/** StaticPayload with a single node far outside 800×600. */
function largeScene(): StaticPayload {
  return {
    name: "large",
    palette: ["#000000", "#ffffff", "#ff0000"],
    background_index: 0,
    nodes: [
      { id: 1, pos: [0, 0], shape: "circle", color: 1 },
      { id: 2, pos: [3000, 2000], shape: "circle", color: 1 },
    ],
    paths: [
      { id: 1, from_pos: [0, 0], to_pos: [3000, 2000], color: 2 },
    ],
    node_names: {},
    path_names: {},
    mover_names: {},
  };
}

describe("Renderer", () => {
  it("draws a static demo frame without throwing", () => {
    const r = makeRenderer();
    const staticMsg = demoStaticEnvelope().payload;
    const snapMsg = demoSnapshotEnvelope().payload;
    if (staticMsg.kind !== "static" || snapMsg.kind !== "snapshot") {
      throw new Error("fixtures changed");
    }
    const theme = themeFromStatic(staticMsg.payload);
    r.warm(theme);
    expect(() =>
      r.draw({
        theme,
        scene: staticMsg.payload,
        movers: snapMsg.payload.movers,
      })
    ).not.toThrow();
  });

  it("warm() is idempotent and accepts larger palettes", () => {
    const r = makeRenderer();
    r.warm(DEFAULT_THEME);
    r.warm(DEFAULT_THEME);
    const bigger = { ...DEFAULT_THEME, palette: [...DEFAULT_THEME.palette, "#abcdef"] };
    expect(() => r.warm(bigger)).not.toThrow();
  });

  it("setScene refills Path2D buckets only when scene identity changes", () => {
    const r = makeRenderer();
    const staticMsg = demoStaticEnvelope().payload;
    if (staticMsg.kind !== "static") throw new Error("fixtures changed");
    r.warm(themeFromStatic(staticMsg.payload));
    // First call rebuilds; second is a no-op (no throw).
    expect(() => r.setScene(staticMsg.payload)).not.toThrow();
    expect(() => r.setScene(staticMsg.payload)).not.toThrow();
  });
});

describe("Renderer viewport", () => {
  it("auto-fits a large scene: scale < 1 and non-zero translation", () => {
    const r = makeRenderer();
    r.warm(DEFAULT_THEME);
    r.setScene(largeScene());
    const vp = r.viewportForTest;
    // World spans 3000×2000, canvas is 800×600 — scale must be well below 1.
    expect(vp.scale).toBeLessThan(1);
    // World centre is at (1500, 1000); offset must shift it onto the canvas.
    expect(vp.offsetX).not.toBe(0);
    expect(vp.offsetY).not.toBe(0);
  });

  it("empty scene keeps identity viewport", () => {
    const r = makeRenderer();
    r.warm(DEFAULT_THEME);
    r.setScene(emptyScene());
    const vp = r.viewportForTest;
    expect(vp.scale).toBe(1);
    expect(vp.offsetX).toBe(0);
    expect(vp.offsetY).toBe(0);
  });

  it("panBy shifts offsets by the requested amount", () => {
    const r = makeRenderer();
    r.warm(DEFAULT_THEME);
    r.setScene(emptyScene()); // identity
    r.panBy(50, -30);
    const vp = r.viewportForTest;
    expect(vp.offsetX).toBe(50);
    expect(vp.offsetY).toBe(-30);
  });

  it("zoomAt keeps the world point under the cursor stable", () => {
    const r = makeRenderer();
    r.warm(DEFAULT_THEME);
    r.setScene(emptyScene()); // identity: scale=1, offset=0,0
    const screenX = 400;
    const screenY = 300;
    // world point under cursor before zoom
    const worldXBefore = (screenX - r.viewportForTest.offsetX) / r.viewportForTest.scale;
    const worldYBefore = (screenY - r.viewportForTest.offsetY) / r.viewportForTest.scale;

    r.zoomAt(screenX, screenY, 2);

    // world point under same screen location after zoom
    const vp = r.viewportForTest;
    const worldXAfter = (screenX - vp.offsetX) / vp.scale;
    const worldYAfter = (screenY - vp.offsetY) / vp.scale;
    expect(worldXAfter).toBeCloseTo(worldXBefore, 6);
    expect(worldYAfter).toBeCloseTo(worldYBefore, 6);
    expect(vp.scale).toBe(2);
  });

  it("zoomAt clamps scale to MIN=0.15 and MAX=8", () => {
    const r = makeRenderer();
    r.warm(DEFAULT_THEME);
    r.setScene(emptyScene());
    r.zoomAt(0, 0, 0.001); // way below min
    expect(r.viewportForTest.scale).toBeCloseTo(0.15, 6);
    r.zoomAt(0, 0, 100_000); // way above max
    expect(r.viewportForTest.scale).toBeCloseTo(8, 6);
  });

  it("resetViewport returns to the scene fit transform", () => {
    const r = makeRenderer();
    r.warm(DEFAULT_THEME);
    r.setScene(largeScene());
    const fitVp = { ...r.viewportForTest };

    // Mutate viewport via pan and zoom.
    r.panBy(200, 100);
    r.zoomAt(100, 100, 3);
    expect(r.viewportForTest.scale).not.toBeCloseTo(fitVp.scale, 3);

    r.resetViewport();
    const vp = r.viewportForTest;
    expect(vp.scale).toBeCloseTo(fitVp.scale, 6);
    expect(vp.offsetX).toBeCloseTo(fitVp.offsetX, 6);
    expect(vp.offsetY).toBeCloseTo(fitVp.offsetY, 6);
  });
});

describe("theme", () => {
  it("paletteColor falls back to foreground on out-of-range index", () => {
    const c = paletteColor(DEFAULT_THEME, 9999);
    expect(c).toBe(DEFAULT_THEME.palette[1]);
  });

  it("backgroundColor honors background_index", () => {
    const t = { ...DEFAULT_THEME, background_index: 2 };
    expect(backgroundColor(t)).toBe(DEFAULT_THEME.palette[2]);
  });

  it("backgroundColor falls back to default-dark on bad index", () => {
    const t = { ...DEFAULT_THEME, background_index: 99 };
    expect(backgroundColor(t)).toBe("#0e1116");
  });

  it("themeFromStatic copies palette + background_index from StaticPayload", () => {
    const staticMsg = demoStaticEnvelope().payload;
    if (staticMsg.kind !== "static") throw new Error("fixtures changed");
    const t = themeFromStatic(staticMsg.payload);
    expect(t.palette).toBe(staticMsg.payload.palette);
    expect(t.background_index).toBe(staticMsg.payload.background_index);
  });
});
