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

import type { MoverState, NodeView, PathView, StaticPayload } from "../protocol/messages";
import { backgroundColor, foregroundColor, paletteColor, type Theme } from "./theme";

const NODE_RADIUS = 18;
const MOVER_RADIUS = 8;
const FLOW_PARTICLE_RADIUS = 4;
const FLOW_PARTICLE_SPEED = 0.25;
const FLOW_PARTICLE_PHASE_OFFSET = 0.37;
const PATH_WIDTH = 4;
const NODE_STROKE_WIDTH = 2;
const FIT_PADDING = NODE_RADIUS + 20;
const MIN_SCALE = 0.15;
const MAX_SCALE = 8;
const LABEL_FONT = "12px ui-monospace, SFMono-Regular, Menlo, monospace";
const LABEL_PADDING_Y = 6;
/** Maximum on-screen label length. Author-supplied place ids that
 *  exceed this are truncated with an ellipsis; the full id remains
 *  available via hover tooltip / inspector. */
const LABEL_MAX_CHARS = 28;
const LABEL_ELLIPSIS = "…";

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
  /** True when the active scene wants labels drawn — used to widen
   *  the bottom fit padding so labels never clip on auto-fit. */
  private hasNodeLabels = false;
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
    this.hasNodeLabels = scene.show_node_labels === true;

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
    this.drawArrowheads(input.theme, input.scene.paths);
    this.drawFlowParticles(input.theme, input.scene.paths);
    this.drawNodes(input.theme, input.scene.nodes);
    this.drawMovers(input.theme, input.movers);
    if (input.scene.show_node_labels === true) {
      this.drawNodeLabels(input.theme, input.scene.nodes, input.scene.node_names);
    }
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

  /** Convert a canvas-relative screen point to world coordinates. */
  screenToWorld(screenX: number, screenY: number): [number, number] {
    return [
      (screenX - this.viewport.offsetX) / this.viewport.scale,
      (screenY - this.viewport.offsetY) / this.viewport.scale,
    ];
  }

  /** Reset to the auto-fit viewport computed at the last setScene(). */
  resetViewport(): void {
    this.viewport = { ...this.fitViewport };
  }

  /**
   * Recompute the fit viewport from the cached world bounds for the
   * current canvas dimensions, then apply it as both the fit baseline
   * and the current viewport. Call this after resizing the canvas.
   */
  refitViewport(): void {
    if (!this.hasWorldBounds) return;
    const fit = this.computeFit();
    this.fitViewport = fit;
    this.viewport = { ...fit };
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
      // Only respond to the primary (left) button to avoid interfering
      // with context-menu and right-drag gestures.
      if (e.button !== 0) return;
      e.preventDefault();
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

    const endDrag = (e: PointerEvent): void => {
      isDragging = false;
      if (
        canvas.releasePointerCapture !== undefined &&
        (canvas.hasPointerCapture === undefined || canvas.hasPointerCapture(e.pointerId))
      ) {
        canvas.releasePointerCapture(e.pointerId);
      }
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

    // When labels are visible, add extra bottom padding so the
    // label text below the bottom row of nodes never clips into the
    // canvas border. Approximation: 12px font + 6px gap = ~22px.
    const labelPadding = this.hasNodeLabels ? 22 : 0;
    const availW = cssW - FIT_PADDING * 2;
    const availH = cssH - FIT_PADDING * 2 - labelPadding;
    const rawScale = Math.min(availW / worldW, availH / worldH);
    const scale = Math.max(
      MIN_SCALE,
      Math.min(MAX_SCALE, Number.isFinite(rawScale) ? rawScale : 1)
    );

    const offsetX = cssW / 2 - (this.worldMinX + worldW / 2) * scale;
    // Shift the geometry slightly upward so the extra label padding
    // shows at the bottom rather than equally on both sides.
    const offsetY =
      cssH / 2 - (this.worldMinY + worldH / 2) * scale - labelPadding / 2;

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

  /** Draw arrowheads for paths that carry a direction hint. The
   *  arrowhead is placed at the `to_pos` end (inset by NODE_RADIUS so
   *  it sits just outside the destination node), pointing along the
   *  link. Bidirectional links get a second arrowhead at `from_pos`.
   *  Legacy paths (arrow undefined) get nothing — preserves the
   *  current transit aesthetic. */
  private drawArrowheads(theme: Theme, paths: PathView[]): void {
    const ctx = this.ctx;
    for (const p of paths) {
      if (p.arrow === undefined) continue;
      const color = paletteColor(theme, p.color);
      drawArrowAtEnd(ctx, p.from_pos, p.to_pos, color);
      if (p.arrow === "bidirectional") {
        drawArrowAtEnd(ctx, p.to_pos, p.from_pos, color);
      }
    }
  }

  /** Draw purely visual data-flow particles for directed SL1 paths.
   *  Legacy paths (arrow undefined) get no particles so transit scenes
   *  keep their existing mover-dot aesthetic. */
  private drawFlowParticles(theme: Theme, paths: PathView[]): void {
    const ctx = this.ctx;
    const nowMs = performance.now();
    for (const p of paths) {
      if (p.arrow === undefined) continue;
      ctx.fillStyle = paletteColor(theme, p.color);
      drawParticleAtProgress(ctx, p.from_pos, p.to_pos, computeParticleProgress(nowMs, p.id, "forward"));
      if (p.arrow === "bidirectional") {
        drawParticleAtProgress(ctx, p.from_pos, p.to_pos, computeParticleProgress(nowMs, p.id, "reverse"));
      }
    }
  }

  /** Draw the node id label below each named node. Counter-scales the
   *  font so text stays a consistent on-screen size regardless of
   *  zoom; the world-space transform is in effect when this is called.
   *  All label text comes from author-supplied `node_names` and is
   *  rendered via the Canvas2D text API (fillText) — not via
   *  innerHTML — preserving the safe-text policy. */
  private drawNodeLabels(
    theme: Theme,
    nodes: NodeView[],
    nodeNames: Record<number, string>
  ): void {
    const ctx = this.ctx;
    const scale = this.viewport.scale;
    if (scale === 0) return;
    // Counter-scale font + offsets so labels look constant on screen.
    const fontScale = 1 / scale;
    ctx.save();
    ctx.font = LABEL_FONT;
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    ctx.fillStyle = foregroundColor(theme);
    const offsetY = (NODE_RADIUS + LABEL_PADDING_Y);
    for (const n of nodes) {
      const raw = nodeNames[n.id];
      if (raw === undefined || raw === "") continue;
      const text = truncateLabel(raw);
      // Translate to the node, then apply font scale so font size
      // measurement happens at constant pixel size.
      ctx.save();
      ctx.translate(n.pos[0], n.pos[1] + offsetY);
      ctx.scale(fontScale, fontScale);
      ctx.fillText(text, 0, 0);
      ctx.restore();
    }
    ctx.restore();
  }
}

/** Truncate label text to a safe on-screen length. Exported so unit
 *  tests can verify the truncation contract directly without going
 *  through the renderer. */
export function truncateLabel(text: string): string {
  if (text.length <= LABEL_MAX_CHARS) return text;
  return text.slice(0, LABEL_MAX_CHARS - 1) + LABEL_ELLIPSIS;
}

export function computeParticleProgress(
  nowMs: number,
  pathId: number,
  direction: "forward" | "reverse"
): number {
  const progress = (((nowMs / 1000) * FLOW_PARTICLE_SPEED + pathId * FLOW_PARTICLE_PHASE_OFFSET) % 1 + 1) % 1;
  return direction === "reverse" ? 1 - progress : progress;
}

export function computeParticlePosition(
  fromPos: readonly [number, number],
  toPos: readonly [number, number],
  progress: number
): [number, number] | undefined {
  const dx = toPos[0] - fromPos[0];
  const dy = toPos[1] - fromPos[1];
  const len = Math.hypot(dx, dy);
  if (len < NODE_RADIUS * 2) return undefined;
  const inset = NODE_RADIUS / len;
  const clamped = Math.max(inset, Math.min(1 - inset, progress));
  return [fromPos[0] + clamped * dx, fromPos[1] + clamped * dy];
}

function drawParticleAtProgress(
  ctx: CanvasRenderingContext2D,
  fromPos: readonly [number, number],
  toPos: readonly [number, number],
  progress: number
): void {
  const pos = computeParticlePosition(fromPos, toPos, progress);
  if (pos === undefined) return;
  ctx.beginPath();
  ctx.arc(pos[0], pos[1], FLOW_PARTICLE_RADIUS, 0, Math.PI * 2);
  ctx.fill();
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

const ARROW_HEAD_LEN = 12;
const ARROW_HEAD_HALF_WIDTH = 5;
const ARROW_TIP_INSET = NODE_RADIUS + 2;

/** Draw a filled triangular arrowhead at the `toPos` end of a segment
 *  from `fromPos`. Tip is inset by `ARROW_TIP_INSET` so it sits just
 *  outside the destination node circle. Exported for unit testing. */
export function drawArrowAtEnd(
  ctx: CanvasRenderingContext2D,
  fromPos: readonly [number, number],
  toPos: readonly [number, number],
  fillStyle: string
): void {
  const dx = toPos[0] - fromPos[0];
  const dy = toPos[1] - fromPos[1];
  const len = Math.hypot(dx, dy);
  if (len === 0 || len < ARROW_TIP_INSET + ARROW_HEAD_LEN) return;
  const ux = dx / len;
  const uy = dy / len;
  // Tip sits short of the destination node.
  const tipX = toPos[0] - ux * ARROW_TIP_INSET;
  const tipY = toPos[1] - uy * ARROW_TIP_INSET;
  // Two base corners perpendicular to the segment direction.
  const baseX = tipX - ux * ARROW_HEAD_LEN;
  const baseY = tipY - uy * ARROW_HEAD_LEN;
  // Perpendicular unit vector.
  const px = -uy;
  const py = ux;
  ctx.fillStyle = fillStyle;
  ctx.beginPath();
  ctx.moveTo(tipX, tipY);
  ctx.lineTo(baseX + px * ARROW_HEAD_HALF_WIDTH, baseY + py * ARROW_HEAD_HALF_WIDTH);
  ctx.lineTo(baseX - px * ARROW_HEAD_HALF_WIDTH, baseY - py * ARROW_HEAD_HALF_WIDTH);
  ctx.closePath();
  ctx.fill();
}
