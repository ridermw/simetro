// frontend/src/ui/overlays.ts
//
// PLAN §11.1 / §11.2 / §20 DoD #11 — fault overlay + heartbeat +
// perf overlay. These three pieces handle every "something is wrong
// or sluggish" surface the user can see:
//
//   FaultOverlay     — full-bleed banner on Fault. Engine is dead/
//                      paused; user sees what happened and is told
//                      what to do (typically: fix the JSON, hit ↻).
//   WarningStrip     — non-blocking pill stack for Warnings (e.g.
//                      InvalidAction, Behind, TickOverBudget).
//   HeartbeatBadge   — green dot when snapshots are arriving on
//                      schedule, amber when stale > 1s, red after
//                      3s (PLAN §13 #1 stale-channel detection).
//   PerfOverlay      — fps + tick budget read-out, toggled by a
//                      query param (?perf=1) or a hotkey.
//
// All text goes through textContent (PLAN §5.1 / §12). Faults carry
// loader-supplied strings; we treat them as untrusted on principle.

import type { EngineFault, EngineWarning } from "../protocol/messages";

// ───────────────── FaultOverlay ──────────────────────────────

export class FaultOverlay {
  private root: HTMLDivElement;
  private message: HTMLDivElement;

  constructor(parent: HTMLElement) {
    this.root = document.createElement("div");
    this.root.id = "simetro-fault-overlay";
    this.root.setAttribute("role", "alert");
    this.root.style.cssText = [
      "position: absolute",
      "inset: 0",
      "display: none",
      "align-items: center",
      "justify-content: center",
      "background: rgba(14, 17, 22, 0.85)",
      "color: #f7768e",
      "font: 14px ui-monospace, SFMono-Regular, monospace",
      "white-space: pre-wrap",
      "padding: 24px",
      "z-index: 100",
    ].join(";");
    this.message = document.createElement("div");
    this.message.style.cssText = "max-width: 720px; text-align: left";
    this.root.appendChild(this.message);
    parent.appendChild(this.root);
  }

  show(fault: EngineFault): void {
    this.message.textContent = formatFault(fault);
    this.root.style.display = "flex";
  }

  hide(): void {
    this.root.style.display = "none";
  }

  __testRoot(): HTMLDivElement {
    return this.root;
  }
}

export function formatFault(fault: EngineFault): string {
  switch (fault.kind) {
    case "LoadError":
      return `SCENE LOAD ERROR\n\nfield: ${fault.field}\n\n${fault.message}\n\n` +
        "Fix the JSON and click ↻ Reload.";
    case "AgentCrashed":
      return `AGENT CRASHED  (agent ${fault.agent_id})\n\n${fault.message}`;
    case "NumericDrift":
      return `NUMERIC DRIFT at tick ${fault.tick}, mover ${fault.mover}\n\n` +
        "Determinism gate failed. See the runbook (docs/runbook.md).";
    case "ChannelSaturated":
      return `BACKPRESSURE  channel saturated (${fault.lag_frames} frames behind)`;
    case "SystemPanic":
      return `SYSTEM PANIC  in ${fault.system}\n\n${fault.message}`;
    case "SchemaMismatch":
      return `PROTOCOL SCHEMA MISMATCH\n\nengine sent v${fault.found}; ` +
        `this build expects v${fault.supported}.\nUpgrade the desktop shell.`;
  }
}

// ───────────────── WarningStrip ──────────────────────────────

export class WarningStrip {
  private root: HTMLDivElement;
  private items: { el: HTMLDivElement; expiresAt: number }[] = [];

  constructor(parent: HTMLElement) {
    this.root = document.createElement("div");
    this.root.id = "simetro-warnings";
    this.root.setAttribute("aria-live", "polite");
    this.root.style.cssText = [
      "position: absolute",
      "top: 12px",
      "right: 12px",
      "display: flex",
      "flex-direction: column",
      "gap: 4px",
      "z-index: 50",
      "pointer-events: none",
    ].join(";");
    parent.appendChild(this.root);
  }

  push(warning: EngineWarning, durationMs = 4000): void {
    const pill = document.createElement("div");
    pill.style.cssText = [
      "padding: 4px 8px",
      "background: rgba(224, 175, 104, 0.18)",
      "border: 1px solid rgba(224, 175, 104, 0.45)",
      "color: #e0af68",
      "font: 11px ui-monospace, SFMono-Regular, monospace",
      "border-radius: 4px",
      "max-width: 320px",
    ].join(";");
    pill.textContent = formatWarning(warning);
    this.root.appendChild(pill);
    const expiresAt = (typeof performance !== "undefined" ? performance.now() : Date.now()) + durationMs;
    this.items.push({ el: pill, expiresAt });
  }

