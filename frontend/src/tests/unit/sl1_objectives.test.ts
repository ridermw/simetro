// frontend/src/tests/unit/sl1_objectives.test.ts
//
// Tests for the SL1 objectives HUD panel: renders objectives sorted
// by descending weight, applies runtime statuses, all text via
// textContent.

import { describe, it, expect, beforeEach } from "vitest";
import { Sl1ObjectivesPanel, describeObjective } from "../../ui/sl1_hud";
import type {
  Sl1ObjectiveRuntimeView,
  Sl1ObjectiveView,
} from "../../protocol/messages";

function obj(
  id: string,
  weight: number,
  type: string,
  params: Sl1ObjectiveView["params"]
): Sl1ObjectiveView {
  return { id, type, weight, params };
}

describe("describeObjective", () => {
  it("keep_fresh produces readable English", () => {
    const o = obj("o1", 3, "keep_fresh", {
      kind: "keep_fresh",
      place: "gpu-platform",
      thing: "dashboard_result",
      max_stale_ticks: 240,
    });
    const text = describeObjective(o);
    expect(text).toContain("Keep");
    expect(text).toContain("dashboard_result");
    expect(text).toContain("gpu-platform");
    expect(text).toContain("240");
  });

  it("complete_jobs_before_deadline mentions demand and max_missed", () => {
    const o = obj("o2", 2, "complete_jobs_before_deadline", {
      kind: "complete_jobs_before_deadline",
      demand: "exec-dashboard-refresh",
      max_missed: 1,
    });
    const text = describeObjective(o);
    expect(text).toContain("exec-dashboard-refresh");
    expect(text).toContain("1");
  });

  it("maintain_utilization mentions percent range", () => {
    const o = obj("o3", 1, "maintain_utilization", {
      kind: "maintain_utilization",
      place: "gpu-platform",
      capacity: "compute_units",
      min_percent: 10,
      max_percent: 95,
    });
    const text = describeObjective(o);
    expect(text).toContain("10");
    expect(text).toContain("95");
    expect(text).toContain("compute_units");
  });

  it("unsupported_in_this_pr surfaces the objective id and type", () => {
    const o = obj("o4", 0, "future_kind", { kind: "unsupported_in_this_pr" });
    const text = describeObjective(o);
    expect(text).toContain("o4");
    expect(text).toContain("future_kind");
  });
});

