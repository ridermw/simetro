// frontend/src/tests/unit/sl1_conditions.test.ts
//
// Tests for the SL1 win/loss conditions HUD panel: renders declared
// failure/victory conditions with safe text and applies runtime states.

import { describe, it, expect, beforeEach } from "vitest";
import {
  Sl1ConditionsPanel,
  describeFailureCondition,
  describeVictoryCondition,
} from "../../ui/sl1_hud";
import type {
  Sl1FailureConditionRuntimeView,
  Sl1FailureConditionView,
  Sl1VictoryConditionRuntimeView,
  Sl1VictoryConditionView,
} from "../../protocol/messages";

function failure(
  id: string,
  type: string,
  params: Sl1FailureConditionView["params"]
): Sl1FailureConditionView {
  return { id, type, params };
}

function victory(
  id: string,
  type: string,
  params: Sl1VictoryConditionView["params"]
): Sl1VictoryConditionView {
  return { id, type, params };
}

describe("describeFailureCondition", () => {
  it("stale_target produces readable English", () => {
    const text = describeFailureCondition(
      failure("loss-stale", "stale_target", {
        kind: "stale_target",
        place: "gpu-platform",
        thing: "exec_dashboard",
        threshold_ticks: 120,
        grace_ticks: 30,
      })
    );
    expect(text).toBe("exec_dashboard in gpu-platform stale > 120 ticks (grace 30)");
  });

  it("place_state produces readable English", () => {
    const text = describeFailureCondition(
      failure("loss-state", "place_state", {
        kind: "place_state",
        place: "storage",
        state: "saturated",
        grace_ticks: 10,
      })
    );
    expect(text).toBe("storage in state saturated (grace 10)");
  });

  it("objective_breach_count produces readable English", () => {
    const text = describeFailureCondition(
      failure("loss-breach", "objective_breach_count", {
        kind: "objective_breach_count",
        objective_id: "keep-dashboards-fresh",
        max_count: 2,
      })
    );
    expect(text).toBe("Objective keep-dashboards-fresh breached > 2 times");
  });
});

describe("describeVictoryCondition", () => {
  it("survive_until produces readable English", () => {
    const text = describeVictoryCondition(
      victory("win-survive", "survive_until", { kind: "survive_until", at_tick: 600 })
    );
    expect(text).toBe("Survive until tick 600");
  });
});

