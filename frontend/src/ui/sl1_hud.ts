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
  Sl1FailureConditionRuntimeView,
  Sl1FailureConditionView,
  Sl1GameOutcomeView,
  Sl1GamePhase,
  Sl1ObjectiveRuntimeView,
  Sl1ObjectiveStatusTag,
  Sl1ObjectiveView,
  Sl1VictoryConditionRuntimeView,
  Sl1VictoryConditionView,
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
//  Sl1ConditionsPanel
//
//  Shows the scene's declared loss and win conditions with their live
//  runtime status. Answers "what are the stakes?" without requiring a
//  viewer to read scenario JSON.
// ────────────────────────────────────────────────────────────────────

interface ConditionRow {
  id: string;
  statusEl: HTMLSpanElement;
  rowEl: HTMLDivElement;
}

const CONDITION_PENDING_COLOR = "#8b949e";
const CONDITION_FIRED_COLOR = "#f7768e";
const CONDITION_ACHIEVED_COLOR = "#9ece6a";
const CONDITION_STREAK_COLOR = "#e0af68";

export class Sl1ConditionsPanel {
  private root: HTMLDivElement;
  private rowsContainer: HTMLDivElement;
  private failureRowsById: Map<string, ConditionRow> = new Map();
  private victoryRowsById: Map<string, ConditionRow> = new Map();

  constructor(parent: HTMLElement) {
    this.root = document.createElement("div");
    this.root.id = "simetro-sl1-conditions";
    this.root.setAttribute("role", "region");
    this.root.setAttribute("aria-label", "Win and loss conditions");
    this.root.style.cssText = [
      "position: absolute",
      "top: 12px",
      // Use right-anchor with width clamp so the panel stays inside
      // the viewport on narrower windows (Codex non-blocking #1).
      "right: 12px",
      "max-width: min(360px, calc(100vw - 24px))",
      "padding: 8px 12px",
      "background: rgba(14, 17, 22, 0.85)",
      "border: 1px solid #2a2e39",
      "border-radius: 6px",
      "color: #e8eaed",
      "font: 12px ui-monospace, SFMono-Regular, monospace",
      "z-index: 20",
      "pointer-events: none",
      "display: none",
      // Wrap long author-supplied ids/values rather than overflow.
      "overflow-wrap: anywhere",
      "word-break: break-word",
    ].join(";");

    const heading = document.createElement("div");
    heading.style.cssText =
      "opacity: 0.65; text-transform: uppercase; letter-spacing: 0.08em; margin-bottom: 6px;";
    heading.textContent = "conditions";
    this.root.appendChild(heading);

    this.rowsContainer = document.createElement("div");
    this.rowsContainer.style.cssText = "display: flex; flex-direction: column; gap: 4px;";
    this.root.appendChild(this.rowsContainer);

    parent.appendChild(this.root);
  }

  setConditions(
    failure: ReadonlyArray<Sl1FailureConditionView>,
    victory: ReadonlyArray<Sl1VictoryConditionView>
  ): void {
    while (this.rowsContainer.firstChild !== null) {
      this.rowsContainer.removeChild(this.rowsContainer.firstChild);
    }
    this.failureRowsById.clear();
    this.victoryRowsById.clear();
    if (failure.length === 0 && victory.length === 0) {
      this.root.style.display = "none";
      return;
    }

    for (const condition of failure) {
      const row = this.createRow("failure", condition.id, "LOSS", describeFailureCondition(condition));
      row.statusEl.textContent = "armed";
      row.statusEl.style.color = CONDITION_PENDING_COLOR;
      row.rowEl.dataset.failureConditionId = condition.id;
      this.rowsContainer.appendChild(row.rowEl);
      this.failureRowsById.set(condition.id, row);
    }

    for (const condition of victory) {
      const row = this.createRow("victory", condition.id, "WIN", describeVictoryCondition(condition));
      row.statusEl.textContent = "pending";
      row.statusEl.style.color = CONDITION_PENDING_COLOR;
      row.rowEl.dataset.victoryConditionId = condition.id;
      this.rowsContainer.appendChild(row.rowEl);
      this.victoryRowsById.set(condition.id, row);
    }

    this.root.style.display = "block";
  }

