// frontend/src/tests/unit/animations.test.ts
import { describe, it, expect, beforeAll } from "vitest";
import { AnimationEngine } from "../../renderer/animation_engine";
import { animations } from "../../renderer/animations";
import { DEFAULT_THEME } from "../../renderer/theme";
import type { SimEvent, SnapshotPayload } from "../../protocol/messages";

const emptySnap: SnapshotPayload = {
  tick: 0,
  nodes: [{ id: 1, pos: [100, 100], shape: "circle", color: 2 }],
  paths: [],
  movers: [{ id: 7, pos: [100, 100], on_path: 0, speed: 1 }],
};

function makeCtx(): CanvasRenderingContext2D {
  const stub = {
    save: () => {},
    restore: () => {},
    beginPath: () => {},
    arc: () => {},
    moveTo: () => {},
    lineTo: () => {},
    closePath: () => {},
    fill: () => {},
    stroke: () => {},
    globalAlpha: 1,
    strokeStyle: "",
    fillStyle: "",
    lineWidth: 0,
  };
  return stub as unknown as CanvasRenderingContext2D;
}

describe("animations table", () => {
  it("has a spec for every SimEvent tag", () => {
    const tags: SimEvent["tag"][] = [
      "MoverDeparted",
      "MoverArrived",
      "MoverSpeedChange",
      "NodeHighlighted",
      "PathPulsed",
      "AgentDecided",
      "Tick",
    ];
    for (const t of tags) {
      expect(animations[t]).toBeDefined();
      expect(animations[t].durationMs).toBeGreaterThanOrEqual(0);
      expect(typeof animations[t].ease).toBe("function");
      expect(typeof animations[t].render).toBe("function");
    }
  });
});

describe("AnimationEngine", () => {
  beforeAll(() => {
    (globalThis as unknown as { Path2D: typeof Path2D }).Path2D = class {} as typeof Path2D;
  });

  it("spawn + draw increments live count, expires after duration", () => {
    const e = new AnimationEngine();
    const t0 = 1000;
    e.spawn({ tag: "MoverArrived", mover: 7, at_node: 1, path: 0 }, t0);
    expect(e.liveCount()).toBe(1);

    const ctx = makeCtx();
    // Mid-animation: still alive.
    const alive1 = e.draw(ctx, t0 + 100, DEFAULT_THEME, emptySnap);
    expect(alive1).toBe(1);

    // Past duration (300ms for MoverArrived): expires this pass.
    const alive2 = e.draw(ctx, t0 + 1000, DEFAULT_THEME, emptySnap);
    expect(alive2).toBe(0);
    expect(e.liveCount()).toBe(0);
  });

  it("ignores zero-duration events (Tick)", () => {
    const e = new AnimationEngine();
    e.spawn({ tag: "Tick", tick: 1 }, 0);
    expect(e.liveCount()).toBe(0);
  });

  it("overflows by recycling oldest slot (bounded memory)", () => {
    const e = new AnimationEngine();
    const cap = e.capacity();
    for (let i = 0; i < cap + 50; i++) {
      e.spawn({ tag: "MoverArrived", mover: 7, at_node: 1, path: 0 }, 0);
    }
    expect(e.liveCount()).toBe(cap);
  });

  it("draw handles missing pieces in payload gracefully", () => {
    const e = new AnimationEngine();
    e.spawn({ tag: "MoverArrived", mover: 999, at_node: 999, path: 999 }, 0);
    const ctx = makeCtx();
    expect(() => e.draw(ctx, 10, DEFAULT_THEME, emptySnap)).not.toThrow();
  });
});
