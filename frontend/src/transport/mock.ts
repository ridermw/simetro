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
  type Sl1FailureConditionRuntimeView,
  type Sl1FailureConditionView,
  type Sl1GamePhase,
  type Sl1MetricStateView,
  type Sl1MetricView,
  type Sl1ObjectiveRuntimeView,
  type Sl1ObjectiveView,
  type Sl1VictoryConditionRuntimeView,
  type Sl1VictoryConditionView,
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
  /** When set, mock fetches /static-payloads/{sceneId}.json instead of using DEMO_STATIC. */
  sceneId?: string;
}

/** Read URL query string and decide whether to enable SL1 mock mode. */
export function sl1ModeFromLocation(search: string | undefined): boolean {
  if (search === undefined) return false;
  const params = new URLSearchParams(search);
  return params.get("sl1demo") === "1";
}

export interface Sl1SceneMeta {
  metrics: Sl1MetricView[];
  objectives: Sl1ObjectiveView[];
  failures: Sl1FailureConditionView[];
  victories: Sl1VictoryConditionView[];
}

export interface Sl1MockRuntime {
  metric_states: Sl1MetricStateView[];
  objective_states: Sl1ObjectiveRuntimeView[];
  failure_condition_states: Sl1FailureConditionRuntimeView[];
  victory_condition_states: Sl1VictoryConditionRuntimeView[];
  phase: Sl1GamePhase;
}

export function payloadHasNativeSl1(payload: StaticPayload): boolean {
  return (
    (payload.sl1_places?.length ?? 0) > 0 ||
    (payload.sl1_observability_metrics?.length ?? 0) > 0 ||
    (payload.sl1_objectives?.length ?? 0) > 0 ||
    (payload.sl1_failure_conditions?.length ?? 0) > 0 ||
    (payload.sl1_victory_conditions?.length ?? 0) > 0
  );
}

export function safeTick(t: number): number {
  if (!Number.isFinite(t)) return 0;
  return ((t % 100000) + 100000) % 100000;
}

const SL1_MOCK_BREACH_START_TICK = 60;
const SL1_MOCK_BREACH_END_TICK = 100;

function isSl1MockBreachActive(tick: number): boolean {
  return tick >= SL1_MOCK_BREACH_START_TICK && tick <= SL1_MOCK_BREACH_END_TICK;
}

function sl1MockBreachTickCount(tick: number): number {
  return isSl1MockBreachActive(tick) ? tick - SL1_MOCK_BREACH_START_TICK + 1 : 0;
}

function sl1MockBreachObjectiveId(scene: Sl1SceneMeta): string | undefined {
  const objectiveIds = new Set(scene.objectives.map((objective) => objective.id));
  const targetedFailure = scene.failures.find(
    (failure) =>
      failure.params.kind === "objective_breach_count" && objectiveIds.has(failure.params.objective_id)
  );
  if (targetedFailure?.params.kind === "objective_breach_count") {
    return targetedFailure.params.objective_id;
  }
  return scene.objectives[0]?.id;
}

function sl1MockBreachFailureId(scene: Sl1SceneMeta, breachObjectiveId: string | undefined): string | undefined {
  if (breachObjectiveId !== undefined) {
    const targetedFailure = scene.failures.find(
      (failure) =>
        failure.params.kind === "objective_breach_count" && failure.params.objective_id === breachObjectiveId
    );
    if (targetedFailure !== undefined) return targetedFailure.id;
  }
  return scene.failures[0]?.id;
}

function sl1FailureFireThreshold(failure: Sl1FailureConditionView): number {
  const rawThreshold = (() => {
    switch (failure.params.kind) {
      case "stale_target":
        return failure.params.threshold_ticks;
      case "place_state":
        return failure.params.grace_ticks;
      case "objective_breach_count":
        return failure.params.max_count;
    }
  })();
  return Math.min(Math.max(Math.floor(rawThreshold), 20), 30);
}

