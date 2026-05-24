// frontend/src/inspector/panel.ts
//
// PLAN §4 / §20 DoD #6 — Inspector panel. Shows the most recent
// AgentReport: agent id (string in the protocol mirror), considered
// actions with confidence, chosen action, free-text rationale, plus
// a tiny scrolling timeline of recent decisions.
//
// All text rendering goes through textContent — never innerHTML —
// per PLAN §5.1 / §12 (eslint-plugin-no-unsanitized enforces this).
//
//   ┌──────────────────────────────────────────────────┐
//   │  AGENT speed_tuner_0   tick 142   conf 0.83      │
//   │  ─────────────────────────────────────────────── │
//   │  CHOSEN   SetSpeed(mover=12, speed=1.60)         │
//   │  CONSIDERED                                      │
//   │   • SetSpeed(mover=12, speed=1.60)  0.83  ●      │
//   │   • SetSpeed(mover=12, speed=1.40)  0.71         │
//   │   • NoOp                            0.42         │
//   │  RATIONALE                                       │
//   │  "crowded pickup, accelerate to clear backlog"   │
//   │  TIMELINE  ▎▎▌▌▎▌▎▎▎▎  (latest 16 ticks)         │
//   └──────────────────────────────────────────────────┘

import {
  formatAction,
  type Action,
  type AgentReport,
} from "../protocol/messages";

const TIMELINE_CAP = 16;

interface PanelHandles {
  root: HTMLElement;
  agentLine: HTMLElement;
  chosen: HTMLElement;
  considered: HTMLElement;
  rationale: HTMLElement;
  timeline: HTMLElement;
}

function actionsEqual(a: Action | null, b: Action): boolean {
  if (a === null) return false;
  return JSON.stringify(a) === JSON.stringify(b);
}

export class InspectorPanel {
  private handles: PanelHandles;
  private recent: AgentReport[] = [];

  constructor(parent: HTMLElement) {
    this.handles = buildDom(parent);
  }

  show(report: AgentReport): void {
    this.recent.push(report);
    if (this.recent.length > TIMELINE_CAP) this.recent.shift();
    this.render();
  }

  setVisible(visible: boolean): void {
    this.handles.root.style.display = visible ? "block" : "none";
  }

  /** Test-only accessor; do not rely on in production code. */
  __testRoot(): HTMLElement {
    return this.handles.root;
  }

  private render(): void {
    const latest = this.recent[this.recent.length - 1];
    if (latest === undefined) return;

    this.handles.agentLine.textContent =
      `AGENT ${latest.agent_id}   tick ${latest.tick}   ` +
      `confidence ${latest.confidence.toFixed(2)}`;

    this.handles.chosen.textContent = `CHOSEN  ${formatAction(latest.chosen)}`;

    // Clear children safely (no innerHTML).
    while (this.handles.considered.firstChild !== null) {
      this.handles.considered.removeChild(this.handles.considered.firstChild);
    }
    const consHeader = document.createElement("div");
    consHeader.textContent = "CONSIDERED";
    consHeader.className = "simetro-inspector-section";
    this.handles.considered.appendChild(consHeader);
    for (const c of latest.considered) {
      const row = document.createElement("div");
      row.className = "simetro-inspector-row";
      const dot = actionsEqual(latest.chosen, c.action) ? " ●" : "";
      row.textContent =
        `  • ${formatAction(c.action)}    ${c.confidence.toFixed(2)}${dot}`;
      this.handles.considered.appendChild(row);
    }

    this.handles.rationale.textContent =
      `RATIONALE\n${latest.rationale || "(no rationale)"}`;

    // Timeline: one glyph per recent report, weighted by confidence.
    const glyphs: string[] = [];
    for (const r of this.recent) {
      const intensity = Math.max(0, Math.min(1, r.confidence));
      glyphs.push(intensity > 0.66 ? "▌" : intensity > 0.33 ? "▎" : "▏");
    }
    this.handles.timeline.textContent = `TIMELINE  ${glyphs.join("")}`;
  }
}

function buildDom(parent: HTMLElement): PanelHandles {
  const root = document.createElement("div");
  root.id = "simetro-inspector";
  root.setAttribute("role", "region");
  root.setAttribute("aria-label", "Agent inspector");
  root.style.cssText = [
    "position: absolute",
    "right: 12px",
    "bottom: 12px",
    "width: 320px",
    "padding: 12px 14px",
    "background: rgba(14, 17, 22, 0.85)",
    "color: #e8eaed",
    "font: 12px ui-monospace, SFMono-Regular, monospace",
    "white-space: pre-wrap",
    "border: 1px solid rgba(232, 234, 237, 0.15)",
    "border-radius: 8px",
    "pointer-events: none",
    "display: block",
    "z-index: 10",
  ].join(";");

  const agentLine = document.createElement("div");
  agentLine.className = "simetro-inspector-header";
  agentLine.textContent = "AGENT  —   tick —   confidence —";

  const divider = document.createElement("div");
  divider.style.cssText =
    "border-bottom: 1px solid rgba(232, 234, 237, 0.15); margin: 6px 0";

  const chosen = document.createElement("div");
  chosen.className = "simetro-inspector-chosen";
  chosen.textContent = "CHOSEN  —";

  const considered = document.createElement("div");
  considered.className = "simetro-inspector-considered";

  const rationale = document.createElement("div");
  rationale.className = "simetro-inspector-rationale";
  rationale.style.cssText = "margin-top: 6px";

  const timeline = document.createElement("div");
  timeline.className = "simetro-inspector-timeline";
  timeline.style.cssText = "margin-top: 8px; letter-spacing: 1px";

  root.appendChild(agentLine);
  root.appendChild(divider);
  root.appendChild(chosen);
  root.appendChild(considered);
  root.appendChild(rationale);
  root.appendChild(timeline);
  parent.appendChild(root);

  return { root, agentLine, chosen, considered, rationale, timeline };
}
