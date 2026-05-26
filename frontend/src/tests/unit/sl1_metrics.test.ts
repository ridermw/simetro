// frontend/src/tests/unit/sl1_metrics.test.ts
//
// Tests for the SL1 observability metrics HUD panel: renders declared
// metrics, applies live metric states, and keeps author strings safe.

import { describe, it, expect, beforeEach } from "vitest";
import { Sl1MetricsPanel, describeMetricSource, formatMetricValue } from "../../ui/sl1_hud";
import type {
  Sl1MetricSourceView,
  Sl1MetricStateView,
  Sl1MetricView,
} from "../../protocol/messages";

function metric(id: string, source: Sl1MetricSourceView): Sl1MetricView {
  return { id, source };
}

describe("describeMetricSource", () => {
  it("describes place capacity percent metrics", () => {
    expect(
      describeMetricSource({
        kind: "place_capacity_used_percent",
        place: "gpu-cluster",
        capacity: "compute_units",
      })
    ).toBe("gpu-cluster.compute_units capacity %");
  });

  it("describes place inventory count metrics", () => {
    expect(
      describeMetricSource({
        kind: "place_inventory_count",
        place: "telemetry-buffer",
        thing: "heartbeat",
      })
    ).toBe("telemetry-buffer: count of heartbeat");
  });

  it("describes dashboard freshness metrics", () => {
    expect(
      describeMetricSource({
        kind: "dashboard_freshness",
        dashboard: "exec-dashboard",
      })
    ).toBe("exec-dashboard freshness (ticks)");
  });
});

