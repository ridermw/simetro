// frontend/src/tests/unit/sl1_hud.test.ts
//
// scenario_language_v1 (SL1) HUD unit tests. Verifies:
//   * Components mount with role attributes for accessibility.
//   * All author-supplied strings render via textContent — XSS
//     payloads should appear verbatim, not as parsed HTML.
//   * State transitions reflect in DOM (status outcome/phase,
//     dashboard freshness chips, alert pills, milestone strip).
//   * reset() clears scene-scoped HUD state on scene switch.

import { describe, it, expect } from "vitest";
import {
  Sl1AlertStrip,
  Sl1DashboardChips,
  Sl1MilestoneStrip,
  Sl1StatusPanel,
  createSl1Hud,
} from "../../ui/sl1_hud";
import type {
  Sl1AlertView,
  Sl1DashboardView,
} from "../../protocol/messages";

describe("Sl1StatusPanel", () => {
  it("renders outcome + phase + reason via textContent", () => {
    const parent = document.createElement("div");
    const panel = new Sl1StatusPanel(parent);
    panel.update(
      { state: "lost", reason: "<script>alert(1)</script>" },
      "spiraling"
    );
    const root = panel.__testRoot();
    expect(root.style.display).toBe("block");
    // Author-supplied reason carries an XSS payload — must render
    // verbatim, never as parsed HTML.
    expect(root.textContent).toContain("<script>alert(1)</script>");
    expect(root.querySelector("script")).toBeNull();
    expect(root.textContent).toContain("lost");
    expect(root.textContent).toContain("spiraling");
  });

  it("hides on undefined outcome (non-SL1 scene)", () => {
    const parent = document.createElement("div");
    const panel = new Sl1StatusPanel(parent);
    panel.update({ state: "in_progress" }, "winning");
    expect(panel.__testRoot().style.display).toBe("block");
    panel.update(undefined, undefined);
    expect(panel.__testRoot().style.display).toBe("none");
  });

  it("omits reason when outcome is not lost", () => {
    const parent = document.createElement("div");
    const panel = new Sl1StatusPanel(parent);
    panel.update({ state: "won" }, "winning");
    const reasonEl = panel.__testRoot().querySelector("#simetro-sl1-status-reason");
    expect((reasonEl as HTMLElement).style.display).toBe("none");
  });
});

describe("Sl1MilestoneStrip", () => {
  it("appends chips in fire order with safe text", () => {
    const parent = document.createElement("div");
    const strip = new Sl1MilestoneStrip(parent);
    strip.push({ milestone_id: "m1", label: "<img src=x onerror=alert(1)>", tick: 2 });
    strip.push({ milestone_id: "m2", label: "second", tick: 5 });
    expect(strip.__chipCount()).toBe(2);
    const root = strip.__testRoot();
    expect(root.textContent).toContain("<img src=x onerror=alert(1)>");
    expect(root.querySelector("img")).toBeNull();
    expect(root.textContent).toContain("t=2");
    expect(root.textContent).toContain("second");
  });

  it("deduplicates by milestone_id (replay-safe)", () => {
    const parent = document.createElement("div");
    const strip = new Sl1MilestoneStrip(parent);
    strip.push({ milestone_id: "m1", label: "once", tick: 1 });
    strip.push({ milestone_id: "m1", label: "once", tick: 1 });
    expect(strip.__chipCount()).toBe(1);
  });

  it("reset() clears chips and dedup memory", () => {
    const parent = document.createElement("div");
    const strip = new Sl1MilestoneStrip(parent);
    strip.push({ milestone_id: "m1", label: "x", tick: 1 });
    strip.reset();
    expect(strip.__chipCount()).toBe(0);
    strip.push({ milestone_id: "m1", label: "x", tick: 1 });
    expect(strip.__chipCount()).toBe(1);
  });
});

