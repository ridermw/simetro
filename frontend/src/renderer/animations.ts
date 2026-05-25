// frontend/src/renderer/animations.ts
//
// ┌──────────────────────────────────────────────────────────────┐
// │       hot-reload animation target: THE HMR TARGET FILE              │
// │                                                              │
// │ Edit this file to retune juice. Vite's HMR boundary lives    │
// │ on this module so saving here patches running animations in  │
// │ <300ms without losing sim state.                             │
// │                                                              │
// │   SimEvent (one of mover_departed / arrived / …)             │
// │       │                                                      │
// │       ▼                                                      │
// │   table lookup ─▶ AnimationSpec { duration, ease, render }   │
// │       │                                                      │
// │       ▼                                                      │
// │   AnimationEngine.spawn() pushes a live slot                 │
// │       │                                                      │
// │       ▼                                                      │
// │   each frame, slots draw at eased t and self-expire          │
// └──────────────────────────────────────────────────────────────┘
//
// Render functions receive (ctx, easedT, payload, ctxResolver).
// They MUST NOT allocate. Per review feedback the
// midpoint helper returns a module-level scratch point rather than
// a fresh `{x, y}` object every call.
//
// ResolveCtx carries the static scene (nodes + paths) because node
// and path lookups operate on `StaticPayload`, not snapshots, since
// wire-protocol contract moved geometry out of `SnapshotPayload`.

import type {
  MoverState,
  NodeView,
  SimEvent,
  SnapshotPayload,
  StaticPayload,
} from "../protocol/messages";
import { easings, foregroundColor, paletteColor, type Theme } from "./theme";

export type SimEventKind = SimEvent["kind"];

export interface ResolveCtx {
  theme: Theme;
  scene: StaticPayload;
  snapshot: SnapshotPayload;
}

export type RenderFn = (
  ctx: CanvasRenderingContext2D,
  easedT: number,
  payload: SimEvent,
  resolve: ResolveCtx
) => void;

export interface AnimationSpec {
  durationMs: number;
  ease: (t: number) => number;
  render: RenderFn;
}

// ──────── helpers (allocation-free; safe in hot path) ───────────

function findNode(scene: StaticPayload, id: number): NodeView | undefined {
  for (const n of scene.nodes) if (n.id === id) return n;
  return undefined;
}

function findMover(snap: SnapshotPayload, id: number): MoverState | undefined {
  for (const m of snap.movers) if (m.id === id) return m;
  return undefined;
}

/** Module-level scratch returned by `findPathMidpoint` so the render
 *  helpers can read x/y without allocating a fresh object per call
 *  (zero-allocation invariant). */
const midpointScratch: { x: number; y: number; ok: boolean } = {
  x: 0,
  y: 0,
  ok: false,
};

/** Resolve the midpoint of `pathId` into `midpointScratch`. Returns
 *  the same scratch instance every call; the `ok` field signals
 *  whether the path was found. NEVER allocates. */
function findPathMidpoint(
  scene: StaticPayload,
  pathId: number
): { x: number; y: number; ok: boolean } {
  midpointScratch.ok = false;
  for (const p of scene.paths) {
    if (p.id !== pathId) continue;
    midpointScratch.x = (p.from_pos[0] + p.to_pos[0]) / 2;
    midpointScratch.y = (p.from_pos[1] + p.to_pos[1]) / 2;
    midpointScratch.ok = true;
    return midpointScratch;
  }
  return midpointScratch;
}

// ──────────────── render functions ──────────────────────────────

