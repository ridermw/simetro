// frontend/src/transport/mock.ts
//
// PLAN §3.3 / §17.2 — In browser-only dev (no Tauri shell) the renderer
// needs a deterministic stream of SimMessages so the visual loop works
// before the Rust transport is wired up. This mock emits a single
// `Static` + `Snapshot` pair drawn from games/demo-paths.json equivalent.
// Step 17 will animate it. Step 22 swaps in a real Tauri transport that
// shares the same Transport interface.
//
//   tick 0          (Static + initial Snapshot)
//      │
//      ▼
//   subscribe()  ──▶ messages dispatched to renderer/inspector/ui
//
// The mock is also what Playwright drives in `deterministic=true` mode.

import {
  type Envelope,
  type SimMessage,
  SCHEMA_VERSION,
} from "../protocol/messages";

export type MessageHandler = (msg: SimMessage) => void;

export interface Transport {
  connect(handler: MessageHandler): void;
  disconnect(): void;
  readonly name: string;
}

const DEMO_PALETTE = ["#0e1116", "#e8eaed", "#7aa2f7", "#bb9af7", "#9ece6a"];

const DEMO_STATIC: SimMessage = {
  type: "Static",
  payload: {
    scene_name: "demo-paths",
    theme: {
      palette: DEMO_PALETTE,
      background_index: 0,
      font: "system-ui",
    },
    id_map: {
      1: "a",
      2: "b",
      3: "c",
      4: "ab",
      5: "bc",
      6: "ca",
      7: "m1",
    },
  },
};

const DEMO_SNAPSHOT: SimMessage = {
  type: "Snapshot",
  payload: {
    tick: 0,
    nodes: [
      { id: 1, pos: [200, 200], shape: "circle", color: 2 },
      { id: 2, pos: [600, 200], shape: "square", color: 3 },
      { id: 3, pos: [400, 480], shape: "triangle", color: 4 },
    ],
    paths: [
      { id: 4, from: 1, to: 2, color: 2 },
      { id: 5, from: 2, to: 3, color: 3 },
      { id: 6, from: 3, to: 1, color: 4 },
    ],
    movers: [{ id: 7, pos: [200, 200], on_path: 4, speed: 1.0 }],
  },
};

export class MockTransport implements Transport {
  readonly name = "mock";
  private handler: MessageHandler | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;

  connect(handler: MessageHandler): void {
    this.handler = handler;
    // Defer one tick so callers can finish wiring before the first
    // message arrives — matches the real transport's async surface.
    this.timer = setTimeout(() => {
      this.handler?.(DEMO_STATIC);
      this.handler?.(DEMO_SNAPSHOT);
    }, 0);
  }

  disconnect(): void {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    this.handler = null;
  }
}

// Helpers exposed so unit tests don't have to spin up a full transport.
export function demoStaticEnvelope(): Envelope<SimMessage> {
  return { schema_version: SCHEMA_VERSION, seq: 0, payload: DEMO_STATIC };
}
export function demoSnapshotEnvelope(): Envelope<SimMessage> {
  return { schema_version: SCHEMA_VERSION, seq: 1, payload: DEMO_SNAPSHOT };
}