export function computeSl1MockRuntime(rawTick: number, scene: Sl1SceneMeta): Sl1MockRuntime {
  const tick = safeTick(rawTick);
  const breachObjectiveId = sl1MockBreachObjectiveId(scene);
  const breachFailureId = sl1MockBreachFailureId(scene, breachObjectiveId);
  const breachCount = sl1MockBreachTickCount(tick);
  const breachActive = breachCount > 0;

  return {
    metric_states: scene.metrics.map((metric, index) => {
      switch (metric.source.kind) {
        case "place_capacity_used_percent":
          return {
            metric_id: metric.id,
            state: "ok",
            value: Math.max(0, Math.min(100, 50 + 30 * Math.sin(tick / 40 + index * 0.17))),
          };
        case "place_inventory_count":
          return {
            metric_id: metric.id,
            state: "ok",
            value: Math.abs(Math.floor(Math.sin(tick * 0.13 + index) * 200)),
          };
        case "dashboard_freshness":
          return {
            metric_id: metric.id,
            state: "ok",
            value: tick % 90,
          };
      }
    }),
    objective_states: scene.objectives.map((objective) => ({
      objective_id: objective.id,
      status: breachActive && objective.id === breachObjectiveId ? "breached" : "met",
      breach_tick_count: breachActive && objective.id === breachObjectiveId ? breachCount : 0,
    })),
    failure_condition_states: scene.failures.map((failure) => {
      const breachStreakTicks = breachActive && failure.id === breachFailureId ? breachCount : 0;
      const state: Sl1FailureConditionRuntimeView = {
        failure_condition_id: failure.id,
        breach_streak_ticks: breachStreakTicks,
      };
      if (breachStreakTicks > sl1FailureFireThreshold(failure)) {
        state.fired_at_tick = tick;
      }
      return state;
    }),
    victory_condition_states: scene.victories.map((victory) => {
      const state: Sl1VictoryConditionRuntimeView = { victory_condition_id: victory.id };
      if (victory.params.kind === "survive_until" && tick >= victory.params.at_tick) {
        state.met_at_tick = victory.params.at_tick;
      }
      return state;
    }),
    phase: breachActive ? "losing" : "winning",
  };
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

function sl1StaticMessage(basePayload: StaticPayload = DEMO_STATIC.payload as StaticPayload): SimMessage {
  // Augment the chosen static payload with SL1 metadata. We rebuild
  // the payload to avoid mutating shared constants or fetched objects.
  return {
    kind: "static",
    payload: {
      ...basePayload,
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
  private readonly sl1Mode: boolean;
  private readonly sceneId: string | undefined;
  private sl1LastOutcome: Sl1GameOutcomeView | undefined;
  private sl1LastPhase: string | undefined;
  private sl1LastDashboardStates: Sl1DashboardStateView[] | undefined;
  private sl1LastAlertStates: Sl1AlertStateView[] | undefined;
  private sl1Scene: Sl1SceneMeta | null = null;

  constructor(options: MockTransportOptions = {}) {
    this.sl1Mode = options.sl1Mode === true;
    this.sceneId = options.sceneId;
  }

  private async loadExternalStatic(sceneId: string, handler: MessageHandler): Promise<void> {
    try {
      const resp = await fetch(`/static-payloads/${sceneId}.json`);
      if (!resp.ok) {
        const msg = `static payload fetch failed for ${sceneId} (HTTP ${resp.status})`;
        console.error(`simetro: ${msg}`);
        handler({
          kind: "fault",
          payload: { kind: "load_error", message: msg, line: null, col: null },
        });
        return;
      }
      const envelope = (await resp.json()) as { schema_version: number; payload: StaticPayload };
      if (envelope.schema_version !== SCHEMA_VERSION) {
        const msg = `schema mismatch for ${sceneId}: got ${envelope.schema_version}, want ${SCHEMA_VERSION}`;
        console.error(`simetro: ${msg}`);
        handler({
          kind: "fault",
          payload: { kind: "load_error", message: msg, line: null, col: null },
        });
        return;
      }
      const payload = envelope.payload;
      const hasSl1Scene = payloadHasNativeSl1(payload);
      this.sl1Scene = hasSl1Scene
        ? {
            metrics: payload.sl1_observability_metrics ?? [],
            objectives: payload.sl1_objectives ?? [],
            failures: payload.sl1_failure_conditions ?? [],
            victories: payload.sl1_victory_conditions ?? [],
          }
        : null;
      const shouldApplyLegacySl1Decoration = this.sl1Mode && !hasSl1Scene;
      handler(shouldApplyLegacySl1Decoration ? sl1StaticMessage(payload) : { kind: "static", payload });
    } catch (e) {
      const msg = `failed to load static for ${sceneId}: ${e instanceof Error ? e.message : String(e)}`;
      console.error(`simetro: ${msg}`);
      handler({
        kind: "fault",
        payload: { kind: "load_error", message: msg, line: null, col: null },
      });
    }
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
    this.sl1Scene = null;

    // Defer initial messages one microtask so callers finish wiring
    // before first dispatch — matches the real transport's async surface.
    this.initTimer = setTimeout(() => {
      this.initTimer = null;
      const emitInitialSnapshot = () => {
        if (this.handler === null) return;
        this.handler(this.encodeSnapshot(0));
        // Start the animation loop after emitting initial state.
        this.interval = setInterval(() => this.step(), TICK_INTERVAL_MS);
      };
      if (this.sceneId !== undefined) {
        void this.loadExternalStatic(this.sceneId, (msg) => {
          if (this.handler === null) return;
          this.handler(msg);
          // Only start snapshot stream if the static actually loaded.
          // On load_error the world has no geometry to interpolate against.
          if (msg.kind === "static") emitInitialSnapshot();
        });
        return;
      }
      this.handler?.(this.sl1Mode ? sl1StaticMessage() : DEMO_STATIC);
      emitInitialSnapshot();
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
    // Only attach SL1 fields when defined; exactOptionalPropertyTypes
    // rejects `undefined` literals on optional properties.
    const payload: SnapshotPayload = { tick, movers: moverStates };
    if (this.sl1Scene !== null) {
      const runtime = computeSl1MockRuntime(tick, this.sl1Scene);
      payload.sl1_metric_states = runtime.metric_states;
      payload.sl1_objective_states = runtime.objective_states;
      payload.sl1_failure_condition_states = runtime.failure_condition_states;
      payload.sl1_victory_condition_states = runtime.victory_condition_states;
      payload.sl1_game_phase = runtime.phase;
    }
    if (this.sl1Mode && this.sl1Scene === null) {
      if (this.sl1LastOutcome !== undefined) payload.sl1_game_outcome = this.sl1LastOutcome;
      if (this.sl1LastPhase !== undefined) payload.sl1_game_phase = this.sl1LastPhase;
      if (this.sl1LastDashboardStates !== undefined) {
        payload.sl1_dashboard_states = this.sl1LastDashboardStates;
      }
      if (this.sl1LastAlertStates !== undefined) payload.sl1_alert_states = this.sl1LastAlertStates;
    }
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
    if (this.sl1Mode && this.sl1Scene === null) {
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