const drawDepartFlare: RenderFn = (ctx, t, payload, resolve) => {
  if (payload.kind !== "mover_departed") return;
  const node = findNode(resolve.scene, payload.from_node);
  if (node === undefined) return;
  const radius = 22 + 18 * t;
  const alpha = 1 - t;
  ctx.save();
  ctx.globalAlpha = alpha * 0.7;
  ctx.strokeStyle = paletteColor(resolve.theme, node.color);
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.arc(node.pos[0], node.pos[1], radius, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();
};

const drawArriveRing: RenderFn = (ctx, t, payload, resolve) => {
  if (payload.kind !== "mover_arrived") return;
  const node = findNode(resolve.scene, payload.at_node);
  if (node === undefined) return;
  const radius = 18 + 24 * t;
  const alpha = 1 - t;
  ctx.save();
  ctx.globalAlpha = alpha;
  ctx.strokeStyle = foregroundColor(resolve.theme);
  ctx.lineWidth = 3 * (1 - t * 0.5);
  ctx.beginPath();
  ctx.arc(node.pos[0], node.pos[1], radius, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();
};

const drawSpeedHint: RenderFn = (ctx, t, payload, resolve) => {
  if (payload.kind !== "mover_speed_change") return;
  const m = findMover(resolve.snapshot, payload.mover);
  if (m === undefined) return;
  const alpha = 1 - t;
  const r = 12 + 6 * t;
  ctx.save();
  ctx.globalAlpha = alpha * 0.8;
  ctx.strokeStyle = payload.new > payload.old ? "#9ece6a" : "#e0af68";
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.arc(m.pos[0], m.pos[1], r, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();
};

const drawNodePulse: RenderFn = (ctx, t, payload, resolve) => {
  if (payload.kind !== "node_highlighted") return;
  const node = findNode(resolve.scene, payload.node);
  if (node === undefined) return;
  const pulse = Math.sin(t * Math.PI);
  const r = 20 + 12 * pulse;
  ctx.save();
  ctx.globalAlpha = pulse * 0.6;
  ctx.strokeStyle = foregroundColor(resolve.theme);
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.arc(node.pos[0], node.pos[1], r, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();
};

const drawPathPulse: RenderFn = (ctx, t, payload, resolve) => {
  if (payload.kind !== "path_pulsed") return;
  const mid = findPathMidpoint(resolve.scene, payload.path);
  if (!mid.ok) return;
  const alpha = 1 - t;
  const r = 8 + 22 * t;
  ctx.save();
  ctx.globalAlpha = alpha * 0.5;
  ctx.fillStyle = foregroundColor(resolve.theme);
  ctx.beginPath();
  ctx.arc(mid.x, mid.y, r, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
};

const drawDecisionPulse: RenderFn = (ctx, t, payload, resolve) => {
  if (payload.kind !== "agent_decided") return;
  // Small pulse anchored top-left; inspector (Inspector) replaces this
  // with a pulse on the affected piece once Action carries it.
  const alpha = 1 - t;
  ctx.save();
  ctx.globalAlpha = alpha;
  ctx.fillStyle = foregroundColor(resolve.theme);
  ctx.beginPath();
  ctx.arc(24, 24, 6 + 4 * (1 - t), 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
};

const noopRender: RenderFn = () => {};

// ──────────────── the binding table ─────────────────────────────

export const animations: Record<SimEventKind, AnimationSpec> = {
  mover_departed: { durationMs: 200, ease: easings.easeOutCubic, render: drawDepartFlare },
  mover_arrived: { durationMs: 300, ease: easings.easeInOutQuad, render: drawArriveRing },
  mover_speed_change: { durationMs: 150, ease: easings.easeOutCubic, render: drawSpeedHint },
  node_highlighted: { durationMs: 600, ease: easings.easeOutCubic, render: drawNodePulse },
  path_pulsed: { durationMs: 400, ease: easings.easeInOutCubic, render: drawPathPulse },
  agent_decided: { durationMs: 250, ease: easings.easeOutQuad, render: drawDecisionPulse },
  tick: { durationMs: 0, ease: easings.linear, render: noopRender },
  // scenario_language_v1 events are surfaced via the HUD (chips, panels)
  // rather than canvas-overlay animations. No canvas pulse needed today.
  sl1_pressure_lifecycle: { durationMs: 0, ease: easings.linear, render: noopRender },
  sl1_objective_state_changed: { durationMs: 0, ease: easings.linear, render: noopRender },
  sl1_failure_condition_fired: { durationMs: 0, ease: easings.linear, render: noopRender },
  sl1_victory_condition_met: { durationMs: 0, ease: easings.linear, render: noopRender },
  sl1_game_outcome_changed: { durationMs: 0, ease: easings.linear, render: noopRender },
  sl1_dashboard_state_changed: { durationMs: 0, ease: easings.linear, render: noopRender },
  sl1_alert_fired: { durationMs: 0, ease: easings.linear, render: noopRender },
  sl1_alert_cleared: { durationMs: 0, ease: easings.linear, render: noopRender },
  sl1_agent_action_applied: { durationMs: 0, ease: easings.linear, render: noopRender },
  sl1_agent_action_rejected: { durationMs: 0, ease: easings.linear, render: noopRender },
  sl1_agent_llm_disabled: { durationMs: 0, ease: easings.linear, render: noopRender },
  sl1_milestone_fired: { durationMs: 0, ease: easings.linear, render: noopRender },
};
