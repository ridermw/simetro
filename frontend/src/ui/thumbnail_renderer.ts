// frontend/src/ui/thumbnail_renderer.ts
//
// Renders a StaticPayload into a mini offscreen canvas for gallery thumbnails.
// Reuses the same drawing approach as the main Renderer but at thumbnail scale.
//
//  StaticPayload ──▶ bounding-box fit ──▶ paths ──▶ nodes ──▶ canvas

import type { StaticPayload } from "../protocol/messages";

const THUMB_NODE_RADIUS = 6;
const THUMB_PATH_WIDTH = 2;

export function renderThumbnail(
  payload: StaticPayload,
  width: number,
  height: number
): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (ctx === null) {
    throw new Error("Canvas2D unavailable for thumbnail");
  }

  // Background.
  const bgColor = payload.palette[payload.background_index] ?? "#0e1116";
  ctx.fillStyle = bgColor;
  ctx.fillRect(0, 0, width, height);

  // Compute bounding box of all geometry to fit into thumbnail.
  const positions: [number, number][] = [];
  for (const n of payload.nodes) positions.push(n.pos);
  for (const p of payload.paths) {
    positions.push(p.from_pos);
    positions.push(p.to_pos);
  }

  if (positions.length === 0) return canvas;

  let minX = Infinity,
    minY = Infinity,
    maxX = -Infinity,
    maxY = -Infinity;
  for (const [x, y] of positions) {
    if (x < minX) minX = x;
    if (y < minY) minY = y;
    if (x > maxX) maxX = x;
    if (y > maxY) maxY = y;
  }

  const padding = 20;
  const sceneW = maxX - minX || 1;
  const sceneH = maxY - minY || 1;
  const scale = Math.min(
    (width - padding * 2) / sceneW,
    (height - padding * 2) / sceneH
  );
  const offsetX = (width - sceneW * scale) / 2 - minX * scale;
  const offsetY = (height - sceneH * scale) / 2 - minY * scale;

  function tx(x: number): number {
    return x * scale + offsetX;
  }
  function ty(y: number): number {
    return y * scale + offsetY;
  }

  // Draw paths.
  ctx.lineWidth = THUMB_PATH_WIDTH;
  for (const p of payload.paths) {
    ctx.strokeStyle = payload.palette[p.color] ?? "#555";
    ctx.beginPath();
    ctx.moveTo(tx(p.from_pos[0]), ty(p.from_pos[1]));
    ctx.lineTo(tx(p.to_pos[0]), ty(p.to_pos[1]));
    ctx.stroke();
  }

  // Draw nodes.
  for (const n of payload.nodes) {
    ctx.fillStyle = payload.palette[n.color] ?? "#aaa";
    ctx.beginPath();
    ctx.arc(tx(n.pos[0]), ty(n.pos[1]), THUMB_NODE_RADIUS, 0, Math.PI * 2);
    ctx.fill();
  }

  return canvas;
}

/** Create a palette swatch fallback when static payload loading fails. */
export function renderPaletteSwatch(
  palette: string[],
  width: number,
  height: number
): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (ctx === null) return canvas;

  const bgColor = palette[0] ?? "#0e1116";
  ctx.fillStyle = bgColor;
  ctx.fillRect(0, 0, width, height);

  // Draw palette colors as horizontal stripes.
  const stripeH = height / Math.max(palette.length, 1);
  for (let i = 1; i < palette.length; i++) {
    const color = palette[i];
    if (color === undefined) continue;
    ctx.fillStyle = color;
    ctx.fillRect(width * 0.2, stripeH * i, width * 0.6, stripeH * 0.6);
  }

  return canvas;
}