  updateFailureStates(states: ReadonlyArray<Sl1FailureConditionRuntimeView>): void {
    for (const state of states) {
      const row = this.failureRowsById.get(state.failure_condition_id);
      if (row === undefined) continue;
      if (state.fired_at_tick !== undefined) {
        row.statusEl.textContent = `FIRED @ tick ${state.fired_at_tick}`;
        row.statusEl.style.color = CONDITION_FIRED_COLOR;
      } else if (state.breach_streak_ticks > 0) {
        row.statusEl.textContent = `streak: ${state.breach_streak_ticks} ticks`;
        row.statusEl.style.color = CONDITION_STREAK_COLOR;
      } else {
        row.statusEl.textContent = "armed";
        row.statusEl.style.color = CONDITION_PENDING_COLOR;
      }
    }
  }

  updateVictoryStates(states: ReadonlyArray<Sl1VictoryConditionRuntimeView>): void {
    for (const state of states) {
      const row = this.victoryRowsById.get(state.victory_condition_id);
      if (row === undefined) continue;
      if (state.met_at_tick !== undefined) {
        row.statusEl.textContent = `ACHIEVED @ tick ${state.met_at_tick}`;
        row.statusEl.style.color = CONDITION_ACHIEVED_COLOR;
      } else {
        row.statusEl.textContent = "pending";
        row.statusEl.style.color = CONDITION_PENDING_COLOR;
      }
    }
  }

  reset(): void {
    while (this.rowsContainer.firstChild !== null) {
      this.rowsContainer.removeChild(this.rowsContainer.firstChild);
    }
    this.failureRowsById.clear();
    this.victoryRowsById.clear();
    this.root.style.display = "none";
  }

  __testRoot(): HTMLElement {
    return this.root;
  }

  private createRow(
    kind: "failure" | "victory",
    id: string,
    badgeText: "LOSS" | "WIN",
    description: string
  ): ConditionRow {
    const rowEl = document.createElement("div");
    rowEl.dataset.conditionKind = kind;
    rowEl.style.cssText = "display: flex; align-items: baseline; gap: 8px;";

    const badge = document.createElement("span");
    const badgeColor = kind === "failure" ? CONDITION_FIRED_COLOR : CONDITION_ACHIEVED_COLOR;
    badge.style.cssText = [
      "min-width: 34px",
      "padding: 1px 5px",
      "border-radius: 3px",
      `background: ${badgeColor}`,
      "color: #0e1116",
      "font-size: 10px",
      "font-weight: 700",
      "text-align: center",
    ].join(";");
    badge.textContent = badgeText;
    rowEl.appendChild(badge);

    const textEl = document.createElement("div");
    textEl.style.cssText = "flex: 1;";
    textEl.textContent = description;
    rowEl.appendChild(textEl);

    const statusEl = document.createElement("span");
    statusEl.dataset.conditionStatus = kind;
    statusEl.style.cssText =
      "min-width: 80px; max-width: 160px; text-align: right; font-weight: 600; overflow-wrap: anywhere;";
    rowEl.appendChild(statusEl);

    return { id, statusEl, rowEl };
  }
}

export function describeFailureCondition(c: Sl1FailureConditionView): string {
  const p = c.params;
  switch (p.kind) {
    case "stale_target":
      return `${p.thing} in ${p.place} stale > ${p.threshold_ticks} ticks (grace ${p.grace_ticks})`;
    case "place_state":
      return `${p.place} in state ${p.state} (grace ${p.grace_ticks})`;
    case "objective_breach_count":
      return `Objective ${p.objective_id} breached > ${p.max_count} times`;
    default:
      return `Failure condition "${c.id}" (${c.type})`;
  }
}

