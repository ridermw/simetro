// frontend/src/ui/sl1_legend.ts
//
// scenario_language_v1 (SL1) role legend — a small floating overlay
// that decodes the canvas shape vocabulary for viewers landing on an
// SL1 scene for the first time. Answers "which shape is which?" so
// the viewer can identify each place at a glance.
//
// The legend is data-driven from `SL1_ROLE_HINTS` (the same table
// SL1 synth uses to assign shapes/colors), so there is no risk of
// drift between rendered shapes and legend swatches.
//
// All text rendered via `textContent` — safe-text policy.

import { SL1_ROLE_HINTS } from "../renderer/sl1_synth";
import type { Theme } from "../renderer/theme";
import { paletteColor } from "../renderer/theme";

const ROLE_LABELS: Record<string, string> = {
  source: "source",
  compute_cluster: "compute",
  dashboard: "dashboard",
  operator: "operator",
};

const SWATCH_SIZE = 16;

export class Sl1RoleLegend {
  private root: HTMLDivElement;

  constructor(parent: HTMLElement) {
    this.root = document.createElement("div");
    this.root.id = "simetro-sl1-legend";
    this.root.setAttribute("role", "region");
    this.root.setAttribute("aria-label", "Scene shape legend");
    this.root.style.cssText = [
      "position: absolute",
      "bottom: 12px",
      "left: 12px",
      "display: none",
      "flex-direction: column",
      "gap: 4px",
      "padding: 8px 10px",
      "background: rgba(14, 17, 22, 0.85)",
      "border: 1px solid rgba(232, 234, 237, 0.15)",
      "border-radius: 6px",
      "font: 11px ui-monospace, SFMono-Regular, Menlo, monospace",
      "color: #e6edf3",
      "z-index: 5",
      "pointer-events: none",
    ].join(";");
    parent.appendChild(this.root);
  }

  /** Show the legend with rows for each role present in the scene,
   *  using the scene theme so swatch colors match drawn nodes. */
  show(theme: Theme, roles: ReadonlySet<string>): void {
    while (this.root.firstChild !== null) this.root.removeChild(this.root.firstChild);
    if (roles.size === 0) {
      this.root.style.display = "none";
      return;
    }

    const heading = document.createElement("div");
    heading.style.cssText = "opacity: 0.65; text-transform: uppercase; letter-spacing: 0.08em;";
    heading.textContent = "legend";
    this.root.appendChild(heading);

    // Always render in canonical role order so reordering scene JSON
    // doesn't reshuffle the legend.
    const orderedRoles: string[] = ["source", "compute_cluster", "dashboard", "operator"];
    for (const role of orderedRoles) {
      if (!roles.has(role)) continue;
      const hint = SL1_ROLE_HINTS[role];
      if (hint === undefined) continue;
      const row = document.createElement("div");
      row.style.cssText = "display: flex; align-items: center; gap: 8px;";
      const swatch = makeSwatchCanvas(hint.shape, paletteColor(theme, hint.color));
      row.appendChild(swatch);
      const text = document.createElement("span");
      text.textContent = ROLE_LABELS[role] ?? role;
      row.appendChild(text);
      this.root.appendChild(row);
    }
    this.root.style.display = "flex";
  }

  hide(): void {
    this.root.style.display = "none";
  }

  /** Test helper: read-only access to the rendered root for assertions. */
  __testRoot(): HTMLElement {
    return this.root;
  }
}

function makeSwatchCanvas(shape: string, fillStyle: string): HTMLCanvasElement {
  const canvas = document.createElement("canvas");
  canvas.width = SWATCH_SIZE;
  canvas.height = SWATCH_SIZE;
  canvas.style.cssText = `width: ${SWATCH_SIZE}px; height: ${SWATCH_SIZE}px;`;
  const ctx = canvas.getContext("2d");
  if (ctx === null) return canvas;
  const cx = SWATCH_SIZE / 2;
  const cy = SWATCH_SIZE / 2;
  const r = SWATCH_SIZE / 2 - 2;
  ctx.fillStyle = fillStyle;
  ctx.strokeStyle = "#e6edf3";
  ctx.lineWidth = 1;
  ctx.beginPath();
  switch (shape) {
    case "circle":
      ctx.arc(cx, cy, r, 0, Math.PI * 2);
      break;
    case "square":
      ctx.rect(cx - r, cy - r, r * 2, r * 2);
      break;
    case "triangle":
      ctx.moveTo(cx, cy - r);
      ctx.lineTo(cx + r * 0.866, cy + r * 0.5);
      ctx.lineTo(cx - r * 0.866, cy + r * 0.5);
      ctx.closePath();
      break;
    case "diamond":
      ctx.moveTo(cx, cy - r);
      ctx.lineTo(cx + r, cy);
      ctx.lineTo(cx, cy + r);
      ctx.lineTo(cx - r, cy);
      ctx.closePath();
      break;
    case "hexagon": {
      const a = Math.PI / 3;
      ctx.moveTo(cx + r, cy);
      for (let i = 1; i < 6; i++) {
        ctx.lineTo(cx + r * Math.cos(i * a), cy + r * Math.sin(i * a));
      }
      ctx.closePath();
      break;
    }
    default:
      ctx.arc(cx, cy, r, 0, Math.PI * 2);
  }
  ctx.fill();
  ctx.stroke();
  return canvas;
}

/** Derive the set of roles present in an SL1 scene's places. Pure
 *  helper exported for unit testing. */
export function rolesInScene(places: ReadonlyArray<{ role: string }>): Set<string> {
  const roles = new Set<string>();
  for (const p of places) roles.add(p.role);
  return roles;
}
