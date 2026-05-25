// frontend/src/renderer/canvas.ts
//
// renderer batching and allocation target — single Canvas2D context; Path2D batching by color;
// **zero per-frame allocations** after warm-up. The renderer owns
// long-lived scratch buffers (Path2D per palette color, mover lerp
// array) and never `new`s during draw().
//
//   ┌─────────────────────────────────────────────────────┐
//   │                       Renderer                      │
//   │                                                     │
//   │  warm(theme)         ── pre-alloc Path2D[palette]   │
//   │  setScene(static)    ── refill buckets ONCE;        │
//   │      │                   auto-fit viewport          │
//   │      ▼                                              │
//   │  draw(scene, snap, movers)                          │
//   │   ├── clear & fill background (screen space)        │
//   │   ├── apply viewport transform                      │
//   │   ├── for each active bucket: stroke once           │
//   │   ├── walk scene.nodes, drawShape                   │
//   │   └── walk interpolated movers, fill circle         │
//   │                                                     │
//   │  Total draw calls for typical scene: ~6 strokes +   │
//   │  N fills (one per piece). Per-frame allocs: 0.      │
//   └─────────────────────────────────────────────────────┘
//
// Review feedback: paths don't move, so
// Path2D buckets are rebuilt only when the scene identity changes —
// not per frame. `activeBuckets` tracks which palette indices have
// segments, so we never stroke empty buckets.

import type { MoverState, NodeView, StaticPayload } from "../protocol/messages";
import { backgroundColor, foregroundColor, paletteColor, type Theme } from "./theme";

const NODE_RADIUS = 18;
const MOVER_RADIUS = 8;
const PATH_WIDTH = 4;
const NODE_STROKE_WIDTH = 2;
const FIT_PADDING = NODE_RADIUS + 20;
const MIN_SCALE = 0.15;
const MAX_SCALE = 8;