export function describeVictoryCondition(c: Sl1VictoryConditionView): string {
  const p = c.params;
  switch (p.kind) {
    case "survive_until":
      return `Survive until tick ${p.at_tick}`;
    default:
      return `Victory condition "${c.id}" (${c.type})`;
  }
}

// ────────────────────────────────────────────────────────────────────
//  Sl1ObjectivesPanel
//
//  Shows the scene's declared objectives (sorted by descending
//  weight) and their runtime status (unknown / met / breached /
//  unsupported). Answers "what is the AI trying to optimize?" in
//  plain English so a viewer doesn't have to read the JSON.
// ────────────────────────────────────────────────────────────────────

interface ObjectiveRow {
  id: string;
  textEl: HTMLDivElement;
  statusEl: HTMLSpanElement;
  rowEl: HTMLDivElement;
}

const OBJECTIVE_STATUS_COLOR: Record<Sl1ObjectiveStatusTag, string> = {
  unknown: "#8b949e",
  met: "#9ece6a",
  breached: "#f7768e",
  unsupported: "#8b949e",
};

const OBJECTIVE_STATUS_LABEL: Record<Sl1ObjectiveStatusTag, string> = {
  unknown: "—",
  met: "met",
  breached: "breached",
  unsupported: "n/a",
};

export class Sl1ObjectivesPanel {
  private root: HTMLDivElement;
  private rowsContainer: HTMLDivElement;
  private rowsById: Map<string, ObjectiveRow> = new Map();

  constructor(parent: HTMLElement) {
    this.root = document.createElement("div");
    this.root.id = "simetro-sl1-objectives";
    this.root.setAttribute("role", "region");
    this.root.setAttribute("aria-label", "Scenario objectives");
    this.root.style.cssText = [
      "position: absolute",
      "top: 12px",
      "left: 240px",
      "max-width: 380px",
      "padding: 8px 12px",
      "background: rgba(14, 17, 22, 0.85)",
      "border: 1px solid #2a2e39",
      "border-radius: 6px",
      "color: #e8eaed",
      "font: 12px ui-monospace, SFMono-Regular, monospace",
      "z-index: 20",
      "pointer-events: none",
      "display: none",
    ].join(";");

    const heading = document.createElement("div");
    heading.style.cssText =
      "opacity: 0.65; text-transform: uppercase; letter-spacing: 0.08em; margin-bottom: 6px;";
    heading.textContent = "objectives";
    this.root.appendChild(heading);

    this.rowsContainer = document.createElement("div");
    this.rowsContainer.style.cssText = "display: flex; flex-direction: column; gap: 4px;";
    this.root.appendChild(this.rowsContainer);

    parent.appendChild(this.root);
  }

  /** Replace the rendered objectives. Sorted by descending weight so
   *  the most important objective is on top. */
  setObjectives(objectives: ReadonlyArray<Sl1ObjectiveView>): void {
    while (this.rowsContainer.firstChild !== null) {
      this.rowsContainer.removeChild(this.rowsContainer.firstChild);
    }
    this.rowsById.clear();
    if (objectives.length === 0) {
      this.root.style.display = "none";
      return;
    }
    const sorted = [...objectives].sort((a, b) => b.weight - a.weight);
    for (const obj of sorted) {
      const rowEl = document.createElement("div");
      rowEl.dataset.objectiveId = obj.id;
      rowEl.style.cssText = "display: flex; align-items: baseline; gap: 8px;";

      const weightBadge = document.createElement("span");
      weightBadge.style.cssText =
        "min-width: 22px; padding: 1px 5px; border-radius: 3px; background: #21262d; color: #c0caf5; font-size: 10px; text-align: center;";
      weightBadge.textContent = `w${obj.weight}`;
      rowEl.appendChild(weightBadge);

      const textEl = document.createElement("div");
      textEl.style.cssText = "flex: 1;";
      textEl.textContent = describeObjective(obj);
      rowEl.appendChild(textEl);

      const statusEl = document.createElement("span");
      statusEl.style.cssText = "min-width: 60px; text-align: right; font-weight: 600;";
      statusEl.textContent = OBJECTIVE_STATUS_LABEL.unknown;
      statusEl.style.color = OBJECTIVE_STATUS_COLOR.unknown;
      rowEl.appendChild(statusEl);

      this.rowsContainer.appendChild(rowEl);
      this.rowsById.set(obj.id, { id: obj.id, textEl, statusEl, rowEl });
    }
    this.root.style.display = "block";
  }