  /** Call every frame. Sweeps expired pills. */
  tick(nowMs: number): void {
    for (let i = this.items.length - 1; i >= 0; i--) {
      const item = this.items[i];
      if (item !== undefined && item.expiresAt <= nowMs) {
        item.el.remove();
        this.items.splice(i, 1);
      }
    }
  }

  __testCount(): number {
    return this.items.length;
  }
}

export function formatWarning(w: EngineWarning): string {
  switch (w.kind) {
    case "InvalidAction":
      return `agent ${w.agent_id} invalid action: ${w.reason}`;
    case "Behind":
      return `engine behind by ${w.lag_frames} frames`;
    case "TickOverBudget":
      return `tick over budget (${w.ms.toFixed(1)} ms)`;
    case "AgentLogSlow":
      return "agent log writer slow";
  }
}

// ───────────────── HeartbeatBadge ────────────────────────────

export type HeartbeatState = "ok" | "stale" | "dead";

export class HeartbeatBadge {
  private root: HTMLDivElement;

  constructor(parent: HTMLElement) {
    this.root = document.createElement("div");
    this.root.id = "simetro-heartbeat";
    this.root.setAttribute("aria-label", "Engine heartbeat");
    this.root.style.cssText = [
      "position: absolute",
      "bottom: 12px",
      "left: 12px",
      "width: 10px",
      "height: 10px",
      "border-radius: 50%",
      "background: #9ece6a",
      "box-shadow: 0 0 4px rgba(158, 206, 106, 0.7)",
      "z-index: 10",
    ].join(";");
    parent.appendChild(this.root);
  }

  /** lastSnapshotAt = ms; nowMs = current time. */
  update(lastSnapshotAt: number, nowMs: number): HeartbeatState {
    if (lastSnapshotAt === 0) {
      // No snapshot yet — treat as stale, not dead.
      this.paint("stale");
      return "stale";
    }
    const age = nowMs - lastSnapshotAt;
    let state: HeartbeatState;
    if (age < 1000) state = "ok";
    else if (age < 3000) state = "stale";
    else state = "dead";
    this.paint(state);
    return state;
  }

  private paint(state: HeartbeatState): void {
    const color =
      state === "ok" ? "#9ece6a" : state === "stale" ? "#e0af68" : "#f7768e";
    this.root.style.background = color;
    this.root.style.boxShadow = `0 0 4px ${color}b0`;
    this.root.setAttribute(
      "aria-label",
      `Engine heartbeat: ${state}`
    );
  }

  __testRoot(): HTMLDivElement {
    return this.root;
  }
}

// ───────────────── PerfOverlay ───────────────────────────────

export class PerfOverlay {
  private root: HTMLDivElement;
  private lastSampleMs = 0;
  private frameCount = 0;
  private fps = 0;
  private enabled = false;

  constructor(parent: HTMLElement) {
    this.root = document.createElement("div");
    this.root.id = "simetro-perf";
    this.root.style.cssText = [
      "position: absolute",
      "bottom: 12px",
      "right: 12px",
      "padding: 4px 8px",
      "background: rgba(14, 17, 22, 0.85)",
      "border: 1px solid rgba(232, 234, 237, 0.15)",
      "color: #e8eaed",
      "font: 11px ui-monospace, SFMono-Regular, monospace",
      "border-radius: 4px",
      "z-index: 10",
      "display: none",
    ].join(";");
    parent.appendChild(this.root);
  }

  setEnabled(on: boolean): void {
    this.enabled = on;
    this.root.style.display = on ? "block" : "none";
  }

  isEnabled(): boolean {
    return this.enabled;
  }

  toggle(): void {
    this.setEnabled(!this.enabled);
  }

  /** Call once per rAF frame. */
  tick(nowMs: number): void {
    this.frameCount += 1;
    if (this.lastSampleMs === 0) this.lastSampleMs = nowMs;
    const elapsed = nowMs - this.lastSampleMs;
    if (elapsed >= 500) {
      this.fps = (this.frameCount * 1000) / elapsed;
      this.frameCount = 0;
      this.lastSampleMs = nowMs;
      if (this.enabled) {
        this.root.textContent = `${this.fps.toFixed(1)} fps`;
      }
    }
  }

  __testFps(): number {
    return this.fps;
  }
}
