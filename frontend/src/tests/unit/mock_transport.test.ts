// frontend/src/tests/unit/mock_transport.test.ts
import { describe, it, expect, vi, afterEach } from "vitest";
import { MockTransport, payloadHasNativeSl1, sl1ModeFromLocation } from "../../transport/mock";
import type { SimMessage, StaticPayload } from "../../protocol/messages";
import { SCHEMA_VERSION } from "../../protocol/messages";

const EXTERNAL_STATIC_PAYLOAD: StaticPayload = {
  name: "external-scene",
  palette: ["#101820", "#f2aa4c"],
  background_index: 0,
  nodes: [{ id: 1, pos: [10, 20], shape: "circle", color: 1 }],
  paths: [],
  node_names: { 1: "entry" },
  path_names: {},
  mover_names: {},
};

const GPU_LAUNCH_WEEK_STATIC_PAYLOAD: StaticPayload = {
  ...EXTERNAL_STATIC_PAYLOAD,
  name: "gpu-launch-week",
  sl1_places: [{ id: "gpu-platform", role: "compute_cluster", pos: [0, 0] }],
  sl1_objectives: [
    {
      id: "keep-dashboard-fresh",
      type: "keep_fresh",
      weight: 3,
      params: { kind: "keep_fresh", place: "gpu-platform", thing: "dashboard_result", max_stale_ticks: 240 },
    },
    {
      id: "no-dropped-refreshes",
      type: "complete_jobs_before_deadline",
      weight: 2,
      params: { kind: "complete_jobs_before_deadline", demand: "exec-dashboard-refresh", max_missed: 1 },
    },
  ],
  sl1_failure_conditions: [
    {
      id: "refresh-objective-breached",
      type: "objective_breach_count",
      params: { kind: "objective_breach_count", objective_id: "no-dropped-refreshes", max_count: 3 },
    },
  ],
  sl1_victory_conditions: [
    { id: "survive-launch-week", type: "survive_until", params: { kind: "survive_until", at_tick: 2800 } },
  ],
  sl1_observability_metrics: [
    {
      id: "platform-compute-load",
      source: { kind: "place_capacity_used_percent", place: "gpu-platform", capacity: "compute_units" },
    },
    {
      id: "heartbeat-backlog",
      source: { kind: "place_inventory_count", place: "gpu-platform", thing: "gpu_heartbeat" },
    },
    {
      id: "exec-dashboard-freshness",
      source: { kind: "dashboard_freshness", dashboard: "exec-report" },
    },
  ],
  sl1_milestones: [
    {
      id: "dashboard-storm",
      label: "Dashboard storm",
      trigger_kind: "pressure_activated",
      trigger: { type: "pressure_activated", pressure: "dashboard-storm" },
      camera_focus: ["gpu-platform"],
      highlight: "exec-dashboard",
    },
    {
      id: "gpu-storm",
      label: "GPU storm",
      trigger_kind: "pressure_activated",
      trigger: { type: "pressure_activated", pressure: "gpu-storm" },
    },
  ],
};

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("MockTransport", () => {
  it("emits static then snapshot to the handler", async () => {
    const t = new MockTransport();
    const received: SimMessage[] = [];
    t.connect((m) => received.push(m));

    await new Promise((resolve) => setTimeout(resolve, 10));

    expect(received.length).toBe(2);
    expect(received[0]?.kind).toBe("static");
    expect(received[1]?.kind).toBe("snapshot");
    t.disconnect();
  });

  it("does not emit after disconnect", async () => {
    const t = new MockTransport();
    const cb = vi.fn();
    t.connect(cb);
    t.disconnect();

    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(cb).not.toHaveBeenCalled();
  });

  it("identifies itself as mock", () => {
    expect(new MockTransport().name).toBe("mock");
  });

  it("fetches external static payloads when sceneId is provided", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ schema_version: SCHEMA_VERSION, payload: EXTERNAL_STATIC_PAYLOAD }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );
    vi.stubGlobal("fetch", fetchMock);

    const t = new MockTransport({ sceneId: "gpu-launch-week" });
    const received: SimMessage[] = [];
    t.connect((m) => received.push(m));

    await new Promise((resolve) => setTimeout(resolve, 10));

    expect(fetchMock).toHaveBeenCalledWith("/static-payloads/gpu-launch-week.json");
    expect(received[0]).toEqual({ kind: "static", payload: EXTERNAL_STATIC_PAYLOAD });
    expect(received[1]?.kind).toBe("snapshot");
    t.disconnect();
  });

  it("emits a load_error fault when fetched external payload schema mismatches", async () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ schema_version: SCHEMA_VERSION + 1, payload: EXTERNAL_STATIC_PAYLOAD }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );
    vi.stubGlobal("fetch", fetchMock);

    const t = new MockTransport({ sceneId: "gpu-launch-week" });
    const received: SimMessage[] = [];
    t.connect((m) => received.push(m));

    await new Promise((resolve) => setTimeout(resolve, 10));

    expect(fetchMock).toHaveBeenCalledWith("/static-payloads/gpu-launch-week.json");
    expect(received[0]?.kind).toBe("fault");
    if (received[0]?.kind === "fault") {
      expect(received[0].payload.kind).toBe("load_error");
    }
    expect(consoleError).toHaveBeenCalled();
    t.disconnect();
  });

  it("decorates non-SL1 external static payloads when sl1Mode is true", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ schema_version: SCHEMA_VERSION, payload: EXTERNAL_STATIC_PAYLOAD }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );
    vi.stubGlobal("fetch", fetchMock);

    const t = new MockTransport({ sceneId: "demo-paths", sl1Mode: true });
    const received: SimMessage[] = [];
    t.connect((m) => received.push(m));

    await new Promise((resolve) => setTimeout(resolve, 10));

    const stat = received[0];
    expect(stat?.kind).toBe("static");
    if (stat?.kind === "static") {
      expect(stat.payload.name).toBe("external-scene");
      expect(stat.payload.sl1_observability_dashboards?.length).toBeGreaterThan(0);
      expect(stat.payload.sl1_observability_alerts?.length).toBeGreaterThan(0);
      expect(stat.payload.sl1_milestones?.length).toBeGreaterThan(0);
    }
    t.disconnect();
  });

  it("sl1Mode=false omits SL1 fields from static (legacy behavior)", async () => {
    const t = new MockTransport();
    const received: SimMessage[] = [];
    t.connect((m) => received.push(m));
    await new Promise((resolve) => setTimeout(resolve, 10));
    const stat = received[0];
    expect(stat?.kind).toBe("static");
    if (stat?.kind === "static") {
      expect(stat.payload.sl1_observability_dashboards).toBeUndefined();
      expect(stat.payload.sl1_observability_alerts).toBeUndefined();
      expect(stat.payload.sl1_milestones).toBeUndefined();
    }
    t.disconnect();
  });

  it("sl1Mode=true attaches SL1 metadata to static and SL1 state to snapshots", async () => {
    const t = new MockTransport({ sl1Mode: true });
    const received: SimMessage[] = [];
    t.connect((m) => received.push(m));
    // Let several ticks elapse so the scripted timeline emits at least
    // one milestone event and one outcome update.
    await new Promise((resolve) => setTimeout(resolve, 200));
    t.disconnect();

    const stat = received[0];
    expect(stat?.kind).toBe("static");
    if (stat?.kind === "static") {
      expect(stat.payload.sl1_observability_dashboards?.length).toBeGreaterThan(0);
      expect(stat.payload.sl1_observability_alerts?.length).toBeGreaterThan(0);
      expect(stat.payload.sl1_milestones?.length).toBeGreaterThan(0);
    }

    const milestoneEvent = received
      .filter((m): m is Extract<SimMessage, { kind: "events" }> => m.kind === "events")
      .flatMap((m) => m.payload)
      .find((ev) => ev.kind === "sl1_milestone_fired");
    expect(milestoneEvent).toBeDefined();
  });

  it("does not apply legacy decoration to native SL1 external scenes when sl1Mode is true", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ schema_version: SCHEMA_VERSION, payload: GPU_LAUNCH_WEEK_STATIC_PAYLOAD }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );
    vi.stubGlobal("fetch", fetchMock);

    const t = new MockTransport({ sceneId: "gpu-launch-week", sl1Mode: true });
    const received: SimMessage[] = [];
    t.connect((m) => received.push(m));

    await new Promise((resolve) => setTimeout(resolve, 70));
    t.disconnect();

    const stat = received[0];
    expect(stat?.kind).toBe("static");
    if (stat?.kind === "static") {
      const dashboardIds = stat.payload.sl1_observability_dashboards?.map((dashboard) => dashboard.id) ?? [];
      expect(dashboardIds).not.toContain("exec-dashboard");
      expect(dashboardIds).not.toContain("copilot-uptime");
      expect(stat.payload.sl1_observability_metrics?.map((metric) => metric.id)).toEqual([
        "platform-compute-load",
        "heartbeat-backlog",
        "exec-dashboard-freshness",
      ]);
    }

    const runtimeSnapshot = received.find(
      (m): m is Extract<SimMessage, { kind: "snapshot" }> =>
        m.kind === "snapshot" && (m.payload.sl1_metric_states?.length ?? 0) > 0
    );
    expect(runtimeSnapshot?.payload.sl1_metric_states?.map((state) => state.metric_id)).toEqual([
      "platform-compute-load",
      "heartbeat-backlog",
      "exec-dashboard-freshness",
    ]);
  });

  it("emits live SL1 runtime state for external SL1 scene metadata without sl1Mode", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ schema_version: SCHEMA_VERSION, payload: GPU_LAUNCH_WEEK_STATIC_PAYLOAD }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );
    vi.stubGlobal("fetch", fetchMock);

    const t = new MockTransport({ sceneId: "gpu-launch-week" });
    const received: SimMessage[] = [];
    t.connect((m) => received.push(m));

    await new Promise((resolve) => setTimeout(resolve, 70));
    t.disconnect();

    const snapshots = received.filter(
      (m): m is Extract<SimMessage, { kind: "snapshot" }> => m.kind === "snapshot"
    );
    expect(snapshots.length).toBeGreaterThanOrEqual(2);
    const secondSnapshot = snapshots[1]!;
    expect(secondSnapshot.payload.sl1_metric_states?.map((state) => state.metric_id)).toEqual([
      "platform-compute-load",
      "heartbeat-backlog",
      "exec-dashboard-freshness",
    ]);
    expect(secondSnapshot.payload.sl1_objective_states?.map((state) => state.objective_id)).toEqual([
      "keep-dashboard-fresh",
      "no-dropped-refreshes",
    ]);
    expect(secondSnapshot.payload.sl1_failure_condition_states?.map((state) => state.failure_condition_id)).toEqual([
      "refresh-objective-breached",
    ]);
    expect(secondSnapshot.payload.sl1_victory_condition_states).toEqual([
      { victory_condition_id: "survive-launch-week" },
    ]);
    expect(secondSnapshot.payload.sl1_game_phase).toBe("winning");
  });

  it("emits metadata-driven SL1 milestone events for external SL1 scenes", async () => {
    vi.useFakeTimers();
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ schema_version: SCHEMA_VERSION, payload: GPU_LAUNCH_WEEK_STATIC_PAYLOAD }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );
    vi.stubGlobal("fetch", fetchMock);

    const t = new MockTransport({ sceneId: "gpu-launch-week" });
    const received: SimMessage[] = [];
    t.connect((m) => received.push(m));

    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(500);
    t.disconnect();

    const milestoneIds = new Set(GPU_LAUNCH_WEEK_STATIC_PAYLOAD.sl1_milestones?.map((milestone) => milestone.id));
    const milestoneEvent = received
      .filter((m): m is Extract<SimMessage, { kind: "events" }> => m.kind === "events")
      .flatMap((m) => m.payload)
      .find((event) => event.kind === "sl1_milestone_fired");
    expect(milestoneEvent).toBeDefined();
    expect(milestoneIds.has(milestoneEvent?.kind === "sl1_milestone_fired" ? milestoneEvent.milestone_id : "")).toBe(
      true
    );
  });

  it("external SL1 scene runtime phase stays consistent with objective state when sl1Mode is also true", async () => {
    vi.useFakeTimers();
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ schema_version: SCHEMA_VERSION, payload: GPU_LAUNCH_WEEK_STATIC_PAYLOAD }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      })
    );
    vi.stubGlobal("fetch", fetchMock);

    const t = new MockTransport({ sceneId: "gpu-launch-week", sl1Mode: true });
    const received: SimMessage[] = [];
    t.connect((m) => received.push(m));

    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(3_600);
    t.disconnect();

    const breachedSnapshot = received
      .filter((m): m is Extract<SimMessage, { kind: "snapshot" }> => m.kind === "snapshot")
      .find((snapshot) =>
        snapshot.payload.sl1_objective_states?.some((state) => state.status === "breached")
      );
    expect(breachedSnapshot?.payload.sl1_game_phase).toBe("losing");
  });
});

