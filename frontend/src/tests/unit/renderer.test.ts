// frontend/src/tests/unit/renderer.test.ts
import { describe, it, expect, beforeAll, vi } from "vitest";
import { Renderer, truncateLabel } from "../../renderer/canvas";
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
  const fillTextCalls: { text: string; x: number; y: number }[] = [];
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
    fillText: (text: string, x: number, y: number) => {
      fillTextCalls.push({ text, x, y });
    },
  };
  const proto = HTMLCanvasElement.prototype;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (proto as any).getContext = () => stub;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (globalThis as any).Path2D = class {
    moveTo(_x: number, _y: number) {}
    lineTo(_x: number, _y: number) {}
  };
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (globalThis as any).__fillTextCalls = fillTextCalls;
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

  it("draws node labels in node draw order when show_node_labels=true", () => {
    const r = makeRenderer();
    const scene: StaticPayload = {
      name: "labeled",
      palette: ["#000", "#fff", "#7aa2f7"],
      background_index: 0,
      nodes: [
        { id: 1, pos: [10, 20], shape: "circle", color: 2 },
        { id: 2, pos: [30, 40], shape: "square", color: 1 },
      ],
      paths: [],
      node_names: { 1: "place-alpha", 2: "place-beta" },
      path_names: {},
      mover_names: {},
      show_node_labels: true,
    };
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const calls: { text: string; x: number; y: number }[] = (globalThis as any).__fillTextCalls;
    calls.length = 0;
    r.warm(themeFromStatic(scene));
    r.draw({ theme: themeFromStatic(scene), scene, movers: [] });
    // Exact order assertion — labels must follow node draw order.
    expect(calls.map((c) => c.text)).toEqual(["place-alpha", "place-beta"]);
  });

  it("does NOT draw node labels by default (legacy scenes have no labels)", () => {
    const r = makeRenderer();
    const scene: StaticPayload = {
      name: "unlabeled",
      palette: ["#000", "#fff", "#7aa2f7"],
      background_index: 0,
      nodes: [{ id: 1, pos: [10, 20], shape: "circle", color: 2 }],
      paths: [],
      node_names: { 1: "should-not-appear" },
      path_names: {},
      mover_names: {},
      // show_node_labels intentionally omitted (default behavior)
    };
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const calls: { text: string; x: number; y: number }[] = (globalThis as any).__fillTextCalls;
    calls.length = 0;
    r.warm(themeFromStatic(scene));
    r.draw({ theme: themeFromStatic(scene), scene, movers: [] });
    expect(calls.map((c) => c.text)).not.toContain("should-not-appear");
  });

  it("skips labels for nodes with no name entry, draws for the rest", () => {
    const r = makeRenderer();
    const scene: StaticPayload = {
      name: "mixed",
      palette: ["#000", "#fff", "#7aa2f7"],
      background_index: 0,
      nodes: [
        { id: 1, pos: [10, 20], shape: "circle", color: 2 },
        { id: 2, pos: [30, 40], shape: "square", color: 1 },
        { id: 3, pos: [50, 60], shape: "diamond", color: 2 },
      ],
      paths: [],
      node_names: { 1: "named-1", 3: "named-3" }, // node 2 has no name
      path_names: {},
      mover_names: {},
      show_node_labels: true,
    };
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const calls: { text: string; x: number; y: number }[] = (globalThis as any).__fillTextCalls;
    calls.length = 0;
    r.warm(themeFromStatic(scene));
    r.draw({ theme: themeFromStatic(scene), scene, movers: [] });
    // Exact list — only named-1 and named-3 in node order.
    expect(calls.map((c) => c.text)).toEqual(["named-1", "named-3"]);
  });

  it("truncates very long node names to avoid label overflow", () => {
    const r = makeRenderer();
    const longName = "a".repeat(200);
    const scene: StaticPayload = {
      name: "long",
      palette: ["#000", "#fff", "#7aa2f7"],
      background_index: 0,
      nodes: [{ id: 1, pos: [10, 20], shape: "circle", color: 2 }],
      paths: [],
      node_names: { 1: longName },
      path_names: {},
      mover_names: {},
      show_node_labels: true,
    };
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const calls: { text: string; x: number; y: number }[] = (globalThis as any).__fillTextCalls;
    calls.length = 0;
    r.warm(themeFromStatic(scene));
    r.draw({ theme: themeFromStatic(scene), scene, movers: [] });
    expect(calls).toHaveLength(1);
    const drawn = calls[0]!.text;
    expect(drawn.length).toBeLessThan(longName.length);
    expect(drawn.endsWith("…")).toBe(true);
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

  it("screenToWorld inverts the viewport transform", () => {
    const r = makeRenderer();
    r.warm(DEFAULT_THEME);
    r.setScene(emptyScene()); // identity: scale=1, offset=0,0
    // Under identity, world == screen.
    expect(r.screenToWorld(100, 200)).toEqual([100, 200]);

    // Pan by (50, 30), then world = (screen - offset) / scale
    r.panBy(50, 30);
    const [wx, wy] = r.screenToWorld(150, 130);
    expect(wx).toBeCloseTo(100, 6);
    expect(wy).toBeCloseTo(100, 6);
  });

  it("screenToWorld works under scale", () => {
    const r = makeRenderer();
    r.warm(DEFAULT_THEME);
    r.setScene(emptyScene());
    r.zoomAt(0, 0, 2); // scale=2, offset=(0,0)
    // Screen (200,400) => world (100,200).
    const [wx, wy] = r.screenToWorld(200, 400);
    expect(wx).toBeCloseTo(100, 6);
    expect(wy).toBeCloseTo(200, 6);
  });

  it("refitViewport updates fit and viewport for new canvas dimensions", () => {
    const canvas = document.createElement("canvas");
    canvas.width = 800;
    canvas.height = 600;
    const r = new Renderer(canvas);
    r.warm(DEFAULT_THEME);
    r.setScene(largeScene());
    const scaleBefore = r.viewportForTest.scale;

    // Shrink canvas — same world, smaller canvas should produce smaller scale.
    canvas.width = 400;
    canvas.height = 300;
    r.refitViewport();

    const scaleAfter = r.viewportForTest.scale;
    expect(scaleAfter).toBeLessThan(scaleBefore);

    // resetViewport should now use the recomputed fit.
    r.panBy(999, 999);
    r.resetViewport();
    expect(r.viewportForTest.scale).toBeCloseTo(scaleAfter, 6);
  });

  it("refitViewport is a no-op when no world bounds are set", () => {
    const r = makeRenderer();
    r.warm(DEFAULT_THEME);
    r.setScene(emptyScene()); // no world bounds
    r.panBy(100, 100);
    r.refitViewport(); // should not throw or crash
    // Pan-offset preserved (no-op since no bounds).
    expect(r.viewportForTest.offsetX).toBe(100);
  });

  it("auto-fit clamps scale to a positive minimum on tiny canvases", () => {
    const canvas = document.createElement("canvas");
    canvas.width = 20;
    canvas.height = 20;
    const r = new Renderer(canvas);
    r.warm(DEFAULT_THEME);
    r.setScene(largeScene());
    const vp = r.viewportForTest;
    expect(vp.scale).toBeCloseTo(0.15, 6);
    expect(Number.isFinite(vp.offsetX)).toBe(true);
    expect(Number.isFinite(vp.offsetY)).toBe(true);
  });

  it("releases pointer capture when dragging ends", () => {
    const canvas = document.createElement("canvas");
    canvas.width = 800;
    canvas.height = 600;
    const r = new Renderer(canvas);
    const setPointerCapture = vi.fn();
    const releasePointerCapture = vi.fn();
    const hasPointerCapture = vi.fn(() => true);
    Object.assign(canvas, { setPointerCapture, releasePointerCapture, hasPointerCapture });
    r.attachViewportControls();

    const down = new MouseEvent("pointerdown", { button: 0, clientX: 10, clientY: 10, bubbles: true });
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (down as any).pointerId = 7;
    canvas.dispatchEvent(down);

    const up = new MouseEvent("pointerup", { button: 0, clientX: 10, clientY: 10, bubbles: true });
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (up as any).pointerId = 7;
    canvas.dispatchEvent(up);

    expect(setPointerCapture).toHaveBeenCalledWith(7);
    expect(releasePointerCapture).toHaveBeenCalledWith(7);
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

describe("truncateLabel", () => {
  it("returns short strings unchanged", () => {
    expect(truncateLabel("short")).toBe("short");
    expect(truncateLabel("")).toBe("");
  });

  it("truncates strings longer than the limit and appends ellipsis", () => {
    const long = "x".repeat(100);
    const result = truncateLabel(long);
    expect(result.length).toBeLessThan(long.length);
    expect(result.endsWith("…")).toBe(true);
  });

  it("is idempotent (truncating already-truncated returns same result)", () => {
    const once = truncateLabel("x".repeat(100));
    const twice = truncateLabel(once);
    expect(twice).toBe(once);
  });

  it("preserves a string at exactly the limit length", () => {
    // LABEL_MAX_CHARS is 28; a 28-char string should pass through.
    const exact = "x".repeat(28);
    expect(truncateLabel(exact)).toBe(exact);
  });
});
