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
    f.show({ kind: "LoadError", field: "<img src=x>", message: "<script>evil</script>" });
    const root = f.__testRoot();
    expect(root.style.display).toBe("flex");
    expect(root.textContent).toContain("<img src=x>");
    expect(root.textContent).toContain("<script>evil</script>");
    expect(root.querySelector("script")).toBeNull();
    expect(root.querySelector("img")).toBeNull();
  });

  it("formatFault covers every fault kind", () => {
    expect(formatFault({ kind: "LoadError", field: "x", message: "y" })).toContain("SCENE LOAD");
    expect(formatFault({ kind: "AgentCrashed", agent_id: 1, message: "m" })).toContain("AGENT CRASHED");
    expect(formatFault({ kind: "NumericDrift", tick: 5, mover: 7 })).toContain("NUMERIC DRIFT");
    expect(formatFault({ kind: "ChannelSaturated", lag_frames: 9 })).toContain("BACKPRESSURE");
    expect(formatFault({ kind: "SystemPanic", system: "movement", message: "boom" })).toContain("SYSTEM PANIC");
    expect(formatFault({ kind: "SchemaMismatch", found: 99, supported: 1 })).toContain("SCHEMA MISMATCH");
  });

  it("hide() restores display: none", () => {
    const parent = document.createElement("div");
    const f = new FaultOverlay(parent);
    f.show({ kind: "ChannelSaturated", lag_frames: 1 });
    f.hide();
    expect(f.__testRoot().style.display).toBe("none");
  });
});

describe("WarningStrip", () => {
  it("push adds a pill; tick() sweeps expired ones", () => {
    const parent = document.createElement("div");
    const s = new WarningStrip(parent);
    s.push({ kind: "Behind", lag_frames: 3 }, 100);
    expect(s.__testCount()).toBe(1);
    s.tick(performance.now() + 1000);
    expect(s.__testCount()).toBe(0);
  });

  it("formatWarning covers every kind", () => {
    expect(formatWarning({ kind: "InvalidAction", agent_id: 1, reason: "x" })).toContain("invalid action");
    expect(formatWarning({ kind: "Behind", lag_frames: 4 })).toContain("behind");
    expect(formatWarning({ kind: "TickOverBudget", ms: 12.3 })).toContain("over budget");
    expect(formatWarning({ kind: "AgentLogSlow" })).toContain("slow");
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
    // Simulate 30 frames over 500ms ≈ 60 fps.
    for (let i = 0; i < 30; i++) p.tick(t0 + (i * 500) / 29);
    p.tick(t0 + 500);
    expect(p.__testFps()).toBeGreaterThan(50);
    expect(p.__testFps()).toBeLessThan(80);
  });
});
