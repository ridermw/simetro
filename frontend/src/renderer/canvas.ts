// frontend/src/renderer/canvas.ts
//
// PLAN §9 / §14 — Single Canvas2D context. Path2D batching by color
// is the central perf trick: instead of one stroke() per path, we
// build one Path2D per palette index, then issue one stroke per
// color. With a 5-color palette that's ~6 draw calls for any number
// of paths in the scene.
//
// Step 16 ships the static-frame entry-point. Step 17 fleshes out
// real per-color batching and the dark-theme geometric primitives.
// Step 18 layers animations on top by mutating a position cache
// between frames — this module stays pure (state in, pixels out).

import type {
  MoverSnapshot,
  NodeSnapshot,
  PathSnapshot,
  SnapshotPayload,
  ThemePayload,
} from "../protocol/messages";

export interface SceneState {
  theme: ThemePayload | null;
  snapshot: SnapshotPayload | null;
}

const NODE_RADIUS = 18;
const MOVER_RADIUS = 8;
const PATH_WIDTH = 4;

export function renderStaticFrame(
  canvas: HTMLCanvasElement,
  scene: SceneState
): void {
  const ctx = canvas.getContext("2d");
  if (ctx === null || scene.theme === null || scene.snapshot === null) {
    return;
  }
  const dpr = window.devicePixelRatio || 1;
  const width = canvas.width;
  const height = canvas.height;

  ctx.save();
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

  const bgColor =
    scene.theme.palette[scene.theme.background_index] ?? "#0e1116";
  ctx.fillStyle = bgColor;
  ctx.fillRect(0, 0, width / dpr, height / dpr);

  drawPathsBatched(ctx, scene.snapshot.paths, scene.snapshot.nodes, scene.theme);
  drawNodes(ctx, scene.snapshot.nodes, scene.theme);
  drawMovers(ctx, scene.snapshot.movers, scene.theme);

  ctx.restore();
}

// PLAN §9: bucket paths by color, build one Path2D per bucket, stroke
// once per color. O(paths) build, O(colors) draw.
function drawPathsBatched(
  ctx: CanvasRenderingContext2D,
  paths: PathSnapshot[],
  nodes: NodeSnapshot[],
  theme: ThemePayload
): void {
  const nodeById = new Map<number, NodeSnapshot>();
  for (const n of nodes) nodeById.set(n.id, n);

  const byColor = new Map<number, Path2D>();
  for (const p of paths) {
    const from = nodeById.get(p.from);
    const to = nodeById.get(p.to);
    if (from === undefined || to === undefined) continue;
    let path = byColor.get(p.color);
    if (path === undefined) {
      path = new Path2D();
      byColor.set(p.color, path);
    }
    path.moveTo(from.pos[0], from.pos[1]);
    path.lineTo(to.pos[0], to.pos[1]);
  }

  ctx.lineWidth = PATH_WIDTH;
  ctx.lineCap = "round";
  for (const [colorIndex, path2d] of byColor) {
    ctx.strokeStyle = colorOf(theme, colorIndex);
    ctx.stroke(path2d);
  }
}

function drawNodes(
  ctx: CanvasRenderingContext2D,
  nodes: NodeSnapshot[],
  theme: ThemePayload
): void {
  for (const n of nodes) {
    ctx.fillStyle = colorOf(theme, n.color);
    ctx.strokeStyle = theme.palette[1] ?? "#e8eaed";
    ctx.lineWidth = 2;
    drawShape(ctx, n.shape, n.pos[0], n.pos[1], NODE_RADIUS);
  }
}

function drawMovers(
  ctx: CanvasRenderingContext2D,
  movers: MoverSnapshot[],
  theme: ThemePayload
): void {
  ctx.fillStyle = theme.palette[1] ?? "#e8eaed";
  ctx.strokeStyle = "transparent";
  for (const m of movers) {
    ctx.beginPath();
    ctx.arc(m.pos[0], m.pos[1], MOVER_RADIUS, 0, Math.PI * 2);
    ctx.fill();
  }
}

function drawShape(
  ctx: CanvasRenderingContext2D,
  shape: NodeSnapshot["shape"],
  x: number,
  y: number,
  r: number
): void {
  ctx.beginPath();
  switch (shape) {
    case "circle":
      ctx.arc(x, y, r, 0, Math.PI * 2);
      break;
    case "square":
      ctx.rect(x - r, y - r, r * 2, r * 2);
      break;
    case "triangle":
      ctx.moveTo(x, y - r);
      ctx.lineTo(x + r, y + r);
      ctx.lineTo(x - r, y + r);
      ctx.closePath();
      break;
    case "diamond":
      ctx.moveTo(x, y - r);
      ctx.lineTo(x + r, y);
      ctx.lineTo(x, y + r);
      ctx.lineTo(x - r, y);
      ctx.closePath();
      break;
  }
  ctx.fill();
  ctx.stroke();
}

function colorOf(theme: ThemePayload, index: number): string {
  return theme.palette[index] ?? "#e8eaed";
}
