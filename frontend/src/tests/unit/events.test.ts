// frontend/src/tests/unit/events.test.ts
import { describe, it, expect } from "vitest";
import { EventQueue } from "../../store/events";
import type { SimEvent } from "../../protocol/messages";

const tick = (n: number): SimEvent => ({ tag: "Tick", tick: n });

describe("EventQueue", () => {
  it("enqueue + drain in FIFO order", () => {
    const q = new EventQueue();
    q.enqueue(tick(1));
    q.enqueue(tick(2));
    q.enqueue(tick(3));
    const out: SimEvent[] = [];
    q.drainInto(out);
    expect(out.map((e) => (e.tag === "Tick" ? e.tick : -1))).toEqual([1, 2, 3]);
    expect(q.length).toBe(0);
  });

  it("reuses the output array (zero-alloc contract)", () => {
    const q = new EventQueue();
    q.enqueue(tick(1));
    const out: SimEvent[] = [];
    const first = q.drainInto(out);
    expect(first).toBe(out);
  });

  it("returns false on overflow", () => {
    const q = new EventQueue();
    let ok = true;
    for (let i = 0; i < 2048; i++) {
      const r = q.enqueue(tick(i));
      if (!r) {
        ok = false;
        break;
      }
    }
    expect(ok).toBe(false);
  });

  it("enqueueAll counts inserted events even if it stops on overflow", () => {
    const q = new EventQueue();
    const events: SimEvent[] = [];
    for (let i = 0; i < 5; i++) events.push(tick(i));
    expect(q.enqueueAll(events)).toBe(5);
    expect(q.length).toBe(5);
  });
});
