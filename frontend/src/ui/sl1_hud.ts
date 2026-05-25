// frontend/src/ui/sl1_hud.ts
//
// scenario_language_v1 (SL1) heads-up display — the visible answer to
// the 30-second viewer litmus questions for AI-operated scenarios:
//
//   1. What is the AI trying to save?         → Sl1StatusPanel
//   2. What is going wrong?                   → Sl1AlertStrip +
//                                               Sl1DashboardChip
//   3. Did the last action help?              → Sl1MilestoneStrip
//
// All author-supplied / engine-supplied strings (milestone labels,
// dashboard ids, alert ids, outcome reasons) are rendered via
// `textContent` (NEVER `innerHTML`). This is the safe-text policy
// that gates every SL1 PR — any string ultimately sourced from JSON,
// user input, or an LLM goes through `textContent` to defeat XSS /
// prompt-injection payloads embedded in scene authoring data.
//
// HUD panels are deliberately tiny DOM islands rather than a
// framework — the engine drives them imperatively from snapshots
// and events, and they expose `__testRoot()` so vitest can assert
// the rendered text directly without relying on jsdom layout.

import type {
  Sl1AlertStateView,
  Sl1AlertView,
  Sl1DashboardStateView,
  Sl1DashboardView,
  Sl1GameOutcomeView,
  Sl1GamePhase,
} from "../protocol/messages";

// ────────────────────────────────────────────────────────────────────
//  Sl1StatusPanel
//
//  Shows GameOutcome (in_progress / won / lost), derived game phase
//  (winning / losing / stabilizing / spiraling), and the loss reason
//  when applicable. Answers "what is the AI trying to save?" + the
//  binary "is the AI still alive?" at a glance.
// ────────────────────────────────────────────────────────────────────

export class Sl1StatusPanel {
  private root: HTMLDivElement;
  private outcomeEl: HTMLDivElement;
  private phaseEl: HTMLDivElement;
  private reasonEl: HTMLDivElement;

  constructor(parent: HTMLElement) {
    this.root = document.createElement("div");
    this.root.id = "simetro-sl1-status";
    this.root.setAttribute("role", "status");
    this.root.setAttribute("aria-label", "Scenario status");
    this.root.style.cssText = [
      "position: absolute",
      "top: 12px",
      "left: 12px",
      "min-width: 200px",
      "padding: 8px 12px",
      "background: rgba(14, 17, 22, 0.85)",
      "border: 1px solid #2a2e39",
      "border-radius: 6px",
      "color: #e8eaed",
      "font: 12px ui-monospace, SFMono-Regular, monospace",
      "z-index: 20",
      "pointer-events: none",
    ].join(";");

    this.outcomeEl = document.createElement("div");
    this.outcomeEl.id = "simetro-sl1-status-outcome";
    this.outcomeEl.style.cssText = "font-weight: 600; margin-bottom: 4px";
    this.outcomeEl.textContent = "Outcome: —";

    this.phaseEl = document.createElement("div");
    this.phaseEl.id = "simetro-sl1-status-phase";
    this.phaseEl.style.cssText = "color: #c0caf5";
    this.phaseEl.textContent = "Phase: —";

    this.reasonEl = document.createElement("div");
    this.reasonEl.id = "simetro-sl1-status-reason";
    this.reasonEl.style.cssText = "color: #f7768e; margin-top: 4px; display: none";

    this.root.appendChild(this.outcomeEl);
    this.root.appendChild(this.phaseEl);
    this.root.appendChild(this.reasonEl);
    parent.appendChild(this.root);
  }

  /** Hide the panel entirely. Used when switching to a non-SL1 scene. */
  reset(): void {
    this.outcomeEl.textContent = "Outcome: —";
    this.phaseEl.textContent = "Phase: —";
    this.reasonEl.textContent = "";
    this.reasonEl.style.display = "none";
    this.root.style.display = "none";
  }

