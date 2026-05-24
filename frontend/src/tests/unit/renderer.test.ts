// frontend/src/tests/unit/renderer.test.ts
import { describe, it, expect } from "vitest";
import { renderStaticFrame, type SceneState } from "../../renderer/canvas";
import { demoSnapshotEnvelope, demoStaticEnvelope } from "../../transport/mock";

describe("renderer", () => {
  it("renders without throwing given valid scene state", () => {
    const canvas = document.createElement("canvas");
    canvas.width = 800;
    canvas.height = 600;
    // jsdom does not implement getContext; stub it for this unit test.
    // Step 22 covers real rendering via Playwright + a real Chromium.
    type StubCtx = Partial<CanvasRenderingContext2D> & { save: () => void };
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
      stroke: (_p?: Path2D) => {},
      fillStyle: "" as string | CanvasGradient | CanvasPattern,
      strokeStyle: "" as string | CanvasGradient | CanvasPattern,
      lineWidth: 0,
      lineCap: "butt",
    };
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    canvas.getContext = ((_: string) => stub) as any;
    // Path2D is also absent in jsdom — provide a minimal stub.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (globalThis as any).Path2D = class {
      moveTo(_x: number, _y: number) {}
      lineTo(_x: number, _y: number) {}
    };

    const staticMsg = demoStaticEnvelope().payload;
    const snapMsg = demoSnapshotEnvelope().payload;
    if (staticMsg.type !== "Static" || snapMsg.type !== "Snapshot") {
      throw new Error("test fixtures changed shape");
    }
    const scene: SceneState = {
      theme: staticMsg.payload.theme,
      snapshot: snapMsg.payload,
    };

    expect(() => renderStaticFrame(canvas, scene)).not.toThrow();
  });

  it("is a no-op when state is incomplete", () => {
    const canvas = document.createElement("canvas");
    canvas.width = 100;
    canvas.height = 100;
    const scene: SceneState = { theme: null, snapshot: null };
    expect(() => renderStaticFrame(canvas, scene)).not.toThrow();
  });
});
