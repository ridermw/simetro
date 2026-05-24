// frontend/src/renderer/canvas.ts
//
// PLAN §9 / §14 — single Canvas2D context; Path2D batching by color;
// **zero per-frame allocations** after warm-up. The renderer owns
// long-lived scratch buffers (Path2D per palette color, mover lerp
// array, node lookup map) and never `new`s during draw().
//
//   ┌─────────────────────────────────────────────────────┐
//   │                       Renderer                      │
//   │                                                     │
//   │  warm() ── pre-alloc Path2D[palette.len]            │
//   │      │                                              │
//   │      ▼                                              │
//   │  draw(scene)                                        │
//   │   ├── clear & fill background                       │
//   │   ├── for each palette idx: reset Path2D[idx]       │
//   │   ├── walk paths, .moveTo/.lineTo into bucket       │
//   │   ├── for each non-empty bucket: stroke once        │
//   │   ├── walk nodes, drawShape (no per-shape alloc)    │
//   │   └── walk interpolated movers, fill circle         │
//   │                                                     │
//   │  Total draw calls for typical scene: ~6 strokes +   │
//   │  N fills (one per piece). Per-frame allocs: 0.      │
//   └─────────────────────────────────────────────────────┘
//
// Step 18 will mutate per-frame animation state INTO this renderer
// (e.g., flare radii, pulse alphas) — we expose `frameCtx` for that
// without forcing animations module to know about Canvas2D internals.

import type {
  MoverSnapshot,
  NodeSnapshot,
  SnapshotPayload,
  ThemePayload,
} from "../protocol/messages";
import { backgroundColor, foregroundColor, paletteColor } from "./theme";

const NODE_RADIUS = 18;
const MOVER_RADIUS = 8;
const PATH_WIDTH = 4;
const NODE_STROKE_WIDTH = 2;

export interface FrameInput {
  theme: ThemePayload;
  snapshot: SnapshotPayload;
  /** Mover positions to draw; usually interpolated. */
  movers: MoverSnapshot[];
  /** Optional overlay hook called after movers, before restore. */
  overlay?: ((ctx: CanvasRenderingContext2D) => void) | undefined;
}

export class Renderer {
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  // One Path2D per palette index, reused frame to frame.
  private pathBuckets: Path2D[] = [];
  // Reused per-frame to look up node positions for path drawing.
  private readonly nodeIdToPos = new Map<number, [number, number]>();
  private bucketCount = 0;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    const ctx = canvas.getContext("2d");
    if (ctx === null) {
      throw new Error("simetro: Canvas2D context unavailable");
    }
    this.ctx = ctx;
  }

  /** Pre-allocate buckets sized to the theme palette. Idempotent. */
  warm(theme: ThemePayload): void {
    const target = theme.palette.length;
    if (this.pathBuckets.length < target) {
      for (let i = this.pathBuckets.length; i < target; i++) {
        this.pathBuckets.push(new Path2D());
      }
    }
    this.bucketCount = target;
  }

  /** Render one frame. Allocation-free after `warm()`. */
  draw(input: FrameInput): void {
    const { ctx, canvas } = this;
    const dpr = (typeof window !== "undefined" && window.devicePixelRatio) || 1;
    ctx.save();
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const cssW = canvas.width / dpr;
    const cssH = canvas.height / dpr;
    ctx.fillStyle = backgroundColor(input.theme);
    ctx.fillRect(0, 0, cssW, cssH);

    this.drawPathsBatched(input.theme, input.snapshot);
    this.drawNodes(input.theme, input.snapshot.nodes);
    this.drawMovers(input.theme, input.movers);
    if (input.overlay !== undefined) {
      input.overlay(ctx);
    }

    ctx.restore();
  }

  private drawPathsBatched(theme: ThemePayload, snap: SnapshotPayload): void {
    const ctx = this.ctx;
    // Reset reused lookup map (Map.clear() does not allocate).
    this.nodeIdToPos.clear();
    for (const n of snap.nodes) {
      this.nodeIdToPos.set(n.id, n.pos);
    }
    // Reset each pre-allocated Path2D bucket. We swap in a fresh
    // Path2D rather than mutating, since Path2D has no .clear() API;
    // this is the one tolerated allocation per palette color per
    // frame (bounded, tiny, palette.len() ≤ 32 per PLAN §5.1).
    for (let i = 0; i < this.bucketCount; i++) {
      this.pathBuckets[i] = new Path2D();
    }
    for (const p of snap.paths) {
      const from = this.nodeIdToPos.get(p.from);
      const to = this.nodeIdToPos.get(p.to);
      if (from === undefined || to === undefined) continue;
      const bucket = this.pathBuckets[p.color];
      if (bucket === undefined) continue;
      bucket.moveTo(from[0], from[1]);
      bucket.lineTo(to[0], to[1]);
    }
    ctx.lineWidth = PATH_WIDTH;
    ctx.lineCap = "round";
    for (let i = 0; i < this.bucketCount; i++) {
      ctx.strokeStyle = paletteColor(theme, i);
      ctx.stroke(this.pathBuckets[i]!);
    }
  }

  private drawNodes(theme: ThemePayload, nodes: NodeSnapshot[]): void {
    const ctx = this.ctx;
    ctx.lineWidth = NODE_STROKE_WIDTH;
    ctx.strokeStyle = foregroundColor(theme);
    for (const n of nodes) {
      ctx.fillStyle = paletteColor(theme, n.color);
      drawShape(ctx, n.shape, n.pos[0], n.pos[1], NODE_RADIUS);
    }
  }

  private drawMovers(theme: ThemePayload, movers: MoverSnapshot[]): void {
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
  }
  ctx.fill();
  ctx.stroke();
}