  update(outcome: Sl1GameOutcomeView | undefined, phase: Sl1GamePhase | string | undefined): void {
    if (outcome === undefined) {
      this.reset();
      return;
    }
    this.root.style.display = "block";
    this.outcomeEl.textContent = `Outcome: ${formatOutcomeState(outcome.state)}`;
    this.phaseEl.textContent = `Phase: ${formatPhase(phase)}`;
    if (outcome.state === "lost" && typeof outcome.reason === "string" && outcome.reason !== "") {
      // outcome.reason is engine-derived (e.g. "failure_condition:exec-dashboard-stale")
      // but the trailing id segment is author-supplied. Rendering via
      // textContent keeps the payload safe.
      this.reasonEl.textContent = `Reason: ${outcome.reason}`;
      this.reasonEl.style.display = "block";
    } else {
      this.reasonEl.textContent = "";
      this.reasonEl.style.display = "none";
    }
  }

  __testRoot(): HTMLDivElement {
    return this.root;
  }
}

export function formatOutcomeState(state: string): string {
  switch (state) {
    case "in_progress":
      return "in progress";
    case "won":
      return "won";
    case "lost":
      return "lost";
    default:
      return state;
  }
}

export function formatPhase(phase: string | undefined): string {
  if (phase === undefined || phase === "") return "—";
  return phase;
}

// ────────────────────────────────────────────────────────────────────
//  Sl1MilestoneStrip
//
//  Append-only chip strip — one chip per milestone fired during the
//  run, in fire order. Answers "did the last action help?" by giving
//  the viewer a visible event-stream they can scan.
// ────────────────────────────────────────────────────────────────────

export interface Sl1MilestoneChipInput {
  milestone_id: string;
  label: string;
  tick: number;
}

export class Sl1MilestoneStrip {
  private root: HTMLDivElement;
  /** Deduplicates so re-emitted milestones (e.g. on Reload) do not
   *  pile up. The wire-protocol contract is that a milestone fires at
   *  most once per scene run, but the HUD must be resilient to
   *  replay/reset paths. */
  private seen: Set<string>;

  constructor(parent: HTMLElement) {
    this.root = document.createElement("div");
    this.root.id = "simetro-sl1-milestones";
    this.root.setAttribute("role", "list");
    this.root.setAttribute("aria-label", "Scenario milestones");
    this.root.style.cssText = [
      "position: absolute",
      "bottom: 56px",
      "left: 12px",
      "right: 12px",
      "display: flex",
      "flex-wrap: wrap",
      "gap: 6px",
      "max-height: 96px",
      "overflow: hidden",
      "z-index: 18",
      "pointer-events: none",
    ].join(";");
    this.seen = new Set();
    parent.appendChild(this.root);
  }

  push(input: Sl1MilestoneChipInput): void {
    if (this.seen.has(input.milestone_id)) return;
    this.seen.add(input.milestone_id);
    const chip = document.createElement("div");
    chip.className = "simetro-sl1-milestone-chip";
    chip.setAttribute("role", "listitem");
    chip.setAttribute("data-milestone-id", input.milestone_id);
    chip.style.cssText = [
      "padding: 4px 8px",
      "background: rgba(122, 162, 247, 0.18)",
      "border: 1px solid #7aa2f7",
      "border-radius: 12px",
      "color: #c0caf5",
      "font: 11px ui-monospace, SFMono-Regular, monospace",
      "white-space: nowrap",
    ].join(";");
    // Author-supplied label + tick are concatenated via textContent so
    // any embedded markup is rendered literally.
    chip.textContent = `t=${input.tick}  ${input.label}`;
    this.root.appendChild(chip);
  }

  reset(): void {
    this.seen.clear();
    while (this.root.firstChild !== null) {
      this.root.removeChild(this.root.firstChild);
    }
  }

  __testRoot(): HTMLDivElement {
    return this.root;
  }

  __chipCount(): number {
    return this.root.children.length;
  }
}

// ────────────────────────────────────────────────────────────────────
//  Sl1DashboardChip strip
//
//  One small chip per dashboard, showing its freshness state (ok /
//  stale / no_data). Answers "what is going wrong?" by surfacing
//  staleness across executive/live/ad-hoc dashboards.
// ────────────────────────────────────────────────────────────────────

