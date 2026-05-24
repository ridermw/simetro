// frontend/src/inspector/hover.ts
//
// PLAN §4 / §20 DoD #6 — hover-to-explain. Tracks the cursor over
// the canvas, hit-tests against the current snapshot's nodes/movers,
// and surfaces a tooltip with the piece's human-readable JSON id
// (from Static.id_map) plus a one-line summary.
//
//   mousemove ─▶ hitTest(snapshot, x, y) ─▶ tooltip.show(id, summary)
//   mouseleave / no-hit ─▶ tooltip.hide()
//
// Pure function `hitTestPiece` is exported so unit tests can verify
// it without a real DOM event loop.

import type { NodeSnapshot, SnapshotPayload } from "../protocol/messages";

export interface HitResult {
  kind: "node" | "mover";
  id: number;
  /** Human-readable id from Static.id_map, if available. */
  label: string;
  pos: [number, number];
}

const NODE_HIT_RADIUS = 22;
const MOVER_HIT_RADIUS = 12;

export function hitTestPiece(
  snap: SnapshotPayload,
  idMap: Record<number, string>,
  x: number,
  y: number
): HitResult | null {
  // Movers are smaller but on top — check them first.
  for (const m of snap.movers) {
    const dx = m.pos[0] - x;
    const dy = m.pos[1] - y;
    if (dx * dx + dy * dy <= MOVER_HIT_RADIUS * MOVER_HIT_RADIUS) {
      return {
        kind: "mover",
        id: m.id,
        label: idMap[m.id] ?? `mover#${m.id}`,
        pos: [m.pos[0], m.pos[1]],
      };
    }
  }
  for (const n of snap.nodes) {
    const dx = n.pos[0] - x;
    const dy = n.pos[1] - y;
    if (dx * dx + dy * dy <= NODE_HIT_RADIUS * NODE_HIT_RADIUS) {
      return {
        kind: "node",
        id: n.id,
        label: idMap[n.id] ?? `node#${n.id}`,
        pos: [n.pos[0], n.pos[1]],
      };
    }
  }
  return null;
}

export function summarizeNode(n: NodeSnapshot, idMap: Record<number, string>): string {
  const label = idMap[n.id] ?? `node#${n.id}`;
  return `${label}  shape=${n.shape}  color=${n.color}`;
}

export class HoverTooltip {
  private el: HTMLDivElement;
  private snap: SnapshotPayload | null = null;
  private idMap: Record<number, string> = {};

  constructor(parent: HTMLElement) {
    this.el = document.createElement("div");
    this.el.id = "simetro-hover-tooltip";
    this.el.style.cssText = [
      "position: absolute",
      "padding: 4px 8px",
      "background: rgba(14, 17, 22, 0.92)",
      "color: #e8eaed",
      "font: 11px ui-monospace, SFMono-Regular, monospace",
      "border: 1px solid rgba(232, 234, 237, 0.2)",
      "border-radius: 4px",
      "pointer-events: none",
      "display: none",
      "z-index: 20",
      "white-space: nowrap",
    ].join(";");
    parent.appendChild(this.el);
  }

  setSnapshot(snap: SnapshotPayload | null, idMap: Record<number, string>): void {
    this.snap = snap;
    this.idMap = idMap;
  }

  attach(canvas: HTMLCanvasElement): void {
    canvas.addEventListener("mousemove", (ev) => this.onMove(ev, canvas));
    canvas.addEventListener("mouseleave", () => this.hide());
  }

  private onMove(ev: MouseEvent, canvas: HTMLCanvasElement): void {
    if (this.snap === null) {
      this.hide();
      return;
    }
    const rect = canvas.getBoundingClientRect();
    const x = ev.clientX - rect.left;
    const y = ev.clientY - rect.top;
    const hit = hitTestPiece(this.snap, this.idMap, x, y);
    if (hit === null) {
      this.hide();
      return;
    }
    this.show(hit, ev.clientX, ev.clientY, canvas);
  }

  private show(hit: HitResult, screenX: number, screenY: number, canvas: HTMLCanvasElement): void {
    const parentRect = canvas.getBoundingClientRect();
    this.el.textContent = `${hit.kind.toUpperCase()}  ${hit.label}`;
    this.el.style.display = "block";
    this.el.style.left = `${screenX - parentRect.left + 12}px`;
    this.el.style.top = `${screenY - parentRect.top + 12}px`;
  }

  hide(): void {
    this.el.style.display = "none";
  }

  /** Test-only accessor. */
  __testEl(): HTMLDivElement {
    return this.el;
  }
}