interface Viewport {
  scale: number;
  offsetX: number;
  offsetY: number;
}

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

  // --- Viewport state ---
  // Current pan/zoom transform: screen = world * scale + offset.
  private viewport: Viewport = { scale: 1, offsetX: 0, offsetY: 0 };
  // The fit transform computed at the last setScene(); resetViewport() returns here.
  private fitViewport: Viewport = { scale: 1, offsetX: 0, offsetY: 0 };
  // World-space bounding box from the last setScene().
  private worldMinX = 0;
  private worldMinY = 0;
  private worldMaxX = 0;
  private worldMaxY = 0;
  private hasWorldBounds = false;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    const ctx = canvas.getContext("2d");
    if (ctx === null) {
      throw new Error("simetro: Canvas2D context unavailable");
    }
    this.ctx = ctx;
  }

  /** Exposed for renderer unit tests only. */
  get viewportForTest(): Readonly<Viewport> {
    return this.viewport;
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
   *  a no-op when called with the same object identity. Auto-fits
   *  the viewport to the scene geometry on identity change. */
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

    // Compute world bounding box from nodes and path endpoints.
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const n of scene.nodes) {
      if (n.pos[0] < minX) minX = n.pos[0];
      if (n.pos[1] < minY) minY = n.pos[1];
      if (n.pos[0] > maxX) maxX = n.pos[0];
      if (n.pos[1] > maxY) maxY = n.pos[1];
    }
    for (const p of scene.paths) {
      for (const pt of [p.from_pos, p.to_pos]) {
        if (pt[0] < minX) minX = pt[0];
        if (pt[1] < minY) minY = pt[1];
        if (pt[0] > maxX) maxX = pt[0];
        if (pt[1] > maxY) maxY = pt[1];
      }
    }
    if (minX !== Infinity) {
      this.worldMinX = minX;
      this.worldMinY = minY;
      this.worldMaxX = maxX;
      this.worldMaxY = maxY;
      this.hasWorldBounds = true;
    } else {
      this.hasWorldBounds = false;
    }

    const fit = this.computeFit();
    this.fitViewport = fit;
    this.viewport = { ...fit };

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
    // Background fill in screen space (no viewport transform).
    ctx.fillStyle = backgroundColor(input.theme);
    ctx.fillRect(0, 0, cssW, cssH);

    // Apply viewport transform for all world-space drawing.
    ctx.save();
    ctx.translate(this.viewport.offsetX, this.viewport.offsetY);
    ctx.scale(this.viewport.scale, this.viewport.scale);

    this.drawPathsBatched(input.theme);
    this.drawNodes(input.theme, input.scene.nodes);
    this.drawMovers(input.theme, input.movers);
    if (input.overlay !== undefined) {
      input.overlay(ctx);
    }

    ctx.restore();
    ctx.restore();
  }

  /** Pan the viewport by (dx, dy) in CSS/screen pixels. */
  panBy(dx: number, dy: number): void {
    this.viewport.offsetX += dx;
    this.viewport.offsetY += dy;
  }

  /**
   * Zoom by `factor` keeping the world point under (screenX, screenY) stable.
   * Scale is clamped to [MIN_SCALE, MAX_SCALE].
   */
  zoomAt(screenX: number, screenY: number, factor: number): void {
    const newScale = Math.max(MIN_SCALE, Math.min(MAX_SCALE, this.viewport.scale * factor));
    const scaleRatio = newScale / this.viewport.scale;
    // Anchor: screenPt = worldPt * scale + offset  →  offset' = screenPt - worldPt * newScale
    //       = screenPt * (1 - scaleRatio) + offset * scaleRatio
    this.viewport.offsetX = screenX * (1 - scaleRatio) + this.viewport.offsetX * scaleRatio;
    this.viewport.offsetY = screenY * (1 - scaleRatio) + this.viewport.offsetY * scaleRatio;
    this.viewport.scale = newScale;
  }

  /** Reset to the auto-fit viewport computed at the last setScene(). */
  resetViewport(): void {
    this.viewport = { ...this.fitViewport };
  }

  /**
   * Wire pointer drag, wheel zoom, and double-click reset to the canvas.
   * Call once after construction in the app boot sequence.
   */
  attachViewportControls(): void {
    const canvas = this.canvas;
    let isDragging = false;
    let lastX = 0;
    let lastY = 0;

    canvas.addEventListener("pointerdown", (e) => {
      isDragging = true;
      lastX = e.clientX;
      lastY = e.clientY;
      if (canvas.setPointerCapture !== undefined) {
        canvas.setPointerCapture(e.pointerId);
      }
    });

    canvas.addEventListener("pointermove", (e) => {
      if (!isDragging) return;
      const dx = e.clientX - lastX;
      const dy = e.clientY - lastY;
      lastX = e.clientX;
      lastY = e.clientY;
      this.panBy(dx, dy);
    });

    const endDrag = (): void => {
      isDragging = false;
    };
    canvas.addEventListener("pointerup", endDrag);
    canvas.addEventListener("pointercancel", endDrag);

    canvas.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        const rect = canvas.getBoundingClientRect();
        const screenX = e.clientX - rect.left;
        const screenY = e.clientY - rect.top;
        // Each wheel tick zooms ~10%; deltaY > 0 → scroll down → zoom out.
        const factor = e.deltaY < 0 ? 1.1 : 1 / 1.1;
        this.zoomAt(screenX, screenY, factor);
      },
      { passive: false }
    );

    canvas.addEventListener("dblclick", () => {
      this.resetViewport();
    });
  }

  // --- Private helpers ---

  /** Compute the fit viewport from the stored world bounds and current canvas size. */
  private computeFit(): Viewport {
    if (!this.hasWorldBounds) {
      return { scale: 1, offsetX: 0, offsetY: 0 };
    }
    const dpr = (typeof window !== "undefined" && window.devicePixelRatio) || 1;
    const cssW = this.canvas.width / dpr;
    const cssH = this.canvas.height / dpr;

    const worldW = this.worldMaxX - this.worldMinX;
    const worldH = this.worldMaxY - this.worldMinY;

    if (worldW <= 0 || worldH <= 0) {
      return { scale: 1, offsetX: 0, offsetY: 0 };
    }

    const availW = cssW - FIT_PADDING * 2;
    const availH = cssH - FIT_PADDING * 2;
    const scale = Math.min(availW / worldW, availH / worldH);

    const offsetX = cssW / 2 - (this.worldMinX + worldW / 2) * scale;
    const offsetY = cssH / 2 - (this.worldMinY + worldH / 2) * scale;

    return { scale, offsetX, offsetY };
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
