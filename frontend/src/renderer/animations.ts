// frontend/src/renderer/animations.ts
//
// ┌──────────────────────────────────────────────────────────────┐
// │       PLAN §9 / §20 DoD #5: THE HMR TARGET FILE              │
// │                                                              │
// │ Edit this file to retune juice. Vite's HMR boundary lives    │
// │ on this module so saving here patches running animations in  │
// │ <300ms without losing sim state. (Measured in Step 22.)      │
// │                                                              │
// │   SimEvent (one of MoverDeparted / Arrived / …)              │
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
// They MUST NOT allocate. ctx state save/restore happens around
// the per-event invocation in the engine, so a render fn is free
// to mutate strokeStyle/lineWidth/globalAlpha.

import type { NodeSnapshot, SimEvent, SnapshotPayload, ThemePayload } from "../protocol/messages";
import { easings, foregroundColor, paletteColor } from "./theme";

export type SimEventTag = SimEvent["tag"];

export interface ResolveCtx {
  theme: ThemePayload;
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

function findNode(snap: SnapshotPayload, id: number): NodeSnapshot | undefined {
  for (const n of snap.nodes) if (n.id === id) return n;
  return undefined;
}

function findPathMidpoint(
  snap: SnapshotPayload,
  pathId: number
): { x: number; y: number } | null {
  for (const p of snap.paths) {
    if (p.id !== pathId) continue;
    const from = findNode(snap, p.from);
    const to = findNode(snap, p.to);
    if (from === undefined || to === undefined) return null;
    return { x: (from.pos[0] + to.pos[0]) / 2, y: (from.pos[1] + to.pos[1]) / 2 };
  }
  return null;
}

// ──────────────── render functions ──────────────────────────────

const drawDepartFlare: RenderFn = (ctx, t, payload, resolve) => {
  if (payload.tag !== "MoverDeparted") return;
  const node = findNode(resolve.snapshot, payload.from_node);
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
  if (payload.tag !== "MoverArrived") return;
  const node = findNode(resolve.snapshot, payload.at_node);
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
  if (payload.tag !== "MoverSpeedChange") return;
  // Find the mover position via the latest snapshot.
  let mx = 0,
    my = 0,
    found = false;
  for (const m of resolve.snapshot.movers) {
    if (m.id === payload.mover) {
      mx = m.pos[0];
      my = m.pos[1];
      found = true;
      break;
    }
  }
  if (!found) return;
  const alpha = 1 - t;
  const r = 12 + 6 * t;
  ctx.save();
  ctx.globalAlpha = alpha * 0.8;
  ctx.strokeStyle = payload.new > payload.old ? "#9ece6a" : "#e0af68";
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.arc(mx, my, r, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();
};

const drawNodePulse: RenderFn = (ctx, t, payload, resolve) => {
  if (payload.tag !== "NodeHighlighted") return;
  const node = findNode(resolve.snapshot, payload.node);
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
  if (payload.tag !== "PathPulsed") return;
  const mid = findPathMidpoint(resolve.snapshot, payload.path);
  if (mid === null) return;
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
  if (payload.tag !== "AgentDecided") return;
  // Use a small pulse anchored top-left for now; Step 20 (Inspector)
  // will replace this with a pulse on the affected piece.
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

export const animations: Record<SimEventTag, AnimationSpec> = {
  MoverDeparted: { durationMs: 200, ease: easings.easeOutCubic, render: drawDepartFlare },
  MoverArrived: { durationMs: 300, ease: easings.easeInOutQuad, render: drawArriveRing },
  MoverSpeedChange: { durationMs: 150, ease: easings.easeOutCubic, render: drawSpeedHint },
  NodeHighlighted: { durationMs: 600, ease: easings.easeOutCubic, render: drawNodePulse },
  PathPulsed: { durationMs: 400, ease: easings.easeInOutCubic, render: drawPathPulse },
  AgentDecided: { durationMs: 250, ease: easings.easeOutQuad, render: drawDecisionPulse },
  Tick: { durationMs: 0, ease: easings.linear, render: noopRender },
};
