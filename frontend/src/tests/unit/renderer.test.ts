// frontend/src/tests/unit/renderer.test.ts
import { describe, it, expect, beforeAll } from "vitest";
import { Renderer } from "../../renderer/canvas";
import { DEFAULT_THEME, paletteColor, backgroundColor } from "../../renderer/theme";
import { demoSnapshotEnvelope, demoStaticEnvelope } from "../../transport/mock";

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

describe("Renderer", () => {
  it("draws a static demo frame without throwing", () => {
    const r = makeRenderer();
    r.warm(DEFAULT_THEME);
    const snapMsg = demoSnapshotEnvelope().payload;
    const staticMsg = demoStaticEnvelope().payload;
    if (snapMsg.type !== "Snapshot" || staticMsg.type !== "Static") {
      throw new Error("fixtures changed");
    }
    expect(() =>
      r.draw({
        theme: staticMsg.payload.theme,
        snapshot: snapMsg.payload,
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
});
