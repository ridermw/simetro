// frontend/src/transport/mock.ts
//
// PLAN §3.3 / P1.5 Step 1 — Animated mock transport for browser-only dev.
//
// In browser dev (no Tauri shell) this transport drives the renderer with
// a continuous stream of snapshots + semantic events so movers visibly
// loop the demo-paths triangle. When the Tauri desktop shell is present,
// TauriTransport takes over and this mock is unused at runtime (but still
// drives Playwright E2E tests in browser mode).
//
//   t=0  (deferred)  →  static + initial snapshot
//   t=50ms, 100ms, … →  snapshot (positions advanced)
//                        + events (arrivals / departures / path pulses)
//   every ~1.5s       →  synthetic agent_report for the inspector
//
// The mock is also what Playwright drives in `deterministic=true` mode.

import {
  type Envelope,
  type SimMessage,
  type SimEvent,
  type MoverState,
  type AgentReport,
  SCHEMA_VERSION,
} from "../protocol/messages";

export type MessageHandler = (msg: SimMessage) => void;

export interface Transport {
  connect(handler: MessageHandler): void;
  disconnect(): void;
  readonly name: string;
}

// ---------------------------------------------------------------------------
//  Demo scene data (mirrors games/demo-paths.json)
// ---------------------------------------------------------------------------

const DEMO_PALETTE = ["#0e1116", "#e8eaed", "#7aa2f7", "#bb9af7", "#9ece6a"];

/** Path definitions: id, from_pos, to_pos, and routing to next path. */
const PATHS = [
  { id: 4, from_pos: [200, 200] as [number, number], to_pos: [600, 200] as [number, number], fromNode: 1, toNode: 2, next: 5 },
  { id: 5, from_pos: [600, 200] as [number, number], to_pos: [400, 480] as [number, number], fromNode: 2, toNode: 3, next: 6 },
  { id: 6, from_pos: [400, 480] as [number, number], to_pos: [200, 200] as [number, number], fromNode: 3, toNode: 1, next: 4 },
];

function pathById(id: number) {
  return PATHS.find((p) => p.id === id)!;
}

interface MockMover {
  id: number;
  pathId: number;
  progress: number; // 0..1 along current segment
  speed: number;    // units per second (path length normalized to ~400px)
}

const TICK_INTERVAL_MS = 50;
const AGENT_REPORT_INTERVAL = 30; // every 30 snapshots ≈ 1.5s

const DEMO_STATIC: SimMessage = {
  kind: "static",
  payload: {
    name: "demo-paths",
    palette: DEMO_PALETTE,
    background_index: 0,
    nodes: [
      { id: 1, pos: [200, 200], shape: "circle", color: 2 },
      { id: 2, pos: [600, 200], shape: "square", color: 3 },
      { id: 3, pos: [400, 480], shape: "triangle", color: 4 },
    ],
    paths: [
      { id: 4, from_pos: [200, 200], to_pos: [600, 200], color: 2 },
      { id: 5, from_pos: [600, 200], to_pos: [400, 480], color: 3 },
      { id: 6, from_pos: [400, 480], to_pos: [200, 200], color: 4 },
    ],
    node_names: { 1: "a", 2: "b", 3: "c" },
    path_names: { 4: "ab", 5: "bc", 6: "ca" },
    mover_names: { 7: "m1", 8: "m2", 9: "m3" },
  },
};

function pathLength(pathId: number): number {
  const p = pathById(pathId);
  const dx = p.to_pos[0] - p.from_pos[0];
  const dy = p.to_pos[1] - p.from_pos[1];
  return Math.sqrt(dx * dx + dy * dy);
}

function moverPos(m: MockMover): [number, number] {
  const p = pathById(m.pathId);
  const t = Math.min(1, Math.max(0, m.progress));
  return [
    p.from_pos[0] + (p.to_pos[0] - p.from_pos[0]) * t,
    p.from_pos[1] + (p.to_pos[1] - p.from_pos[1]) * t,
  ];
}

function initialMovers(): MockMover[] {
  return [
    { id: 7, pathId: 4, progress: 0, speed: 0.8 },
    { id: 8, pathId: 5, progress: 0, speed: 1.0 },
    { id: 9, pathId: 6, progress: 0, speed: 1.2 },
  ];
}

