import { describe, expect, it } from "vitest";
import { computeSl1MockRuntime, safeTick, type Sl1SceneMeta } from "../../transport/mock";

const SCENE: Sl1SceneMeta = {
  metrics: [
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
  objectives: [
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
  failures: [
    {
      id: "refresh-objective-breached",
      type: "objective_breach_count",
      params: { kind: "objective_breach_count", objective_id: "no-dropped-refreshes", max_count: 3 },
    },
  ],
  victories: [
    {
      id: "survive-launch-week",
      type: "survive_until",
      params: { kind: "survive_until", at_tick: 2800 },
    },
  ],
};

function stateForMetric(tick: number, metricId: string) {
  const state = computeSl1MockRuntime(tick, SCENE).metric_states.find((m) => m.metric_id === metricId);
  expect(state).toBeDefined();
  return state!;
}

describe("computeSl1MockRuntime", () => {
  it("returns empty runtime arrays for an empty SL1 scene", () => {
    const runtime = computeSl1MockRuntime(0, { metrics: [], objectives: [], failures: [], victories: [] });
    expect(runtime).toEqual({
      metric_states: [],
      objective_states: [],
      failure_condition_states: [],
      victory_condition_states: [],
      phase: "winning",
    });
  });

  it("place capacity metrics are ok and stay in the 0-100 range at key ticks", () => {
    for (const tick of [0, 50, 100]) {
      const state = stateForMetric(tick, "platform-compute-load");
      expect(state.state).toBe("ok");
      expect(state.value).toBeGreaterThanOrEqual(0);
      expect(state.value).toBeLessThanOrEqual(100);
    }
  });

  it("place inventory metrics are ok finite counts at key ticks", () => {
    for (const tick of [0, 50, 100]) {
      const state = stateForMetric(tick, "heartbeat-backlog");
      expect(state.state).toBe("ok");
      expect(Number.isFinite(state.value)).toBe(true);
      expect(state.value).toBeGreaterThanOrEqual(0);
    }
  });

  it("dashboard freshness metrics are ok finite ticks at key ticks", () => {
    for (const tick of [0, 50, 100]) {
      const state = stateForMetric(tick, "exec-dashboard-freshness");
      expect(state.state).toBe("ok");
      expect(Number.isFinite(state.value)).toBe(true);
      expect(state.value).toBeGreaterThanOrEqual(0);
    }
  });

  it("is deterministic for the same scene and tick", () => {
    expect(computeSl1MockRuntime(77, SCENE)).toEqual(computeSl1MockRuntime(77, SCENE));
  });

  it("flips the objective targeted by an objective_breach_count failure during the breach window", () => {
    const objective = computeSl1MockRuntime(75, SCENE).objective_states.find(
      (state) => state.objective_id === "no-dropped-refreshes"
    );
    expect(objective).toEqual({
      objective_id: "no-dropped-refreshes",
      status: "breached",
      breach_tick_count: 16,
    });
  });

  it("returns the breached objective to met after the breach window", () => {
    const objective = computeSl1MockRuntime(120, SCENE).objective_states.find(
      (state) => state.objective_id === "no-dropped-refreshes"
    );
    expect(objective).toEqual({
      objective_id: "no-dropped-refreshes",
      status: "met",
      breach_tick_count: 0,
    });
  });

  it("keeps non-target objectives met during the breach window", () => {
    const objective = computeSl1MockRuntime(75, SCENE).objective_states.find(
      (state) => state.objective_id === "keep-dashboard-fresh"
    );
    expect(objective).toEqual({
      objective_id: "keep-dashboard-fresh",
      status: "met",
      breach_tick_count: 0,
    });
  });

  it("grows failure condition breach streak during the breached objective window", () => {
    expect(computeSl1MockRuntime(60, SCENE).failure_condition_states[0]).toEqual({
      failure_condition_id: "refresh-objective-breached",
      breach_streak_ticks: 1,
    });
    expect(computeSl1MockRuntime(75, SCENE).failure_condition_states[0]).toEqual({
      failure_condition_id: "refresh-objective-breached",
      breach_streak_ticks: 16,
    });
  });

  it("sets failure fired_at_tick during the breach window and clears it after reset", () => {
    expect(computeSl1MockRuntime(75, SCENE).failure_condition_states[0]).toEqual({
      failure_condition_id: "refresh-objective-breached",
      breach_streak_ticks: 16,
    });
    expect(computeSl1MockRuntime(80, SCENE).failure_condition_states[0]).toEqual({
      failure_condition_id: "refresh-objective-breached",
      breach_streak_ticks: 21,
      fired_at_tick: 80,
    });
    expect(computeSl1MockRuntime(120, SCENE).failure_condition_states[0]).toEqual({
      failure_condition_id: "refresh-objective-breached",
      breach_streak_ticks: 0,
    });
  });

  it("returns victory condition states with met_at_tick only when survive_until is reached", () => {
    expect(computeSl1MockRuntime(100, SCENE).victory_condition_states).toEqual([
      { victory_condition_id: "survive-launch-week" },
    ]);
    expect(computeSl1MockRuntime(2800, SCENE).victory_condition_states).toEqual([
      { victory_condition_id: "survive-launch-week", met_at_tick: 2800 },
    ]);
  });

  it("sets phase to losing only during the breach window", () => {
    expect(computeSl1MockRuntime(0, SCENE).phase).toBe("winning");
    expect(computeSl1MockRuntime(75, SCENE).phase).toBe("losing");
    expect(computeSl1MockRuntime(120, SCENE).phase).toBe("winning");
  });

  it("safeTick normalizes non-finite and huge ticks", () => {
    expect(safeTick(Number.NaN)).toBe(0);
    expect(safeTick(Number.POSITIVE_INFINITY)).toBe(0);
    const hugeTick = safeTick(Number.MAX_SAFE_INTEGER);
    expect(Number.isFinite(hugeTick)).toBe(true);
    expect(hugeTick).toBeGreaterThanOrEqual(0);
    expect(hugeTick).toBeLessThan(100000);
  });

  it("all metric values are finite", () => {
    for (const tick of [0, 50, 100, 150, Number.NaN, Number.POSITIVE_INFINITY, Number.MAX_SAFE_INTEGER]) {
      for (const state of computeSl1MockRuntime(tick, SCENE).metric_states) {
        expect(Number.isFinite(state.value)).toBe(true);
      }
    }
  });
});
