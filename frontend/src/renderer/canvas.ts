// frontend/src/renderer/canvas.ts
//
// PLAN §9 / §14 — single Canvas2D context; Path2D batching by color;
// **zero per-frame allocations** after warm-up. The renderer owns
// long-lived scratch buffers (Path2D per palette color, mover lerp
// array) and never `new`s during draw().
//
//   ┌─────────────────────────────────────────────────────┐
//   │                       Renderer                      │
//   │                                                     │
//   │  warm(theme)         ── pre-alloc Path2D[palette]   │
//   │  setScene(static)    ── refill buckets ONCE         │
//   │      │                                              │
//   │      ▼                                              │
//   │  draw(scene, snap, movers)                          │
//   │   ├── clear & fill background                       │
//   │   ├── for each active bucket: stroke once           │
//   │   ├── walk scene.nodes, drawShape                   │
//   │   └── walk interpolated movers, fill circle         │
//   │                                                     │
//   │  Total draw calls for typical scene: ~6 strokes +   │
//   │  N fills (one per piece). Per-frame allocs: 0.      │
//   └─────────────────────────────────────────────────────┘
//
// Per review feedback on PR #1 (Copilot, P1): paths don't move, so
// Path2D buckets are rebuilt only when the scene identity changes —
// not per frame. `activeBuckets` tracks which palette indices have
// segments, so we never stroke empty buckets.

import type { MoverState, NodeView, StaticPayload } from "../protocol/messages";
import { backgroundColor, foregroundColor, paletteColor, type Theme } from "./theme";

const NODE_RADIUS = 18;
const MOVER_RADIUS = 8;
const PATH_WIDTH = 4;
const NODE_STROKE_WIDTH = 2;

export interface FrameInput {
  theme: Theme;
  /** Static scene metadata (palette + geometry); supplies node draws. */
  scene: StaticPayload;
  /** Mover positions to draw; usually interpolated. */
  movers: MoverState[];
  /** Optional overlay hook called after movers, before restore. */
  overlay?: ((ctx: CanvasRenderingContext2D) => void) | undefined;
}

export class Renderer {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  // One Path2D per palette index, reused across frames. Refilled in
  // setScene(), not in draw().
  private pathBuckets: Path2D[] = [];
  /** True at index i iff palette color i has at least one segment.
   *  We stroke only active buckets — saves per-frame work and gets
   *  the zero-alloc test past empty palettes (Copilot review). */
  private activeBuckets: boolean[] = [];
  private bucketCount = 0;
  /** Identity of the StaticPayload we last absorbed; lets the rAF
   *  loop call setScene() unconditionally without rebuild cost. */
  private currentScene: StaticPayload | null = null;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    const ctx = canvas.getContext("2d");
    if (ctx === null) {
      throw new Error("simetro: Canvas2D context unavailable");
    }
    this.ctx = ctx;
  }

  /** Pre-allocate buckets sized to the theme palette. Idempotent. */
  warm(theme: Theme): void {
    const target = theme.palette.length;
    while (this.pathBuckets.length < target) {
      this.pathBuckets.push(new Path2D());
      this.activeBuckets.push(false);
    }
    this.bucketCount = target;
    // Buckets may carry stale geometry from a previous scene; clear.
    for (let i = 0; i < this.bucketCount; i++) {
      this.pathBuckets[i] = new Path2D();
      this.activeBuckets[i] = false;
    }
    this.currentScene = null;
  }

  /** Rebuild path buckets from the new static scene. Idempotent;
   *  a no-op when called with the same object identity. */
  setScene(scene: StaticPayload): void {
    if (this.currentScene === scene) return;
    // Ensure buckets large enough for the scene palette.
    const target = scene.palette.length;
    while (this.pathBuckets.length < target) {
      this.pathBuckets.push(new Path2D());
      this.activeBuckets.push(false);
    }
    this.bucketCount = Math.max(this.bucketCount, target);
    // Reset all in-range buckets.
    for (let i = 0; i < this.bucketCount; i++) {
      this.pathBuckets[i] = new Path2D();
      this.activeBuckets[i] = false;
    }
    // Fill from baked PathView endpoints (no node lookup needed —
    // protocol bakes positions into the path view for exactly this).
    for (const p of scene.paths) {
      if (p.color < 0 || p.color >= this.bucketCount) continue;
      const bucket = this.pathBuckets[p.color];
      if (bucket === undefined) continue;
      bucket.moveTo(p.from_pos[0], p.from_pos[1]);
      bucket.lineTo(p.to_pos[0], p.to_pos[1]);
      this.activeBuckets[p.color] = true;
    }
    this.currentScene = scene;
  }

  /** Render one frame. Allocation-free after `setScene()`. */
  draw(input: FrameInput): void {
    const { ctx, canvas } = this;
    // setScene is cheap when identity is unchanged; lets callers
    // pass the latest static every frame without bookkeeping.
    this.setScene(input.scene);

    const dpr = (typeof window !== "undefined" && window.devicePixelRatio) || 1;
    ctx.save();
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const cssW = canvas.width / dpr;
    const cssH = canvas.height / dpr;
    ctx.fillStyle = backgroundColor(input.theme);
    ctx.fillRect(0, 0, cssW, cssH);

    this.drawPathsBatched(input.theme);
    this.drawNodes(input.theme, input.scene.nodes);
    this.drawMovers(input.theme, input.movers);
    if (input.overlay !== undefined) {
      input.overlay(ctx);
    }

    ctx.restore();
  }

  private drawPathsBatched(theme: Theme): void {
    const ctx = this.ctx;
    ctx.lineWidth = PATH_WIDTH;
    ctx.lineCap = "round";
    for (let i = 0; i < this.bucketCount; i++) {
      if (!this.activeBuckets[i]) continue;
      const bucket = this.pathBuckets[i];
      if (bucket === undefined) continue;
      ctx.strokeStyle = paletteColor(theme, i);
      ctx.stroke(bucket);
    }
  }

  private drawNodes(theme: Theme, nodes: NodeView[]): void {
    const ctx = this.ctx;
    ctx.lineWidth = NODE_STROKE_WIDTH;
    ctx.strokeStyle = foregroundColor(theme);
    for (const n of nodes) {
      ctx.fillStyle = paletteColor(theme, n.color);
      drawShape(ctx, n.shape, n.pos[0], n.pos[1], NODE_RADIUS);
    }
  }

  private drawMovers(theme: Theme, movers: MoverState[]): void {
    const ctx = this.ctx;
    ctx.fillStyle = foregroundColor(theme);
    for (const m of movers) {
      ctx.beginPath();
      ctx.arc(m.pos[0], m.pos[1], MOVER_RADIUS, 0, Math.PI * 2);
      ctx.fill();
    }
  }
}

function drawShape(
  ctx: CanvasRenderingContext2D,
  shape: NodeView["shape"],
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
      ctx.lineTo(x + r * 0.866, y + r * 0.5);
      ctx.lineTo(x - r * 0.866, y + r * 0.5);
      ctx.closePath();
      break;
    case "diamond":
      ctx.moveTo(x, y - r);
      ctx.lineTo(x + r, y);
      ctx.lineTo(x, y + r);
      ctx.lineTo(x - r, y);
      ctx.closePath();
      break;
    case "hexagon": {
      const a = Math.PI / 3;
      ctx.moveTo(x + r, y);
      for (let i = 1; i < 6; i++) {
        ctx.lineTo(x + r * Math.cos(i * a), y + r * Math.sin(i * a));
      }
      ctx.closePath();
      break;
    }
  }
  ctx.fill();
  ctx.stroke();
}