export class Sl1DashboardChips {
  private root: HTMLDivElement;
  private chips: Map<string, HTMLDivElement>;

  constructor(parent: HTMLElement) {
    this.root = document.createElement("div");
    this.root.id = "simetro-sl1-dashboards";
    this.root.setAttribute("role", "list");
    this.root.setAttribute("aria-label", "Dashboard freshness");
    this.root.style.cssText = [
      "position: absolute",
      "top: 12px",
      "right: 12px",
      "display: flex",
      "flex-direction: column",
      "gap: 4px",
      "max-width: 240px",
      "z-index: 20",
      "pointer-events: none",
    ].join(";");
    this.chips = new Map();
    parent.appendChild(this.root);
  }

  /** Set the static list of dashboards (called from the `static` message). */
  setDashboards(dashboards: Sl1DashboardView[]): void {
    // Clear existing chips and rebuild — dashboard set is immutable per
    // scene, but switching scenes is a real path.
    this.chips.clear();
    while (this.root.firstChild !== null) {
      this.root.removeChild(this.root.firstChild);
    }
    for (const d of dashboards) {
      const chip = document.createElement("div");
      chip.className = "simetro-sl1-dashboard-chip";
      chip.setAttribute("role", "listitem");
      chip.setAttribute("data-dashboard-id", d.id);
      chip.style.cssText = [
        "padding: 4px 8px",
        "background: rgba(14, 17, 22, 0.85)",
        "border: 1px solid #2a2e39",
        "border-radius: 6px",
        "color: #e8eaed",
        "font: 11px ui-monospace, SFMono-Regular, monospace",
        "white-space: nowrap",
      ].join(";");
      // Author-supplied id; safe text.
      chip.textContent = `${d.id}  •  —`;
      this.root.appendChild(chip);
      this.chips.set(d.id, chip);
    }
  }

  updateStates(states: Sl1DashboardStateView[]): void {
    for (const s of states) {
      const chip = this.chips.get(s.dashboard_id);
      if (chip === undefined) continue;
      chip.setAttribute("data-state", s.state);
      const freshness =
        typeof s.freshness_ticks === "number" ? `  ${s.freshness_ticks}t` : "";
      chip.textContent = `${s.dashboard_id}  •  ${s.state}${freshness}`;
      chip.style.borderColor = dashboardBorderColor(s.state);
      chip.style.color = dashboardTextColor(s.state);
    }
  }

  reset(): void {
    this.chips.clear();
    while (this.root.firstChild !== null) {
      this.root.removeChild(this.root.firstChild);
    }
  }

  __testRoot(): HTMLDivElement {
    return this.root;
  }
}

function dashboardBorderColor(state: string): string {
  switch (state) {
    case "ok":
      return "#9ece6a";
    case "stale":
      return "#e0af68";
    case "no_data":
      return "#f7768e";
    default:
      return "#2a2e39";
  }
}

function dashboardTextColor(state: string): string {
  switch (state) {
    case "ok":
      return "#9ece6a";
    case "stale":
      return "#e0af68";
    case "no_data":
      return "#f7768e";
    default:
      return "#e8eaed";
  }
}

// ────────────────────────────────────────────────────────────────────
//  Sl1AlertStrip
//
//  Stack of pills for firing alerts (severity-coded). Inactive
//  alerts are hidden. Answers "what is going wrong?" at a glance.
// ────────────────────────────────────────────────────────────────────

export class Sl1AlertStrip {
  private root: HTMLDivElement;
  private alerts: Map<string, Sl1AlertView>;
  private pills: Map<string, HTMLDivElement>;

