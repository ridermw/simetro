// frontend/src/store/snapshots.ts
//
// PLAN §4 / §13 — Snapshot buffer + interpolation gate.
//
//   ┌──────────────────────────────────────────────────────────┐
//   │  transport ──▶ push(snapshot) ──▶ ring buffer (cap 4)    │
//   │                                       │                  │
//   │                                       ▼                  │
//   │              renderer ──── current() ── interp(t) ───▶ ▢ │
//   └──────────────────────────────────────────────────────────┘
//
// Two-snapshot interpolation: renderer asks for `interpolated()` at
// every animation frame; if we have a previous + current pair we
// lerp positions over the tick window. With a single snapshot we
// render statically (covers Step 16's case and PLAN §13 edge case
// #2: "first snapshot before first event").
//
// Tab-refocus edge case (PLAN §13 #5): when the page is hidden the
// rAF loop pauses; on resume we jump-cut to the latest snapshot
// instead of catching up animations — see `markStale()`.
//
// Per PR #1 review (Copilot, P1): `prevById` is a reusable Map
// field that is cleared and refilled each interpolation pass,
// rather than `new Map(...)`'d every frame. Snapshots are
// movers-only now (geometry lives in `StaticPayload`).

import type { MoverState, SnapshotPayload } from "../protocol/messages";

const RING_CAP = 4;

export class SnapshotBuffer {
  private ring: SnapshotPayload[] = [];
  private stale = false;
  /** Lookup of previous-tick movers keyed by id. Persistent across
   *  frames; cleared and refilled inside `interpolatedMovers`. */
  private readonly prevById: Map<number, MoverState> = new Map();

  push(snap: SnapshotPayload): void {
    if (this.stale) {
      this.ring = [snap];
      this.stale = false;
      return;
    }
    this.ring.push(snap);
    if (this.ring.length > RING_CAP) {
      this.ring.shift();
    }
  }

  /** Latest snapshot, or null if none received yet. */
  current(): SnapshotPayload | null {
    return this.ring.length === 0 ? null : (this.ring[this.ring.length - 1] ?? null);
  }

  /** Previous snapshot, used as the interpolation anchor. */
  previous(): SnapshotPayload | null {
    return this.ring.length < 2 ? null : (this.ring[this.ring.length - 2] ?? null);
  }

  /** Drop history; on next push we treat it as the only frame. */
  markStale(): void {
    this.stale = true;
  }

  /**
   * Lerp mover positions between previous and current at fraction
   * `alpha` in [0,1]. If only one snapshot is available, returns it
   * unchanged. Mutates and returns the supplied `out` array — zero
   * per-frame allocation (PLAN §14).
   */
  interpolatedMovers(alpha: number, out: MoverState[]): MoverState[] {
    const cur = this.current();
    if (cur === null) {
      out.length = 0;
      return out;
    }
    const prev = this.previous();
    if (prev === null) {
      out.length = cur.movers.length;
      for (let i = 0; i < cur.movers.length; i++) {
        const c = cur.movers[i]!;
        const slot = out[i];
        if (slot === undefined) {
          out[i] = { id: c.id, pos: [c.pos[0], c.pos[1]], on_path: c.on_path, speed: c.speed };
        } else {
          slot.id = c.id;
          slot.pos[0] = c.pos[0];
          slot.pos[1] = c.pos[1];
          slot.on_path = c.on_path;
          slot.speed = c.speed;
        }
      }
      return out;
    }
    // Lerp by id. Clear-and-refill the reusable map to avoid the
    // per-frame `new Map(...)` allocation flagged in PR #1 review.
    this.prevById.clear();
    for (const m of prev.movers) this.prevById.set(m.id, m);
    const a = Math.max(0, Math.min(1, alpha));
    out.length = cur.movers.length;
    for (let i = 0; i < cur.movers.length; i++) {
      const c = cur.movers[i]!;
      const p = this.prevById.get(c.id);
      const px = p?.pos[0] ?? c.pos[0];
      const py = p?.pos[1] ?? c.pos[1];
      const x = px + (c.pos[0] - px) * a;
      const y = py + (c.pos[1] - py) * a;
      const slot = out[i];
      if (slot === undefined) {
        out[i] = { id: c.id, pos: [x, y], on_path: c.on_path, speed: c.speed };
      } else {
        slot.id = c.id;
        slot.pos[0] = x;
        slot.pos[1] = y;
        slot.on_path = c.on_path;
        slot.speed = c.speed;
      }
    }
    return out;
  }
}