describe("payloadHasNativeSl1", () => {
  it("returns true for milestones-only payloads", () => {
    expect(
      payloadHasNativeSl1({
        ...EXTERNAL_STATIC_PAYLOAD,
        sl1_milestones: [
          {
            id: "dashboard-storm",
            label: "Dashboard storm",
            trigger_kind: "pressure_activated",
            trigger: { type: "pressure_activated", pressure: "dashboard-storm" },
          },
        ],
      })
    ).toBe(true);
  });

  it("returns true for objectives-only payloads", () => {
    expect(
      payloadHasNativeSl1({
        ...EXTERNAL_STATIC_PAYLOAD,
        sl1_objectives: [
          {
            id: "keep-dashboard-fresh",
            type: "keep_fresh",
            weight: 1,
            params: { kind: "keep_fresh", place: "gpu-platform", thing: "dashboard_result", max_stale_ticks: 240 },
          },
        ],
      })
    ).toBe(true);
  });

  it("returns false when all native SL1 metadata arrays are omitted", () => {
    expect(payloadHasNativeSl1(EXTERNAL_STATIC_PAYLOAD)).toBe(false);
  });
});

describe("sl1ModeFromLocation", () => {
  it("returns false for undefined / empty / unrelated query", () => {
    expect(sl1ModeFromLocation(undefined)).toBe(false);
    expect(sl1ModeFromLocation("")).toBe(false);
    expect(sl1ModeFromLocation("?foo=bar")).toBe(false);
  });

  it("returns true only when sl1demo=1", () => {
    expect(sl1ModeFromLocation("?sl1demo=1")).toBe(true);
    expect(sl1ModeFromLocation("?sl1demo=0")).toBe(false);
    expect(sl1ModeFromLocation("?sl1demo=true")).toBe(false);
  });
});
