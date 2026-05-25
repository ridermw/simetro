// frontend/src/renderer/animation_engine.ts
//
// renderer batching and allocation target: animations are draw calls layered ON TOP of the
// static scene from canvas.ts. Each SimEvent becomes a short-lived
// slot in a ring of active animations; each frame the engine walks
// the slots and asks each spec.render to draw at the current eased t.
//
// The slot array is pre-allocated and reused; spawning is O(1).
// Expired slots are marked free in-place — no array compaction, no
// per-frame allocation.

import type { SimEvent, SnapshotPayload, StaticPayload } from "../protocol/messages";
import { animations, type AnimationSpec, type ResolveCtx, type SimEventKind } from "./animations";
import type { Theme } from "./theme";

interface Slot {
  alive: boolean;
  startMs: number;
  spec: AnimationSpec;
  payload: SimEvent;
}

const SLOT_CAPACITY = 256;

export class AnimationEngine {
  private readonly slots: Slot[] = [];
  private nextIndex = 0;
  /** ResolveCtx is reused every frame (mutated in place) to avoid
   *  per-frame allocations (zero-allocation target). */
  private readonly resolve: ResolveCtx = {
    theme: { palette: [], background_index: 0, font: "" },
    scene: {
      name: "",
      palette: [],
      background_index: 0,
      nodes: [],
      paths: [],
      node_names: {},
      path_names: {},
      mover_names: {},
    },
    snapshot: { tick: 0, movers: [] },
  };

  constructor() {
    for (let i = 0; i < SLOT_CAPACITY; i++) {
      this.slots.push({
        alive: false,
        startMs: 0,
        spec: animations.tick,
        payload: { kind: "tick", tick: 0 },
      });
    }
  }

  /** Spawn an animation for `event` starting at `nowMs`. */
  spawn(event: SimEvent, nowMs: number): void {
    const kind: SimEventKind = event.kind;
    const spec = animations[kind];
    // Defensive: unknown event kinds (e.g. new Rust SimEvent variants
    // not yet mirrored in the TS table) are ignored rather than
    // crashing the render loop. Mirror coverage is enforced separately
    // by the "every SimEvent kind has a spec" unit test.
    if (spec === undefined || spec.durationMs <= 0) return;

    // Find a free slot starting from nextIndex; if none, overwrite
    // the oldest by sweeping.
    for (let probe = 0; probe < SLOT_CAPACITY; probe++) {
      const i = (this.nextIndex + probe) % SLOT_CAPACITY;
      const slot = this.slots[i]!;
      if (!slot.alive) {
        slot.alive = true;
        slot.startMs = nowMs;
        slot.spec = spec;
        slot.payload = event;
        this.nextIndex = (i + 1) % SLOT_CAPACITY;
        return;
      }
    }
    // All slots active — overwrite the one we'd recycle next.
    const i = this.nextIndex;
    const slot = this.slots[i]!;
    slot.alive = true;
    slot.startMs = nowMs;
    slot.spec = spec;
    slot.payload = event;
    this.nextIndex = (i + 1) % SLOT_CAPACITY;
  }

  /**
   * Draw all live animations at `nowMs`. Expired slots are released.
   * Returns the number of slots still alive after the pass.
   */
  draw(
    ctx: CanvasRenderingContext2D,
    nowMs: number,
    theme: Theme,
    scene: StaticPayload,
    snapshot: SnapshotPayload
  ): number {
    this.resolve.theme = theme;
    this.resolve.scene = scene;
    this.resolve.snapshot = snapshot;
    let alive = 0;
    for (let i = 0; i < SLOT_CAPACITY; i++) {
      const slot = this.slots[i]!;
      if (!slot.alive) continue;
      const elapsed = nowMs - slot.startMs;
      if (elapsed >= slot.spec.durationMs) {
        slot.alive = false;
        continue;
      }
      const t = elapsed / slot.spec.durationMs;
      const eased = slot.spec.ease(t);
      slot.spec.render(ctx, eased, slot.payload, this.resolve);
      alive += 1;
    }
    return alive;
  }

  liveCount(): number {
    let n = 0;
    for (const s of this.slots) if (s.alive) n += 1;
    return n;
  }

  clear(): void {
    for (const s of this.slots) s.alive = false;
    this.nextIndex = 0;
  }

  /** Test/diagnostic only. */
  capacity(): number {
    return SLOT_CAPACITY;
  }
}
