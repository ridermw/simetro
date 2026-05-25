// frontend/src/transport/mock.ts
//
// browser-only animated mock transport — Animated mock transport for browser-only dev.
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
  type Sl1AlertView,
  type Sl1DashboardView,
  type Sl1DashboardStateView,
  type Sl1AlertStateView,
  type Sl1MilestoneView,
  type Sl1GameOutcomeView,
  type StaticPayload,
  type SnapshotPayload,
  SCHEMA_VERSION,
} from "../protocol/messages";

export type MessageHandler = (msg: SimMessage) => void;

export interface Transport {
  connect(handler: MessageHandler): void;
  disconnect(): void;
  readonly name: string;
}

export interface MockTransportOptions {
  /**
   * When true, the mock decorates `DEMO_STATIC` with SL1 observability
   * metadata (one dashboard, one alert, two milestones) and runs a
   * scripted timeline so the SL1 HUD components mount and exercise
   * each state transition. Default false — the mock then behaves
   * exactly like the non-SL1 demo so legacy tests stay stable.
   */
  sl1Mode?: boolean;
}

/** Read URL query string and decide whether to enable SL1 mock mode. */
export function sl1ModeFromLocation(search: string | undefined): boolean {
  if (search === undefined) return false;
  const params = new URLSearchParams(search);
  return params.get("sl1demo") === "1";
}

// ---------------------------------------------------------------------------
//  Demo scene data (mirrors games/demo-paths.json)
// ---------------------------------------------------------------------------

const DEMO_PALETTE = ["#0e1116", "#e8eaed", "#7aa2f7", "#bb9af7", "#9ece6a"];

/** Path definitions: id, from_pos, to_pos, and routing to next path. */
const PATHS = [
  {
    id: 4,
    from_pos: [200, 200] as [number, number],
    to_pos: [600, 200] as [number, number],
    fromNode: 1,
    toNode: 2,
    next: 5,
  },
  {
    id: 5,
    from_pos: [600, 200] as [number, number],
    to_pos: [400, 480] as [number, number],
    fromNode: 2,
    toNode: 3,
    next: 6,
  },
  {
    id: 6,
    from_pos: [400, 480] as [number, number],
    to_pos: [200, 200] as [number, number],
    fromNode: 3,
    toNode: 1,
    next: 4,
  },
];

function pathById(id: number) {
  return PATHS.find((p) => p.id === id)!;
}

interface MockMover {
  id: number;
  pathId: number;
  progress: number; // 0..1 along current segment
  speed: number; // units per second (path length normalized to ~400px)
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

// ---------------------------------------------------------------------------
//  SL1 demo metadata (browser-only mock; surfaces SL1 HUD panels for
//  Playwright E2E without requiring the Tauri shell + Rust engine).
// ---------------------------------------------------------------------------

const SL1_DASHBOARDS: Sl1DashboardView[] = [
  {
    id: "exec-dashboard",
    type: "executive",
    depends_on: ["telemetry"],
    freshness_slo_ticks: 40,
  },
];

const SL1_ALERTS: Sl1AlertView[] = [
  {
    id: "exec-dashboard-stale",
    metric: "exec-dashboard-freshness",
    predicate: { kind: "gt", threshold: 30 },
    severity: "warning",
  },
];

const SL1_MILESTONES: Sl1MilestoneView[] = [
  {
    id: "first-pressure",
    label: "Spot eviction wave begins",
    trigger_kind: "pressure_activated",
    trigger: { type: "pressure_activated", pressure: "evict-1" },
  },
  {
    id: "exec-recovered",
    label: "Executive dashboard recovered",
    trigger_kind: "dashboard_state",
    trigger: {
      type: "dashboard_state",
      dashboard: "exec-dashboard",
      state: "ok",
    },
  },
];

function sl1StaticMessage(): SimMessage {
  // Augment DEMO_STATIC with SL1 metadata. We rebuild the payload to
  // avoid mutating the shared constant (other tests rely on it).
  const base = DEMO_STATIC.payload as StaticPayload;
  return {
    kind: "static",
    payload: {
      ...base,
      sl1_observability_dashboards: SL1_DASHBOARDS,
      sl1_observability_alerts: SL1_ALERTS,
      sl1_milestones: SL1_MILESTONES,
    },
  };
}

interface Sl1Step {
  /** Snapshot tick at which this step takes effect. */
  atTick: number;
  outcome?: Sl1GameOutcomeView;
  phase?: string;
  dashboardStates?: Sl1DashboardStateView[];
  alertStates?: Sl1AlertStateView[];
  events?: SimEvent[];
}

/** Scripted SL1 timeline so the HUD shows every state.
 *
 *  Timings are chosen so the firing/stale window is wide enough for
 *  Playwright to observe (≥1s) without making the suite long. At
 *  50ms/tick the schedule is ~1.5s end-to-end. */
const SL1_SCRIPT: Sl1Step[] = [
  {
    atTick: 1,
    outcome: { state: "in_progress" },
    phase: "winning",
    dashboardStates: [{ dashboard_id: "exec-dashboard", state: "ok", freshness_ticks: 0 }],
    alertStates: [{ alert_id: "exec-dashboard-stale", state: "inactive" }],
  },
  {
    atTick: 2,
    events: [
      {
        kind: "sl1_milestone_fired",
        milestone_id: "first-pressure",
        label: "Spot eviction wave begins",
        trigger_kind: "pressure_activated",
        tick: 2,
      },
    ],
  },
  {
    atTick: 5,
    outcome: { state: "in_progress" },
    phase: "spiraling",
    dashboardStates: [{ dashboard_id: "exec-dashboard", state: "stale", freshness_ticks: 35 }],
    alertStates: [{ alert_id: "exec-dashboard-stale", state: "firing", fired_at_tick: 5 }],
  },
  {
    atTick: 25,
    outcome: { state: "in_progress" },
    phase: "stabilizing",
    dashboardStates: [{ dashboard_id: "exec-dashboard", state: "ok", freshness_ticks: 0 }],
    alertStates: [{ alert_id: "exec-dashboard-stale", state: "inactive" }],
    events: [
      {
        kind: "sl1_milestone_fired",
        milestone_id: "exec-recovered",
        label: "Executive dashboard recovered",
        trigger_kind: "dashboard_state",
        tick: 25,
      },
    ],
  },
  {
    atTick: 30,
    outcome: { state: "won" },
    phase: "winning",
  },
];

export class MockTransport implements Transport {
  readonly name = "mock";
  private handler: MessageHandler | null = null;
  private initTimer: ReturnType<typeof setTimeout> | null = null;
  private interval: ReturnType<typeof setInterval> | null = null;
  private movers: MockMover[] = [];
  private tick = 0;
  private snapshotCount = 0;
  private sl1Mode: boolean;
  private sl1LastOutcome: Sl1GameOutcomeView | undefined;
  private sl1LastPhase: string | undefined;
  private sl1LastDashboardStates: Sl1DashboardStateView[] | undefined;
  private sl1LastAlertStates: Sl1AlertStateView[] | undefined;

