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
    expect(
      formatFault({ kind: "load_error", message: "bad", line: 1, col: 2 })
    ).toContain("SCENE LOAD");
    expect(
      formatFault({ kind: "load_error", message: "bad", line: null, col: null })
    ).toContain("SCENE LOAD");
    expect(
      formatFault({ kind: "agent_crashed", agent_id: "a", message: "m" })
    ).toContain("AGENT CRASHED");
    expect(formatFault({ kind: "numeric_drift", tick: 5 })).toContain("NUMERIC DRIFT");
    expect(formatFault({ kind: "engine_fault", message: "kaboom" })).toContain("ENGINE FAULT");
    expect(
      formatFault({ kind: "baseline_hash_mismatch", expected: "aaa", found: "bbb" })
    ).toContain("BASELINE HASH MISMATCH");
    expect(
      formatFault({ kind: "schema_mismatch", expected: 1, found: 99 })
    ).toContain("SCHEMA MISMATCH");
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

  it("formatWarning covers every kind", () => {
    expect(
      formatWarning({ kind: "invalid_action", agent_id: "a", reason: "x" })
    ).toContain("invalid action");
    expect(formatWarning({ kind: "behind", lag_frames: 4 })).toContain("behind");
    expect(formatWarning({ kind: "tick_over_budget", ms: 12.3 })).toContain("over budget");
    expect(formatWarning({ kind: "agent_log_slow" })).toContain("slow");
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
