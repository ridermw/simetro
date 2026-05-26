// frontend/src/tests/unit/ui.test.ts
import { describe, it, expect, vi } from "vitest";
import { ControlsBar, type ControlIntent } from "../../ui/controls";
import {
  FaultOverlay,
  HeartbeatBadge,
  PerfOverlay,
  WarningStrip,
  formatFault,
  formatWarning,
} from "../../ui/overlays";

describe("ControlsBar", () => {
  it("dispatches TogglePause on play/pause click", () => {
    const handler = vi.fn();
    const parent = document.createElement("div");
    new ControlsBar(parent, handler);
    const btn = parent.querySelector<HTMLButtonElement>("#simetro-play-pause");
    expect(btn).not.toBeNull();
    btn?.click();
    expect(handler).toHaveBeenCalledWith({ kind: "TogglePause" });
  });

  it("dispatches Step / Reload / SetSpeed", () => {
    const intents: ControlIntent[] = [];
    const parent = document.createElement("div");
    new ControlsBar(parent, (i) => intents.push(i));
    parent.querySelector<HTMLButtonElement>("#simetro-step")?.click();
    parent.querySelector<HTMLButtonElement>("#simetro-reload")?.click();
    parent.querySelector<HTMLButtonElement>("#simetro-speed-2")?.click();
    expect(intents).toEqual([
      { kind: "Step" },
      { kind: "Reload" },
      { kind: "SetSpeed", factor: 2 },
    ]);
  });

  it("reflects paused state via aria-label", () => {
    const parent = document.createElement("div");
    const bar = new ControlsBar(parent, () => {});
    bar.setPaused(true);
    const btn = parent.querySelector<HTMLButtonElement>("#simetro-play-pause");
    expect(btn?.getAttribute("aria-label")).toBe("Play");
    bar.setPaused(false);
    expect(btn?.getAttribute("aria-label")).toBe("Pause");
  });

  it("show() and hide() toggle toolbar visibility", () => {
    const parent = document.createElement("div");
    const bar = new ControlsBar(parent, () => {});

    bar.hide();
    expect(bar.__testRoot().style.display).toBe("none");

    bar.show();
    expect(bar.__testRoot().style.display).toBe("flex");
  });
});

describe("FaultOverlay", () => {
  it("renders fault messages via textContent (no innerHTML)", () => {
    const parent = document.createElement("div");
    const f = new FaultOverlay(parent);
    f.show({
      kind: "load_error",
      message: "<script>evil</script>",
      line: 5,
      col: 3,
    });
    const root = f.__testRoot();
    expect(root.style.display).toBe("flex");
    expect(root.textContent).toContain("<script>evil</script>");
    expect(root.querySelector("script")).toBeNull();
    expect(root.textContent).toContain("line 5");
  });

  it("formatFault covers every fault kind", () => {
    expect(formatFault({ kind: "load_error", message: "bad", line: 1, col: 2 })).toContain(
      "SCENE LOAD"
    );
    expect(formatFault({ kind: "load_error", message: "bad", line: null, col: null })).toContain(
      "SCENE LOAD"
    );
    expect(formatFault({ kind: "agent_crashed", agent_id: "a", message: "m" })).toContain(
      "AGENT CRASHED"
    );
    expect(formatFault({ kind: "numeric_drift", tick: 5 })).toContain("NUMERIC DRIFT");
    expect(formatFault({ kind: "engine_fault", message: "kaboom" })).toContain("ENGINE FAULT");
    expect(
      formatFault({ kind: "baseline_hash_mismatch", expected: "aaa", found: "bbb" })
    ).toContain("BASELINE HASH MISMATCH");
    expect(formatFault({ kind: "schema_mismatch", expected: 1, found: 99 })).toContain(
      "SCHEMA MISMATCH"
    );
    expect(formatFault({ kind: "transport_lost" })).toContain("TRANSPORT LOST");
  });

  it("hide() restores display: none", () => {
    const parent = document.createElement("div");
    const f = new FaultOverlay(parent);
    f.show({ kind: "transport_lost" });
    f.hide();
    expect(f.__testRoot().style.display).toBe("none");
  });
});