  constructor(options: MockTransportOptions = {}) {
    this.sl1Mode = options.sl1Mode === true;
  }

  connect(handler: MessageHandler): void {
    this.handler = handler;
    this.movers = initialMovers();
    this.tick = 0;
    this.snapshotCount = 0;
    this.sl1LastOutcome = undefined;
    this.sl1LastPhase = undefined;
    this.sl1LastDashboardStates = undefined;
    this.sl1LastAlertStates = undefined;

    // Defer initial messages one microtask so callers finish wiring
    // before first dispatch — matches the real transport's async surface.
    this.initTimer = setTimeout(() => {
      this.initTimer = null;
      this.handler?.(this.sl1Mode ? sl1StaticMessage() : DEMO_STATIC);
      this.handler?.(this.encodeSnapshot(0));
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

  private encodeSnapshot(tick: number): SimMessage {
    const moverStates: MoverState[] = this.movers.map((m) => ({
      id: m.id,
      pos: moverPos(m),
      on_path: m.pathId,
      speed: m.speed,
    }));
    if (!this.sl1Mode) {
      return { kind: "snapshot", payload: { tick, movers: moverStates } };
    }
    // Only attach SL1 fields when defined; exactOptionalPropertyTypes
    // rejects `undefined` literals on optional properties.
    const payload: SnapshotPayload = { tick, movers: moverStates };
    if (this.sl1LastOutcome !== undefined) payload.sl1_game_outcome = this.sl1LastOutcome;
    if (this.sl1LastPhase !== undefined) payload.sl1_game_phase = this.sl1LastPhase;
    if (this.sl1LastDashboardStates !== undefined) {
      payload.sl1_dashboard_states = this.sl1LastDashboardStates;
    }
    if (this.sl1LastAlertStates !== undefined) payload.sl1_alert_states = this.sl1LastAlertStates;
    return { kind: "snapshot", payload };
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
        events.push({
          kind: "mover_arrived",
          mover: m.id,
          at_node: arrivedPath.toNode,
          path: m.pathId,
        });
        events.push({ kind: "path_pulsed", path: m.pathId });
        // Route to next segment.
        m.pathId = arrivedPath.next;
        const departedPath = pathById(m.pathId);
        events.push({
          kind: "mover_departed",
          mover: m.id,
          from_node: departedPath.fromNode,
          path: m.pathId,
        });
      }
    }

    // Apply scripted SL1 state changes (mutates last* fields so the
    // snapshot emitted below carries the latest values).
    if (this.sl1Mode) {
      for (const stepEntry of SL1_SCRIPT) {
        if (stepEntry.atTick === this.tick) {
          if (stepEntry.outcome !== undefined) this.sl1LastOutcome = stepEntry.outcome;
          if (stepEntry.phase !== undefined) this.sl1LastPhase = stepEntry.phase;
          if (stepEntry.dashboardStates !== undefined) {
            this.sl1LastDashboardStates = stepEntry.dashboardStates;
          }
          if (stepEntry.alertStates !== undefined) {
            this.sl1LastAlertStates = stepEntry.alertStates;
          }
          if (stepEntry.events !== undefined) {
            events.push(...stepEntry.events);
          }
        }
      }
    }

    // Emit events batch if there are semantic events.
    if (events.length > 0) {
      this.handler({ kind: "events", payload: events });
    }

    // Emit snapshot.
    this.handler(this.encodeSnapshot(this.tick));

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
