// frontend/src/store/events.ts
//
// PLAN §4 — event queue. SimEvents arrive in bursts (one Events
// message can carry many) but are consumed by the animation engine
// frame-by-frame. We use a tiny FIFO that pre-allocates capacity to
// avoid per-tick churn (PLAN §14 zero-alloc invariant on the frontend
// side).
//
//   transport ──▶ enqueue(events) ──▶ ring ──▶ drainInto(buffer)
//                                       │           │
//                                       ▼           ▼
//                                animation engine, audio engine
//
// Capacity 1024 covers the worst case for a 1000-mover stress scene
// emitting one event per mover per tick at 30Hz (PLAN §14 stretch).

import type { SimEvent } from "../protocol/messages";

const CAPACITY = 1024;

export class EventQueue {
  private readonly slots: (SimEvent | null)[] = new Array(CAPACITY).fill(null);
  private head = 0;
  private tail = 0;
  private size = 0;

  enqueue(event: SimEvent): boolean {
    if (this.size >= CAPACITY) return false; // overflow — caller decides
    this.slots[this.tail] = event;
    this.tail = (this.tail + 1) % CAPACITY;
    this.size += 1;
    return true;
  }

  enqueueAll(events: SimEvent[]): number {
    let n = 0;
    for (const e of events) {
      if (e === undefined) continue;
      if (this.enqueue(e)) n += 1;
      else break;
    }
    return n;
  }

  /** Drain into `out` (mutated, returned). Reuses caller's buffer. */
  drainInto(out: SimEvent[]): SimEvent[] {
    out.length = 0;
    while (this.size > 0) {
      const ev = this.slots[this.head];
      if (ev !== null && ev !== undefined) out.push(ev);
      this.slots[this.head] = null;
      this.head = (this.head + 1) % CAPACITY;
      this.size -= 1;
    }
    return out;
  }

  get length(): number {
    return this.size;
  }
}