  constructor(parent: HTMLElement) {
    this.root = document.createElement("div");
    this.root.id = "simetro-sl1-alerts";
    this.root.setAttribute("role", "list");
    this.root.setAttribute("aria-label", "Active alerts");
    this.root.style.cssText = [
      "position: absolute",
      "top: 96px",
      "right: 12px",
      "display: flex",
      "flex-direction: column",
      "gap: 4px",
      "max-width: 280px",
      "z-index: 19",
      "pointer-events: none",
    ].join(";");
    this.alerts = new Map();
    this.pills = new Map();
    parent.appendChild(this.root);
  }

  setAlerts(alerts: Sl1AlertView[]): void {
    this.alerts.clear();
    this.pills.clear();
    while (this.root.firstChild !== null) {
      this.root.removeChild(this.root.firstChild);
    }
    for (const a of alerts) {
      this.alerts.set(a.id, a);
    }
  }

  updateStates(states: Sl1AlertStateView[]): void {
    for (const s of states) {
      const def = this.alerts.get(s.alert_id);
      if (def === undefined) continue;
      const existing = this.pills.get(s.alert_id);
      if (s.state === "firing") {
        if (existing === undefined) {
          const pill = document.createElement("div");
          pill.className = "simetro-sl1-alert-pill";
          pill.setAttribute("role", "listitem");
          pill.setAttribute("data-alert-id", s.alert_id);
          pill.setAttribute("data-severity", def.severity);
          pill.style.cssText = [
            "padding: 4px 8px",
            "background: rgba(14, 17, 22, 0.9)",
            `border: 1px solid ${alertBorderColor(def.severity)}`,
            "border-radius: 6px",
            `color: ${alertBorderColor(def.severity)}`,
            "font: 11px ui-monospace, SFMono-Regular, monospace",
            "white-space: nowrap",
          ].join(";");
          // All concatenated strings come from author-supplied ids;
          // textContent renders any embedded markup literally.
          pill.textContent = `[${def.severity}] ${s.alert_id}`;
          this.root.appendChild(pill);
          this.pills.set(s.alert_id, pill);
        }
      } else {
        if (existing !== undefined) {
          this.root.removeChild(existing);
          this.pills.delete(s.alert_id);
        }
      }
    }
  }

  reset(): void {
    this.alerts.clear();
    this.pills.clear();
    while (this.root.firstChild !== null) {
      this.root.removeChild(this.root.firstChild);
    }
  }

  __testRoot(): HTMLDivElement {
    return this.root;
  }
}

function alertBorderColor(severity: string): string {
  switch (severity) {
    case "critical":
      return "#f7768e";
    case "warning":
      return "#e0af68";
    case "info":
      return "#7aa2f7";
    default:
      return "#7aa2f7";
  }
}

// ────────────────────────────────────────────────────────────────────
//  Composite helper used by main.ts boot wiring
// ────────────────────────────────────────────────────────────────────

export interface Sl1Hud {
  status: Sl1StatusPanel;
  milestones: Sl1MilestoneStrip;
  dashboards: Sl1DashboardChips;
  alerts: Sl1AlertStrip;
  reset(): void;
}

export function createSl1Hud(parent: HTMLElement): Sl1Hud {
  const status = new Sl1StatusPanel(parent);
  const milestones = new Sl1MilestoneStrip(parent);
  const dashboards = new Sl1DashboardChips(parent);
  const alerts = new Sl1AlertStrip(parent);
  return {
    status,
    milestones,
    dashboards,
    alerts,
    reset(): void {
      status.reset();
      milestones.reset();
      dashboards.reset();
      alerts.reset();
    },
  };
}

export function resetSl1Hud(hud: Sl1Hud): void {
  hud.reset();
}

/** Show / hide all SL1 panels based on whether the current scene
 *  carries SL1 metadata. Mock or non-SL1 scenes leave the panels
 *  hidden so the canvas remains visually unchanged. */
export function applySl1HudStatic(
  hud: Sl1Hud,
  staticDashboards: Sl1DashboardView[] | undefined,
  staticAlerts: Sl1AlertView[] | undefined
): void {
  hud.dashboards.setDashboards(staticDashboards ?? []);
  hud.alerts.setAlerts(staticAlerts ?? []);
}