  /** Apply per-tick objective runtime states. Unknown objectives in
   *  the input are ignored; rows for objectives not in the input keep
   *  their previous status (no flicker on sparse updates). */
  updateStates(states: ReadonlyArray<Sl1ObjectiveRuntimeView>): void {
    for (const s of states) {
      const row = this.rowsById.get(s.objective_id);
      if (row === undefined) continue;
      const label = OBJECTIVE_STATUS_LABEL[s.status] ?? OBJECTIVE_STATUS_LABEL.unknown;
      const color = OBJECTIVE_STATUS_COLOR[s.status] ?? OBJECTIVE_STATUS_COLOR.unknown;
      row.statusEl.textContent = label;
      row.statusEl.style.color = color;
    }
  }

  reset(): void {
    while (this.rowsContainer.firstChild !== null) {
      this.rowsContainer.removeChild(this.rowsContainer.firstChild);
    }
    this.rowsById.clear();
    this.root.style.display = "none";
  }

  __testRoot(): HTMLElement {
    return this.root;
  }
}

/** Convert an objective into a short human-readable sentence.
 *  Pure helper, exported for unit testing. */
export function describeObjective(obj: Sl1ObjectiveView): string {
  const p = obj.params;
  switch (p.kind) {
    case "keep_fresh":
      return `Keep ${p.thing} fresh in ${p.place} (≤${p.max_stale_ticks} ticks stale)`;
    case "complete_jobs_before_deadline":
      return `Complete demand ${p.demand} (≤${p.max_missed} missed)`;
    case "maintain_utilization":
      return `Keep ${p.place} ${p.capacity} utilization between ${p.min_percent}% and ${p.max_percent}%`;
    case "unsupported_in_this_pr":
      return `Objective "${obj.id}" (${obj.type}) — not yet supported by this renderer`;
    default:
      return `Objective "${obj.id}" (${obj.type})`;
  }
}

// ────────────────────────────────────────────────────────────────────
//  Composite helper used by main.ts boot wiring
// ────────────────────────────────────────────────────────────────────

export interface Sl1Hud {
  status: Sl1StatusPanel;
  objectives: Sl1ObjectivesPanel;
  conditions: Sl1ConditionsPanel;
  milestones: Sl1MilestoneStrip;
  dashboards: Sl1DashboardChips;
  alerts: Sl1AlertStrip;
  reset(): void;
}

export function createSl1Hud(parent: HTMLElement): Sl1Hud {
  const status = new Sl1StatusPanel(parent);
  const objectives = new Sl1ObjectivesPanel(parent);
  const conditions = new Sl1ConditionsPanel(parent);
  const milestones = new Sl1MilestoneStrip(parent);
  const dashboards = new Sl1DashboardChips(parent);
  const alerts = new Sl1AlertStrip(parent);
  return {
    status,
    objectives,
    conditions,
    milestones,
    dashboards,
    alerts,
    reset(): void {
      status.reset();
      objectives.reset();
      conditions.reset();
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
  staticAlerts: Sl1AlertView[] | undefined,
  staticObjectives: Sl1ObjectiveView[] | undefined = undefined,
  staticFailure: Sl1FailureConditionView[] | undefined = undefined,
  staticVictory: Sl1VictoryConditionView[] | undefined = undefined
): void {
  hud.dashboards.setDashboards(staticDashboards ?? []);
  hud.alerts.setAlerts(staticAlerts ?? []);
  hud.objectives.setObjectives(staticObjectives ?? []);
  hud.conditions.setConditions(staticFailure ?? [], staticVictory ?? []);
}