describe("Sl1ObjectivesPanel", () => {
  let parent: HTMLElement;
  beforeEach(() => {
    parent = document.createElement("div");
    document.body.appendChild(parent);
  });

  it("starts hidden when no objectives are set", () => {
    const panel = new Sl1ObjectivesPanel(parent);
    expect(panel.__testRoot().style.display).toBe("none");
  });

  it("hides when setObjectives([]) is called", () => {
    const panel = new Sl1ObjectivesPanel(parent);
    panel.setObjectives([
      obj("o1", 1, "keep_fresh", {
        kind: "keep_fresh",
        place: "p",
        thing: "t",
        max_stale_ticks: 10,
      }),
    ]);
    panel.setObjectives([]);
    expect(panel.__testRoot().style.display).toBe("none");
  });

  it("renders objectives sorted by descending weight", () => {
    const panel = new Sl1ObjectivesPanel(parent);
    panel.setObjectives([
      obj("low", 1, "keep_fresh", {
        kind: "keep_fresh",
        place: "p",
        thing: "a",
        max_stale_ticks: 5,
      }),
      obj("high", 3, "keep_fresh", {
        kind: "keep_fresh",
        place: "p",
        thing: "c",
        max_stale_ticks: 5,
      }),
      obj("mid", 2, "keep_fresh", {
        kind: "keep_fresh",
        place: "p",
        thing: "b",
        max_stale_ticks: 5,
      }),
    ]);
    const root = panel.__testRoot();
    const rows = root.querySelectorAll<HTMLDivElement>("[data-objective-id]");
    expect(rows.length).toBe(3);
    expect(rows[0]!.dataset.objectiveId).toBe("high");
    expect(rows[1]!.dataset.objectiveId).toBe("mid");
    expect(rows[2]!.dataset.objectiveId).toBe("low");
  });

  it("shows weight badges (w1, w2, w3) per row", () => {
    const panel = new Sl1ObjectivesPanel(parent);
    panel.setObjectives([
      obj("a", 2, "keep_fresh", {
        kind: "keep_fresh",
        place: "p",
        thing: "x",
        max_stale_ticks: 1,
      }),
    ]);
    expect(panel.__testRoot().textContent).toContain("w2");
  });

  it("initial status is unknown until snapshot arrives", () => {
    const panel = new Sl1ObjectivesPanel(parent);
    panel.setObjectives([
      obj("a", 1, "keep_fresh", {
        kind: "keep_fresh",
        place: "p",
        thing: "x",
        max_stale_ticks: 1,
      }),
    ]);
    expect(panel.__testRoot().textContent).toContain("—");
  });

  it("updateStates applies runtime status labels and colors", () => {
    const panel = new Sl1ObjectivesPanel(parent);
    panel.setObjectives([
      obj("a", 1, "keep_fresh", {
        kind: "keep_fresh",
        place: "p",
        thing: "x",
        max_stale_ticks: 1,
      }),
      obj("b", 1, "keep_fresh", {
        kind: "keep_fresh",
        place: "p",
        thing: "y",
        max_stale_ticks: 1,
      }),
    ]);
    const states: Sl1ObjectiveRuntimeView[] = [
      { objective_id: "a", status: "met", breach_tick_count: 0 },
      { objective_id: "b", status: "breached", breach_tick_count: 12 },
    ];
    panel.updateStates(states);
    const text = panel.__testRoot().textContent ?? "";
    expect(text).toContain("met");
    expect(text).toContain("breached");
  });

  it("updateStates ignores unknown objective ids without throwing", () => {
    const panel = new Sl1ObjectivesPanel(parent);
    panel.setObjectives([
      obj("a", 1, "keep_fresh", {
        kind: "keep_fresh",
        place: "p",
        thing: "x",
        max_stale_ticks: 1,
      }),
    ]);
    expect(() =>
      panel.updateStates([
        { objective_id: "nope", status: "met", breach_tick_count: 0 },
      ])
    ).not.toThrow();
    expect(panel.__testRoot().textContent).toContain("—");
  });

  it("reset() clears all rows and hides the panel", () => {
    const panel = new Sl1ObjectivesPanel(parent);
    panel.setObjectives([
      obj("a", 1, "keep_fresh", {
        kind: "keep_fresh",
        place: "p",
        thing: "x",
        max_stale_ticks: 1,
      }),
    ]);
    panel.reset();
    expect(panel.__testRoot().style.display).toBe("none");
    expect(panel.__testRoot().querySelectorAll("[data-objective-id]").length).toBe(0);
  });

  it("renders text via textContent — no script execution from hostile place names", () => {
    const panel = new Sl1ObjectivesPanel(parent);
    panel.setObjectives([
      obj("a", 1, "keep_fresh", {
        kind: "keep_fresh",
        place: "<script>window.__hostile=true</script>",
        thing: "<img src=x onerror=window.__hostile2=true>",
        max_stale_ticks: 1,
      }),
    ]);
    const root = panel.__testRoot();
    expect(root.querySelector("script")).toBeNull();
    expect(root.querySelector("img")).toBeNull();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((window as any).__hostile).toBeUndefined();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((window as any).__hostile2).toBeUndefined();
  });

  it("has region role and aria-label for screen readers", () => {
    const panel = new Sl1ObjectivesPanel(parent);
    const root = panel.__testRoot();
    expect(root.getAttribute("role")).toBe("region");
    expect(root.getAttribute("aria-label")).toBe("Scenario objectives");
  });

  it("setObjectives is idempotent on re-render", () => {
    const panel = new Sl1ObjectivesPanel(parent);
    const o1 = obj("a", 1, "keep_fresh", {
      kind: "keep_fresh",
      place: "p",
      thing: "x",
      max_stale_ticks: 1,
    });
    panel.setObjectives([o1]);
    panel.setObjectives([o1]);
    expect(panel.__testRoot().querySelectorAll("[data-objective-id]").length).toBe(1);
  });
});
