// frontend/src/ui/gallery_card.ts
//
// Individual scene card for the gallery grid. Renders a thumbnail from
// a pre-built StaticPayload, with palette swatch fallback on failure.
// All text rendered via textContent — never innerHTML.

import type { SceneCatalogEntry } from "../catalog/scenes";
import type { StaticPayload } from "../protocol/messages";
import { SCHEMA_VERSION } from "../protocol/messages";
import { renderThumbnail, renderPaletteSwatch } from "./thumbnail_renderer";
import { synthesizeSl1Geometry } from "../renderer/sl1_synth";

const THUMB_WIDTH = 320;
const THUMB_HEIGHT = 180;

const DIFFICULTY_COLORS: Record<string, string> = {
  intro: "#4ade80",
  easy: "#60a5fa",
  medium: "#fbbf24",
  hard: "#f87171",
};

export class GalleryCard {
  readonly element: HTMLButtonElement;
  private thumbContainer: HTMLElement;
  private loaded = false;

  constructor(
    private readonly scene: SceneCatalogEntry,
    private readonly onClick: () => void
  ) {
    this.element = document.createElement("button");
    this.element.type = "button";
    // data-scene-id for E2E selectors (rubber-duck fix: avoid hitting filter chips).
    this.element.dataset.sceneId = scene.id;
    // Keep native button role — do NOT override with role="listitem" or
    // similar; that would prevent screen readers from announcing the
    // card as an actionable button (Codex P1 finding on PR #51).
    // Accessible label so screen readers announce scene title + metadata,
    // not just the button role.
    this.element.setAttribute(
      "aria-label",
      `${scene.title}: ${scene.subtitle}. ${scene.difficulty} difficulty.`
    );
    this.element.style.cssText = `
      display: flex; flex-direction: column; border: 1px solid #30363d;
      border-radius: 8px; overflow: hidden; background: #161b22;
      cursor: pointer; padding: 0; text-align: left; width: 100%;
      transition: transform 0.15s, border-color 0.15s, box-shadow 0.15s;
    `;
    this.element.addEventListener("mouseenter", () => {
      this.element.style.transform = "scale(1.02)";
      this.element.style.borderColor = "#58a6ff";
    });
    this.element.addEventListener("mouseleave", () => {
      this.element.style.transform = "scale(1)";
      if (document.activeElement !== this.element) {
        this.element.style.borderColor = "#30363d";
      }
    });
    // Visible focus indicator for keyboard navigation.
    this.element.addEventListener("focus", () => {
      this.element.style.borderColor = "#58a6ff";
      this.element.style.boxShadow = "0 0 0 2px #58a6ff66";
    });
    this.element.addEventListener("blur", () => {
      this.element.style.borderColor = "#30363d";
      this.element.style.boxShadow = "none";
    });
    this.element.addEventListener("click", this.onClick);

    // Thumbnail container.
    this.thumbContainer = document.createElement("div");
    this.thumbContainer.style.cssText = `
      width: 100%; aspect-ratio: 16/9; background: #0e1116; position: relative;
    `;
    this.element.appendChild(this.thumbContainer);

    // Text content — all via textContent for XSS safety.
    const info = document.createElement("div");
    info.style.cssText = "padding: 12px;";

    const title = document.createElement("div");
    title.style.cssText =
      "font-weight: 600; color: #e6edf3; font-size: 14px; margin-bottom: 4px;";
    title.textContent = scene.title;
    info.appendChild(title);

    const subtitle = document.createElement("div");
    subtitle.style.cssText =
      "color: #8b949e; font-size: 12px; margin-bottom: 8px;";
    subtitle.textContent = scene.subtitle;
    info.appendChild(subtitle);

    // Difficulty pill.
    const pill = document.createElement("span");
    pill.style.cssText = `
      display: inline-block; padding: 2px 8px; border-radius: 12px;
      font-size: 11px; font-weight: 500;
      background: ${DIFFICULTY_COLORS[scene.difficulty] ?? "#555"}22;
      color: ${DIFFICULTY_COLORS[scene.difficulty] ?? "#aaa"};
    `;
    pill.textContent = scene.difficulty;
    info.appendChild(pill);

    this.element.appendChild(info);
  }

  /** Load and render thumbnail. Call when card becomes visible (IntersectionObserver). */
  async loadThumbnail(): Promise<void> {
    if (this.loaded) return;
    this.loaded = true;

    try {
      const resp = await fetch(`/static-payloads/${this.scene.id}.json`);
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      const envelope = (await resp.json()) as {
        schema_version: number;
        payload: StaticPayload;
      };
      if (envelope.schema_version !== SCHEMA_VERSION) {
        throw new Error(`schema mismatch: ${envelope.schema_version}`);
      }
      const canvas = renderThumbnail(
        synthesizeSl1Geometry(envelope.payload),
        THUMB_WIDTH,
        THUMB_HEIGHT
      );
      canvas.style.cssText = "width: 100%; height: 100%; object-fit: cover;";
      this.thumbContainer.appendChild(canvas);
    } catch (e) {
      console.warn(`simetro: thumbnail fallback for ${this.scene.id}:`, e);
      const fallback = renderPaletteSwatch(
        this.scene.palette_name === "simetro_dark"
          ? ["#0e1116", "#e8eaed", "#7aa2f7", "#bb9af7", "#9ece6a"]
          : ["#0e1116", "#e8eaed", "#7aa2f7"],
        THUMB_WIDTH,
        THUMB_HEIGHT
      );
      fallback.style.cssText = "width: 100%; height: 100%;";
      this.thumbContainer.appendChild(fallback);
    }
  }

  /** Release canvas GPU memory (call when leaving gallery view). */
  releaseMemory(): void {
    const canvas = this.thumbContainer.querySelector("canvas");
    if (canvas) {
      canvas.width = 0;
      canvas.height = 0;
    }
    this.thumbContainer.replaceChildren();
    this.loaded = false;
  }
}