describe("WarningStrip", () => {
  it("push adds a pill; tick() sweeps expired ones", () => {
    const parent = document.createElement("div");
    const s = new WarningStrip(parent);
    s.push({ kind: "behind", lag_frames: 3 }, 100);
    expect(s.__testCount()).toBe(1);
    s.tick(performance.now() + 1000);
    expect(s.__testCount()).toBe(0);
  });

  it("clear removes all active warning pills", () => {
    const parent = document.createElement("div");
    const s = new WarningStrip(parent);
    s.push({ kind: "behind", lag_frames: 3 });
    s.push({ kind: "agent_log_slow" });
    s.clear();
    expect(s.__testCount()).toBe(0);
    expect(parent.textContent).toBe("");
  });

  it("formatWarning covers every kind", () => {
    expect(formatWarning({ kind: "invalid_action", agent_id: "a", reason: "x" })).toContain(
      "invalid action"
    );
    expect(formatWarning({ kind: "behind", lag_frames: 4 })).toContain("behind");
    expect(formatWarning({ kind: "tick_over_budget", ms: 12.3 })).toContain("over budget");
    expect(formatWarning({ kind: "agent_log_slow" })).toContain("slow");
  });

  // ───────────── Regression: yellow-box flood on right side ─────────────
  //
  // User-reported bug (2026-05-26): in SL1 scenes (e.g. clinic-triage-desk)
  // the Rust engine emits a flood of warnings during the first second of
  // play. WarningStrip blindly appends a pill per warning, producing a
  // long vertical column of amber pills cascading down the right side of
  // the canvas. These tests pin the contract that prevents that visual
  // bug from regressing.

  const WARNING_STRIP_MAX_VISIBLE = 5;

  it("caps the number of simultaneously visible pills (regression: yellow-box flood)", () => {
    const parent = document.createElement("div");
    const s = new WarningStrip(parent);
    // Engine emits a flood of distinct warnings (e.g., 30 invalid_action
    // warnings for different agent ids in a single tick).
    for (let i = 0; i < 30; i++) {
      s.push({ kind: "invalid_action", agent_id: `agent-${i}`, reason: "x" });
    }
    expect(s.__testCount()).toBeLessThanOrEqual(WARNING_STRIP_MAX_VISIBLE);
  });

  it("coalesces repeated identical warning kinds into a single pill with a counter", () => {
    const parent = document.createElement("div");
    const s = new WarningStrip(parent);
    // Same warning fired 20 times — should NOT produce 20 pills.
    for (let i = 0; i < 20; i++) {
      s.push({ kind: "agent_log_slow" });
    }
    expect(s.__testCount()).toBe(1);
    // The single pill should indicate the count somehow (×N or count).
    const text = parent.textContent ?? "";
    expect(text).toMatch(/×\s*20|x\s*20|\(\s*20\s*\)|20×/i);
  });

  it("dropping oldest when cap is exceeded keeps newest warnings visible", () => {
    const parent = document.createElement("div");
    const s = new WarningStrip(parent);
    // Push 10 distinct warnings; only the most recent few should remain.
    for (let i = 0; i < 10; i++) {
      s.push({ kind: "invalid_action", agent_id: `agent-${i}`, reason: "r" });
    }
    const text = parent.textContent ?? "";
    // The most recently pushed warning (agent-9) must be visible.
    expect(text).toContain("agent-9");
    // The oldest warnings should have been dropped.
    expect(text).not.toContain("agent-0");
  });

  it("recovers visible state across coalesce-then-distinct interleaving", () => {
    const parent = document.createElement("div");
    const s = new WarningStrip(parent);
    // 5 of the same kind, then 1 of a different kind.
    for (let i = 0; i < 5; i++) s.push({ kind: "agent_log_slow" });
    s.push({ kind: "behind", lag_frames: 2 });
    // Should have 2 pills: one for the coalesced agent_log_slow, one for behind.
    expect(s.__testCount()).toBe(2);
    expect(parent.textContent).toContain("behind");
  });

  it("coalesce refreshes pill text from the LATEST warning payload (Codex P2 #1)", () => {
    // Repeating a payload-bearing warning with escalating values
    // must show the new values, not the original. Hiding escalation
    // (e.g. lag 3 → lag 50) defeats the warning's purpose.
    const parent = document.createElement("div");
    const s = new WarningStrip(parent);
    s.push({ kind: "behind", lag_frames: 3 });
    s.push({ kind: "behind", lag_frames: 50 });
    expect(s.__testCount()).toBe(1);
    const text = parent.textContent ?? "";
    expect(text).toContain("50");
    expect(text).not.toContain("by 3 ");
  });

  it("coalesce bumps an item to the end so it survives the visibility cap (Codex P2 #2)", () => {
    // A frequently-repeating warning that was inserted FIRST should
    // not be evicted by shift() just because of insertion order —
    // its repeated activity should keep it 'most recent'.
    const parent = document.createElement("div");
    const s = new WarningStrip(parent);
    s.push({ kind: "agent_log_slow" }); // First pushed.
    for (let i = 0; i < 10; i++) {
      // Refresh the first warning between distinct warnings.
      s.push({ kind: "agent_log_slow" });
      s.push({ kind: "invalid_action", agent_id: `agent-${i}`, reason: "r" });
    }
    // Even though agent_log_slow was inserted first, its constant
    // refresh means it must still be visible at the end.
    expect(parent.textContent).toContain("slow");
  });
});

describe("HeartbeatBadge", () => {
  it("reports ok / stale / dead based on snapshot age", () => {
    const parent = document.createElement("div");
    const h = new HeartbeatBadge(parent);
    expect(h.update(1000, 1500)).toBe("ok");
    expect(h.update(1000, 2500)).toBe("stale");
    expect(h.update(1000, 5000)).toBe("dead");
    expect(h.update(0, 1000)).toBe("stale");
  });
});

describe("PerfOverlay", () => {
  it("toggle changes visibility", () => {
    const parent = document.createElement("div");
    const p = new PerfOverlay(parent);
    expect(p.isEnabled()).toBe(false);
    p.toggle();
    expect(p.isEnabled()).toBe(true);
    p.toggle();
    expect(p.isEnabled()).toBe(false);
  });

  it("samples fps roughly correctly after enough frames", () => {
    const parent = document.createElement("div");
    const p = new PerfOverlay(parent);
    p.setEnabled(true);
    const t0 = 1000;
    for (let i = 0; i < 30; i++) p.tick(t0 + (i * 500) / 29);
    p.tick(t0 + 500);
    expect(p.__testFps()).toBeGreaterThan(50);
    expect(p.__testFps()).toBeLessThan(80);
  });
});