describe("Sl1ConditionsPanel", () => {
  let parent: HTMLElement;

  beforeEach(() => {
    parent = document.createElement("div");
    document.body.appendChild(parent);
  });

  it("starts hidden when no conditions are set", () => {
    const panel = new Sl1ConditionsPanel(parent);
    expect(panel.__testRoot().style.display).toBe("none");
  });

  it("hides when setConditions([], []) is called", () => {
    const panel = new Sl1ConditionsPanel(parent);
    panel.setConditions([
      failure("loss", "place_state", {
        kind: "place_state",
        place: "p",
        state: "bad",
        grace_ticks: 1,
      }),
    ], []);
    panel.setConditions([], []);
    expect(panel.__testRoot().style.display).toBe("none");
  });

  it("renders failure rows with LOSS badges", () => {
    const panel = new Sl1ConditionsPanel(parent);
    panel.setConditions([
      failure("loss", "place_state", {
        kind: "place_state",
        place: "p",
        state: "bad",
        grace_ticks: 1,
      }),
    ], []);
    const row = panel.__testRoot().querySelector<HTMLElement>("[data-failure-condition-id]");
    expect(row).not.toBeNull();
    expect(row?.textContent).toContain("LOSS");
  });

  it("renders victory rows with WIN badges", () => {
    const panel = new Sl1ConditionsPanel(parent);
    panel.setConditions([], [victory("win", "survive_until", { kind: "survive_until", at_tick: 10 })]);
    const row = panel.__testRoot().querySelector<HTMLElement>("[data-victory-condition-id]");
    expect(row).not.toBeNull();
    expect(row?.textContent).toContain("WIN");
  });

  it("renders failures before victory conditions in DOM order", () => {
    const panel = new Sl1ConditionsPanel(parent);
    panel.setConditions([
      failure("loss", "place_state", {
        kind: "place_state",
        place: "p",
        state: "bad",
        grace_ticks: 1,
      }),
    ], [victory("win", "survive_until", { kind: "survive_until", at_tick: 10 })]);
    const rows = panel.__testRoot().querySelectorAll<HTMLElement>("[data-condition-kind]");
    expect(rows[0]?.dataset.conditionKind).toBe("failure");
    expect(rows[1]?.dataset.conditionKind).toBe("victory");
  });

  it("initial status is armed for failures and pending for victory", () => {
    const panel = new Sl1ConditionsPanel(parent);
    panel.setConditions([
      failure("loss", "place_state", {
        kind: "place_state",
        place: "p",
        state: "bad",
        grace_ticks: 1,
      }),
    ], [victory("win", "survive_until", { kind: "survive_until", at_tick: 10 })]);
    const text = panel.__testRoot().textContent ?? "";
    expect(text).toContain("armed");
    expect(text).toContain("pending");
  });

  it("updateFailureStates with fired_at_tick shows FIRED at the tick", () => {
    const panel = new Sl1ConditionsPanel(parent);
    panel.setConditions([
      failure("loss", "place_state", {
        kind: "place_state",
        place: "p",
        state: "bad",
        grace_ticks: 1,
      }),
    ], []);
    const states: Sl1FailureConditionRuntimeView[] = [
      { failure_condition_id: "loss", breach_streak_ticks: 4, fired_at_tick: 42 },
    ];
    panel.updateFailureStates(states);
    const status = panel.__testRoot().querySelector<HTMLElement>("[data-condition-status='failure']");
    expect(status?.textContent).toBe("FIRED @ tick 42");
    expect(status?.style.color).toBe("rgb(247, 118, 142)");
  });

  it("updateFailureStates with breach streak before firing shows streak", () => {
    const panel = new Sl1ConditionsPanel(parent);
    panel.setConditions([
      failure("loss", "place_state", {
        kind: "place_state",
        place: "p",
        state: "bad",
        grace_ticks: 1,
      }),
    ], []);
    panel.updateFailureStates([{ failure_condition_id: "loss", breach_streak_ticks: 3 }]);
    expect(panel.__testRoot().textContent).toContain("streak: 3 ticks");
  });

  it("updateVictoryStates with met_at_tick shows ACHIEVED at the tick", () => {
    const panel = new Sl1ConditionsPanel(parent);
    panel.setConditions([], [victory("win", "survive_until", { kind: "survive_until", at_tick: 10 })]);
    const states: Sl1VictoryConditionRuntimeView[] = [{ victory_condition_id: "win", met_at_tick: 99 }];
    panel.updateVictoryStates(states);
    const status = panel.__testRoot().querySelector<HTMLElement>("[data-condition-status='victory']");
    expect(status?.textContent).toBe("ACHIEVED @ tick 99");
    expect(status?.style.color).toBe("rgb(158, 206, 106)");
  });

  it("unknown condition ids in update states are ignored without throwing", () => {
    const panel = new Sl1ConditionsPanel(parent);
    panel.setConditions([
      failure("loss", "place_state", {
        kind: "place_state",
        place: "p",
        state: "bad",
        grace_ticks: 1,
      }),
    ], [victory("win", "survive_until", { kind: "survive_until", at_tick: 10 })]);
    expect(() => {
      panel.updateFailureStates([{ failure_condition_id: "missing", breach_streak_ticks: 1 }]);
      panel.updateVictoryStates([{ victory_condition_id: "missing", met_at_tick: 12 }]);
    }).not.toThrow();
    const text = panel.__testRoot().textContent ?? "";
    expect(text).toContain("armed");
    expect(text).toContain("pending");
  });

  it("reset clears rows and hides", () => {
    const panel = new Sl1ConditionsPanel(parent);
    panel.setConditions([
      failure("loss", "place_state", {
        kind: "place_state",
        place: "p",
        state: "bad",
        grace_ticks: 1,
      }),
    ], [victory("win", "survive_until", { kind: "survive_until", at_tick: 10 })]);
    panel.reset();
    expect(panel.__testRoot().style.display).toBe("none");
    expect(panel.__testRoot().querySelectorAll("[data-condition-kind]").length).toBe(0);
  });

  it("renders hostile strings safely", () => {
    const panel = new Sl1ConditionsPanel(parent);
    panel.setConditions([
      failure("loss", "stale_target", {
        kind: "stale_target",
        place: "<script>window.__sl1Place=true</script>",
        thing: "<img src=x onerror=window.__sl1Thing=true>",
        threshold_ticks: 2,
        grace_ticks: 1,
      }),
    ], []);
    const root = panel.__testRoot();
    expect(root.textContent).toContain("<script>window.__sl1Place=true</script>");
    expect(root.textContent).toContain("<img src=x onerror=window.__sl1Thing=true>");
    expect(root.querySelector("script")).toBeNull();
    expect(root.querySelector("img")).toBeNull();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((window as any).__sl1Place).toBeUndefined();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect((window as any).__sl1Thing).toBeUndefined();
  });

  it("has region role and aria-label for screen readers", () => {
    const panel = new Sl1ConditionsPanel(parent);
    const root = panel.__testRoot();
    expect(root.getAttribute("role")).toBe("region");
    expect(root.getAttribute("aria-label")).toBe("Win and loss conditions");
  });

  it("setConditions is idempotent on re-render", () => {
    const panel = new Sl1ConditionsPanel(parent);
    const loss = failure("loss", "place_state", {
      kind: "place_state",
      place: "p",
      state: "bad",
      grace_ticks: 1,
    });
    const win = victory("win", "survive_until", { kind: "survive_until", at_tick: 10 });
    panel.setConditions([loss], [win]);
    panel.setConditions([loss], [win]);
    expect(panel.__testRoot().querySelectorAll("[data-condition-kind]").length).toBe(2);
  });

  it("panel layout uses right-anchor + viewport-clamped max-width", () => {
    const panel = new Sl1ConditionsPanel(parent);
    const css = panel.__testRoot().style.cssText;
    // Right-anchor so the panel never overflows past the right edge.
    expect(css).toContain("right: 12px");
    // Should NOT use a fixed left position that can collide on narrow viewports.
    expect(css).not.toContain("left: 640px");
  });

  it("long author ids do not overflow horizontally", () => {
    const panel = new Sl1ConditionsPanel(parent);
    const longId = "a".repeat(200);
    const loss = failure(longId, "stale_target", {
      kind: "stale_target",
      place: longId,
      thing: longId,
      threshold_ticks: 999999999,
      grace_ticks: 999999999,
    });
    panel.setConditions([loss], []);
    const root = panel.__testRoot();
    // The full id appears via textContent — wrapping is CSS-driven so we
    // just verify the panel renders without throwing and keeps the text.
    expect(root.textContent).toContain(longId);
    expect(root.style.cssText).toContain("overflow-wrap");
  });
});