function encodeSnapshot(tick: number, movers: MockMover[]): SimMessage {
  const moverStates: MoverState[] = movers.map((m) => ({
    id: m.id,
    pos: moverPos(m),
    on_path: m.pathId,
    speed: m.speed,
  }));
  return { kind: "snapshot", payload: { tick, movers: moverStates } };
}

// ---------------------------------------------------------------------------
//  MockTransport
// ---------------------------------------------------------------------------

export class MockTransport implements Transport {
  readonly name = "mock";
  private handler: MessageHandler | null = null;
  private initTimer: ReturnType<typeof setTimeout> | null = null;
  private interval: ReturnType<typeof setInterval> | null = null;
  private movers: MockMover[] = [];
  private tick = 0;
  private snapshotCount = 0;

  connect(handler: MessageHandler): void {
    this.handler = handler;
    this.movers = initialMovers();
    this.tick = 0;
    this.snapshotCount = 0;

    // Defer initial messages one microtask so callers finish wiring
    // before first dispatch — matches the real transport's async surface.
    this.initTimer = setTimeout(() => {
      this.initTimer = null;
      this.handler?.(DEMO_STATIC);
      this.handler?.(encodeSnapshot(0, this.movers));
      // Start the animation loop after emitting initial state.
      this.interval = setInterval(() => this.step(), TICK_INTERVAL_MS);
    }, 0);
  }

  disconnect(): void {
    if (this.initTimer !== null) {
      clearTimeout(this.initTimer);
      this.initTimer = null;
    }
    if (this.interval !== null) {
      clearInterval(this.interval);
      this.interval = null;
    }
    this.handler = null;
  }

  private step(): void {
    if (this.handler === null) return;

    this.tick += 1;
    const dt = TICK_INTERVAL_MS / 1000; // seconds per interval
    const events: SimEvent[] = [];

    // Advance each mover along its current path.
    for (const m of this.movers) {
      const len = pathLength(m.pathId);
      const progressDelta = (m.speed * dt * 120) / len; // scale so movement is visible
      m.progress += progressDelta;

      // Handle arrival at end of segment.
      while (m.progress >= 1.0) {
        m.progress -= 1.0;
        const arrivedPath = pathById(m.pathId);
        events.push({ kind: "mover_arrived", mover: m.id, at_node: arrivedPath.toNode, path: m.pathId });
        events.push({ kind: "path_pulsed", path: m.pathId });
        // Route to next segment.
        m.pathId = arrivedPath.next;
        const departedPath = pathById(m.pathId);
        events.push({ kind: "mover_departed", mover: m.id, from_node: departedPath.fromNode, path: m.pathId });
      }
    }

    // Emit events batch if there are semantic events.
    if (events.length > 0) {
      this.handler({ kind: "events", payload: events });
    }

    // Emit snapshot.
    this.handler(encodeSnapshot(this.tick, this.movers));

    // Emit agent report periodically.
    this.snapshotCount += 1;
    if (this.snapshotCount % AGENT_REPORT_INTERVAL === 0) {
      this.handler({ kind: "agent_report", payload: syntheticAgentReport(this.tick) });
    }
  }
}

function syntheticAgentReport(tick: number): AgentReport {
  return {
    tick,
    agent_id: "speed_tuner_0",
    considered: [
      { action: { kind: "set_speed", mover: 7, speed: 1.0 }, confidence: 0.7 },
      { action: { kind: "no_op" }, confidence: 0.3 },
    ],
    chosen: { kind: "set_speed", mover: 7, speed: 1.0 },
    rationale: "maintaining optimal flow through triangle loop",
    confidence: 0.7,
  };
}

// ---------------------------------------------------------------------------
//  Helpers for unit tests
// ---------------------------------------------------------------------------

/** Initial static message (for tests that need the scene without running transport). */
export function demoStaticEnvelope(): Envelope<SimMessage> {
  return { schema_version: SCHEMA_VERSION, seq: 0, payload: DEMO_STATIC };
}

/** Initial snapshot message at tick 0 with all three movers. */
export function demoSnapshotEnvelope(): Envelope<SimMessage> {
  return {
    schema_version: SCHEMA_VERSION,
    seq: 1,
    payload: encodeSnapshot(0, initialMovers()),
  };
}