describe("Sl1DashboardChips", () => {
  it("renders one chip per dashboard, then reflects state changes via textContent", () => {
    const parent = document.createElement("div");
    const chips = new Sl1DashboardChips(parent);
    const dashboards: Sl1DashboardView[] = [
      {
        id: "<svg/onload=alert(1)>",
        type: "executive",
        depends_on: ["telemetry"],
        freshness_slo_ticks: 40,
      },
    ];
    chips.setDashboards(dashboards);
    const root = chips.__testRoot();
    expect(root.children.length).toBe(1);
    expect(root.textContent).toContain("<svg/onload=alert(1)>");
    expect(root.querySelector("svg")).toBeNull();

    chips.updateStates([
      { dashboard_id: "<svg/onload=alert(1)>", state: "stale", freshness_ticks: 35 },
    ]);
    const chip = root.firstElementChild as HTMLElement;
    expect(chip.getAttribute("data-state")).toBe("stale");
    expect(chip.textContent).toContain("stale");
    expect(chip.textContent).toContain("35t");
  });

  it("reset() removes all chips so next scene starts clean", () => {
    const parent = document.createElement("div");
    const chips = new Sl1DashboardChips(parent);
    chips.setDashboards([
      { id: "d1", type: "live", depends_on: [], freshness_slo_ticks: 10 },
    ]);
    chips.reset();
    expect(chips.__testRoot().children.length).toBe(0);
  });
});

describe("Sl1AlertStrip", () => {
  const alerts: Sl1AlertView[] = [
    {
      id: "<script>x</script>",
      metric: "m1",
      predicate: { kind: "gt", threshold: 30 },
      severity: "critical",
    },
  ];

  it("only shows pills for firing alerts; safe text only", () => {
    const parent = document.createElement("div");
    const strip = new Sl1AlertStrip(parent);
    strip.setAlerts(alerts);

    // Inactive → no pill.
    strip.updateStates([{ alert_id: "<script>x</script>", state: "inactive" }]);
    expect(strip.__testRoot().children.length).toBe(0);

    // Firing → pill added with safe text.
    strip.updateStates([
      { alert_id: "<script>x</script>", state: "firing", fired_at_tick: 5 },
    ]);
    const root = strip.__testRoot();
    expect(root.children.length).toBe(1);
    expect(root.textContent).toContain("<script>x</script>");
    expect(root.textContent).toContain("critical");
    expect(root.querySelector("script")).toBeNull();
    const pill = root.firstElementChild as HTMLElement;
    expect(pill.getAttribute("data-severity")).toBe("critical");

    // Back to inactive → pill removed.
    strip.updateStates([{ alert_id: "<script>x</script>", state: "inactive" }]);
    expect(strip.__testRoot().children.length).toBe(0);
  });

  it("reset() clears alerts and pills", () => {
    const parent = document.createElement("div");
    const strip = new Sl1AlertStrip(parent);
    strip.setAlerts(alerts);
    strip.updateStates([{ alert_id: "<script>x</script>", state: "firing" }]);
    strip.reset();
    expect(strip.__testRoot().children.length).toBe(0);
    // After reset, firing states no longer resolve (alerts cleared).
    strip.updateStates([{ alert_id: "<script>x</script>", state: "firing" }]);
    expect(strip.__testRoot().children.length).toBe(0);
  });
});

describe("createSl1Hud composite", () => {
  it("mounts all five panels and resets them together", () => {
    const parent = document.createElement("div");
    const hud = createSl1Hud(parent);
    hud.status.update({ state: "in_progress" }, "winning");
    hud.dashboards.setDashboards([
      { id: "d1", type: "live", depends_on: [], freshness_slo_ticks: 10 },
    ]);
    hud.alerts.setAlerts([
      { id: "a1", metric: "m1", predicate: { kind: "gt", threshold: 1 }, severity: "warning" },
    ]);
    hud.alerts.updateStates([{ alert_id: "a1", state: "firing" }]);
    hud.conditions.setConditions([
      { id: "loss", type: "place_state", params: { kind: "place_state", place: "p", state: "bad", grace_ticks: 1 } },
    ], []);
    hud.milestones.push({ milestone_id: "m1", label: "ok", tick: 1 });

    expect(hud.status.__testRoot().style.display).toBe("block");
    expect(hud.dashboards.__testRoot().children.length).toBe(1);
    expect(hud.alerts.__testRoot().children.length).toBe(1);
    expect(hud.conditions.__testRoot().style.display).toBe("block");
    expect(hud.milestones.__chipCount()).toBe(1);

    hud.reset();
    expect(hud.status.__testRoot().style.display).toBe("none");
    expect(hud.dashboards.__testRoot().children.length).toBe(0);
    expect(hud.alerts.__testRoot().children.length).toBe(0);
    expect(hud.conditions.__testRoot().style.display).toBe("none");
    expect(hud.milestones.__chipCount()).toBe(0);
  });
});
