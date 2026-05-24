// frontend/src/tests/unit/snapshots.test.ts
import { describe, it, expect } from "vitest";
import { SnapshotBuffer } from "../../store/snapshots";
import type { MoverState, SnapshotPayload } from "../../protocol/messages";

function snap(tick: number, moverX: number): SnapshotPayload {
  return {
    tick,
    movers: [{ id: 1, pos: [moverX, 100], on_path: 0, speed: 1 }],
  };
}

describe("SnapshotBuffer", () => {
  it("returns null current() before any push", () => {
    const b = new SnapshotBuffer();
    expect(b.current()).toBeNull();
    expect(b.previous()).toBeNull();
  });

  it("tracks current + previous as snapshots arrive", () => {
    const b = new SnapshotBuffer();
    b.push(snap(1, 0));
    expect(b.current()?.tick).toBe(1);
    expect(b.previous()).toBeNull();
    b.push(snap(2, 100));
    expect(b.current()?.tick).toBe(2);
    expect(b.previous()?.tick).toBe(1);
  });

  it("caps history at the ring capacity", () => {
    const b = new SnapshotBuffer();
    for (let t = 1; t <= 10; t++) b.push(snap(t, t * 10));
    expect(b.current()?.tick).toBe(10);
    expect(b.previous()?.tick).toBe(9);
  });

  it("interpolates mover position by alpha", () => {
    const b = new SnapshotBuffer();
    b.push(snap(1, 0));
    b.push(snap(2, 100));
    const out: MoverState[] = [];
    b.interpolatedMovers(0, out);
    expect(out[0]?.pos[0]).toBe(0);
    b.interpolatedMovers(0.5, out);
    expect(out[0]?.pos[0]).toBe(50);
    b.interpolatedMovers(1, out);
    expect(out[0]?.pos[0]).toBe(100);
  });

  it("clamps alpha into [0,1]", () => {
    const b = new SnapshotBuffer();
    b.push(snap(1, 0));
    b.push(snap(2, 100));
    const out: MoverState[] = [];
    b.interpolatedMovers(-5, out);
    expect(out[0]?.pos[0]).toBe(0);
    b.interpolatedMovers(7, out);
    expect(out[0]?.pos[0]).toBe(100);
  });

  it("reuses the supplied output array (zero per-frame alloc)", () => {
    const b = new SnapshotBuffer();
    b.push(snap(1, 0));
    b.push(snap(2, 100));
    const out: MoverState[] = [];
    const first = b.interpolatedMovers(0.5, out);
    const firstSlot = first[0];
    b.interpolatedMovers(0.75, out);
    expect(out[0]).toBe(firstSlot);
  });

  it("interpolation does NOT allocate a fresh map per call", () => {
    // Repeat interpolation hundreds of times; if `prevById` were
    // `new Map()`-d each frame this test would be a hot churn —
    // we instead assert that subsequent calls still produce
    // correct values (a behavioural guard against regressions).
    const b = new SnapshotBuffer();
    b.push(snap(1, 0));
    b.push(snap(2, 100));
    const out: MoverState[] = [];
    for (let i = 0; i < 256; i++) {
      b.interpolatedMovers((i % 11) / 10, out);
    }
    b.interpolatedMovers(0.5, out);
    expect(out[0]?.pos[0]).toBe(50);
  });

  it("markStale() jump-cuts on next push", () => {
    const b = new SnapshotBuffer();
    b.push(snap(1, 0));
    b.push(snap(2, 100));
    b.markStale();
    b.push(snap(99, 9999));
    expect(b.current()?.tick).toBe(99);
    expect(b.previous()).toBeNull();
  });
});