describe("Sl1MetricsPanel", () => {
  let parent: HTMLElement;

  beforeEach(() => {
    parent = document.createElement("div");
    document.body.appendChild(parent);
  });

  it("starts hidden with region metadata and hides when setMetrics([]) is called", () => {
    const panel = new Sl1MetricsPanel(parent);
    expect(panel.__testRoot().style.display).toBe("none");
    expect(panel.__testRoot().getAttribute("role")).toBe("region");
    expect(panel.__testRoot().getAttribute("aria-label")).toBe("Observability metrics");
    panel.setMetrics([
      metric("compute-utilization", {
        kind: "place_capacity_used_percent",
        place: "gpu-cluster",
        capacity: "compute_units",
      }),
    ]);
    panel.setMetrics([]);
    expect(panel.__testRoot().style.display).toBe("none");
  });

  it("renders rows with metric id and source description", () => {
    const panel = new Sl1MetricsPanel(parent);
    panel.setMetrics([
      metric("dashboard-freshness", {
        kind: "dashboard_freshness",
        dashboard: "exec-dashboard",
      }),
    ]);
    const row = panel.__testRoot().querySelector<HTMLElement>("[data-metric-id]");
    expect(row).not.toBeNull();
    expect(row?.textContent).toContain("dashboard-freshness");
    expect(row?.textContent).toContain("exec-dashboard freshness (ticks)");
  });

  it("initial value shows dash in gray no_data state", () => {
    const panel = new Sl1MetricsPanel(parent);
    panel.setMetrics([
      metric("heartbeat-backlog", {
        kind: "place_inventory_count",
        place: "telemetry-buffer",
        thing: "heartbeat",
      }),
    ]);
    const value = panel.__testRoot().querySelector<HTMLElement>("[data-metric-value]");
    const state = panel.__testRoot().querySelector<HTMLElement>("[data-metric-state]");
    expect(value?.textContent).toBe("—");
    expect(value?.style.color).toBe("rgb(139, 148, 158)");
    expect(state?.textContent).toBe("no data");
    expect(state?.style.color).toBe("rgb(139, 148, 158)");
  });

  it("updateStates with ok and value shows formatted numeric value", () => {
    const panel = new Sl1MetricsPanel(parent);
    panel.setMetrics([
      metric("heartbeat-backlog", {
        kind: "place_inventory_count",
        place: "telemetry-buffer",
        thing: "heartbeat",
      }),
    ]);
    const states: Sl1MetricStateView[] = [
      { metric_id: "heartbeat-backlog", state: "ok", value: 12.25 },
    ];
    panel.updateStates(states);
    const value = panel.__testRoot().querySelector<HTMLElement>("[data-metric-value]");
    const state = panel.__testRoot().querySelector<HTMLElement>("[data-metric-state]");
    expect(value?.textContent).toBe("12.3");
    expect(state?.textContent).toBe("ok");
    expect(state?.style.color).toBe("rgb(158, 206, 106)");
  });

  it("formats place_capacity_used_percent values with percent suffix", () => {
    const panel = new Sl1MetricsPanel(parent);
    panel.setMetrics([
      metric("compute-utilization", {
        kind: "place_capacity_used_percent",
        place: "gpu-cluster",
        capacity: "compute_units",
      }),
    ]);
    panel.updateStates([{ metric_id: "compute-utilization", state: "ok", value: 83 }]);
    expect(panel.__testRoot().querySelector<HTMLElement>("[data-metric-value]")?.textContent).toBe(
      "83%"
    );
  });

  it("no_data state shows dash even when a value is present", () => {
    const panel = new Sl1MetricsPanel(parent);
    panel.setMetrics([
      metric("dashboard-freshness", {
        kind: "dashboard_freshness",
        dashboard: "exec-dashboard",
      }),
    ]);
    panel.updateStates([{ metric_id: "dashboard-freshness", state: "no_data", value: 10 }]);
    expect(panel.__testRoot().querySelector<HTMLElement>("[data-metric-value]")?.textContent).toBe(
      "—"
    );
  });

  it("unknown metric ids in updateStates are ignored", () => {
    const panel = new Sl1MetricsPanel(parent);
    panel.setMetrics([
      metric("known", {
        kind: "dashboard_freshness",
        dashboard: "exec-dashboard",
      }),
    ]);
    expect(() => panel.updateStates([{ metric_id: "missing", state: "ok", value: 1 }])).not.toThrow();
    expect(panel.__testRoot().querySelector<HTMLElement>("[data-metric-value]")?.textContent).toBe(
      "—"
    );
  });

  it("state ok with missing value shows dash gracefully", () => {
    const panel = new Sl1MetricsPanel(parent);
    panel.setMetrics([
      metric("known", {
        kind: "dashboard_freshness",
        dashboard: "exec-dashboard",
      }),
    ]);
    panel.updateStates([{ metric_id: "known", state: "ok" }]);
    expect(panel.__testRoot().querySelector<HTMLElement>("[data-metric-value]")?.textContent).toBe(
      "—"
    );
    expect(panel.__testRoot().querySelector<HTMLElement>("[data-metric-state]")?.textContent).toBe(
      "ok"
    );
  });

  it("reset clears rows and hides", () => {
    const panel = new Sl1MetricsPanel(parent);
    panel.setMetrics([
      metric("m", {
        kind: "dashboard_freshness",
        dashboard: "exec-dashboard",
      }),
    ]);
    panel.reset();
    expect(panel.__testRoot().style.display).toBe("none");
    expect(panel.__testRoot().querySelectorAll("[data-metric-id]").length).toBe(0);
  });

  it("renders hostile place thing dashboard ids safely via textContent", () => {
    const panel = new Sl1MetricsPanel(parent);
    panel.setMetrics([
      metric("<script>window.__metricId=true</script>", {
        kind: "place_inventory_count",
        place: "<script>window.__metricPlace=true</script>",
        thing: "<img src=x onerror=window.__metricThing=true>",
      }),
      metric("fresh", {
        kind: "dashboard_freshness",
        dashboard: "<img src=x onerror=window.__metricDashboard=true>",
      }),
    ]);
    const root = panel.__testRoot();
    expect(root.textContent).toContain("<script>window.__metricPlace=true</script>");
    expect(root.textContent).toContain("<img src=x onerror=window.__metricThing=true>");
    expect(root.textContent).toContain("<img src=x onerror=window.__metricDashboard=true>");
    expect(root.querySelector("script")).toBeNull();
    expect(root.querySelector("img")).toBeNull();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((window as any).__metricPlace).toBeUndefined();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((window as any).__metricThing).toBeUndefined();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((window as any).__metricDashboard).toBeUndefined();
  });

  it("setMetrics is idempotent on re-render", () => {
    const panel = new Sl1MetricsPanel(parent);
    const m = metric("compute-utilization", {
      kind: "place_capacity_used_percent",
      place: "gpu-cluster",
      capacity: "compute_units",
    });
    panel.setMetrics([m]);
    panel.setMetrics([m]);
    expect(panel.__testRoot().querySelectorAll("[data-metric-id]").length).toBe(1);
  });

  it("panel styling is bounded and wraps to avoid horizontal overflow", () => {
    const panel = new Sl1MetricsPanel(parent);
    const css = panel.__testRoot().style.cssText;
    expect(parent.contains(panel.__testRoot())).toBe(true);
    // Metrics panel now lives inside the flex stack — its max-width is
    // stack-relative (100%) rather than viewport-relative.
    expect(css).toContain("max-width: 100%");
    expect(css).toContain("background: rgba(14, 17, 22, 0.85)");
    expect(css).toContain("overflow-wrap: anywhere");
  });
});

