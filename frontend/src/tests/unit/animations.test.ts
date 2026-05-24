// frontend/src/tests/unit/animations.test.ts
import { describe, it, expect, beforeAll } from "vitest";
import { AnimationEngine } from "../../renderer/animation_engine";
import { animations } from "../../renderer/animations";
import { DEFAULT_THEME } from "../../renderer/theme";
import type { SimEvent, SnapshotPayload, StaticPayload } from "../../protocol/messages";

const scene: StaticPayload = {
  name: "test",
  palette: ["#000", "#fff", "#7aa2f7"],
  background_index: 0,
  nodes: [{ id: 1, pos: [100, 100], shape: "circle", color: 2 }],
  paths: [],
  node_names: {},
  path_names: {},
  mover_names: {},
};

const snap: SnapshotPayload = {
  tick: 0,
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
  it("has a spec for every SimEvent kind", () => {
    const kinds: SimEvent["kind"][] = [
      "mover_departed",
      "mover_arrived",
      "mover_speed_change",
      "node_highlighted",
      "path_pulsed",
      "agent_decided",
      "tick",
    ];
    for (const k of kinds) {
      expect(animations[k]).toBeDefined();
      expect(animations[k].durationMs).toBeGreaterThanOrEqual(0);
      expect(typeof animations[k].ease).toBe("function");
      expect(typeof animations[k].render).toBe("function");
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
    e.spawn({ kind: "mover_arrived", mover: 7, at_node: 1, path: 0 }, t0);
    expect(e.liveCount()).toBe(1);

    const ctx = makeCtx();
    // Mid-animation: still alive.
    const alive1 = e.draw(ctx, t0 + 100, DEFAULT_THEME, scene, snap);
    expect(alive1).toBe(1);

    // Past duration (300ms for mover_arrived): expires this pass.
    const alive2 = e.draw(ctx, t0 + 1000, DEFAULT_THEME, scene, snap);
    expect(alive2).toBe(0);
    expect(e.liveCount()).toBe(0);
  });

  it("ignores zero-duration events (tick)", () => {
    const e = new AnimationEngine();
    e.spawn({ kind: "tick", tick: 1 }, 0);
    expect(e.liveCount()).toBe(0);
  });

  it("overflows by recycling oldest slot (bounded memory)", () => {
    const e = new AnimationEngine();
    const cap = e.capacity();
    for (let i = 0; i < cap + 50; i++) {
      e.spawn({ kind: "mover_arrived", mover: 7, at_node: 1, path: 0 }, 0);
    }
    expect(e.liveCount()).toBe(cap);
  });

  it("clear drops all active slots", () => {
    const e = new AnimationEngine();
    e.spawn({ kind: "mover_arrived", mover: 7, at_node: 1, path: 0 }, 0);
    e.spawn({ kind: "path_pulsed", path: 42 }, 0);
    e.clear();
    expect(e.liveCount()).toBe(0);
  });

  it("draw handles missing pieces in payload gracefully", () => {
    const e = new AnimationEngine();
    e.spawn({ kind: "mover_arrived", mover: 999, at_node: 999, path: 999 }, 0);
    const ctx = makeCtx();
    expect(() => e.draw(ctx, 10, DEFAULT_THEME, scene, snap)).not.toThrow();
  });

  it("path_pulsed uses baked endpoints from the static scene", () => {
    const e = new AnimationEngine();
    const sceneWithPaths: StaticPayload = {
      ...scene,
      paths: [{ id: 42, from_pos: [0, 0], to_pos: [200, 200], color: 2 }],
    };
    e.spawn({ kind: "path_pulsed", path: 42 }, 0);
    const ctx = makeCtx();
    expect(() => e.draw(ctx, 50, DEFAULT_THEME, sceneWithPaths, snap)).not.toThrow();
  });
});