describe("formatMetricValue", () => {
  it("formats integer values without decimal point", () => {
    const src = { kind: "place_inventory_count" as const, place: "p", thing: "t" };
    expect(formatMetricValue(src, 42)).toBe("42");
  });

  it("formats fractional values with one decimal", () => {
    const src = { kind: "place_inventory_count" as const, place: "p", thing: "t" };
    expect(formatMetricValue(src, 3.14)).toBe("3.1");
  });

  it("appends % suffix for place_capacity_used_percent", () => {
    const src = {
      kind: "place_capacity_used_percent" as const,
      place: "p",
      capacity: "c",
    };
    expect(formatMetricValue(src, 75)).toBe("75%");
  });
});

describe("Sl1MetricsPanel numeric edge cases", () => {
  let parent: HTMLElement;
  beforeEach(() => {
    parent = document.createElement("div");
    document.body.appendChild(parent);
  });

  it("renders NaN value as no-data placeholder (not 'NaN%')", () => {
    const panel = new Sl1MetricsPanel(parent);
    const metric = {
      id: "m1",
      source: {
        kind: "place_capacity_used_percent" as const,
        place: "p",
        capacity: "c",
      },
    };
    panel.setMetrics([metric]);
    panel.updateStates([{ metric_id: "m1", state: "ok", value: Number.NaN }]);
    const text = panel.__testRoot().textContent ?? "";
    expect(text).not.toContain("NaN");
    expect(text).toContain("—");
  });

  it("renders Infinity value as no-data placeholder", () => {
    const panel = new Sl1MetricsPanel(parent);
    const metric = {
      id: "m1",
      source: { kind: "place_inventory_count" as const, place: "p", thing: "t" },
    };
    panel.setMetrics([metric]);
    panel.updateStates([{ metric_id: "m1", state: "ok", value: Number.POSITIVE_INFINITY }]);
    const text = panel.__testRoot().textContent ?? "";
    expect(text).not.toContain("Infinity");
    expect(text).toContain("—");
  });

  it("renders -Infinity as no-data placeholder", () => {
    const panel = new Sl1MetricsPanel(parent);
    const metric = {
      id: "m1",
      source: { kind: "dashboard_freshness" as const, dashboard: "d" },
    };
    panel.setMetrics([metric]);
    panel.updateStates([{ metric_id: "m1", state: "ok", value: Number.NEGATIVE_INFINITY }]);
    const text = panel.__testRoot().textContent ?? "";
    expect(text).not.toContain("Infinity");
    expect(text).toContain("—");
  });

  it("renders very large finite values without truncation", () => {
    const panel = new Sl1MetricsPanel(parent);
    const metric = {
      id: "m1",
      source: { kind: "place_inventory_count" as const, place: "p", thing: "t" },
    };
    panel.setMetrics([metric]);
    panel.updateStates([{ metric_id: "m1", state: "ok", value: 9999999999 }]);
    const text = panel.__testRoot().textContent ?? "";
    expect(text).toContain("9999999999");
  });

  it("renders negative percent values verbatim (does not suppress)", () => {
    // Negative percent is unusual but a finite number; we surface it
    // rather than hide it so an author can see their bug.
    const panel = new Sl1MetricsPanel(parent);
    const metric = {
      id: "m1",
      source: {
        kind: "place_capacity_used_percent" as const,
        place: "p",
        capacity: "c",
      },
    };
    panel.setMetrics([metric]);
    panel.updateStates([{ metric_id: "m1", state: "ok", value: -5 }]);
    expect(panel.__testRoot().textContent).toContain("-5%");
  });
});
