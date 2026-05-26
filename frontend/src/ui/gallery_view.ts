// frontend/src/ui/gallery_view.ts
//
// Full-page gallery view. Sections: "SL1 Scenarios" then "Transit Loops".
// Sorted by difficulty within each section. IntersectionObserver lazy-loads
// thumbnails as cards scroll into view.
//
//   ┌──────────────────────────────────────────────┐
//   │  simetro           [All] [SL1] [Transit]     │
//   ├──────────────────────────────────────────────┤
//   │  ── SL1 Scenarios ──────────────────────     │
//   │  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐       │
//   │  │ card │ │ card │ │ card │ │ card │        │
//   │  └──────┘ └──────┘ └──────┘ └──────┘       │
//   │  ── Transit Loops ──────────────────────     │
//   │  ┌──────┐ ┌──────┐ ┌──────┐ ...            │
//   │  │ card │ │ card │ │ card │                 │
//   │  └──────┘ └──────┘ └──────┘                 │
//   └──────────────────────────────────────────────┘

import type {
  SceneCatalogEntry,
  SceneDifficulty,
  SceneWorldKind,
} from "../catalog/scenes";
import { GalleryCard } from "./gallery_card";

export interface SceneSelectIntent {
  readonly kind: "SelectScene";
  readonly scene_id: string;
}

export type SceneSelectHandler = (intent: SceneSelectIntent) => void;

export interface GalleryFilter {
  world_kind: "all" | SceneWorldKind;
  difficulty: "all" | SceneDifficulty;
}

const DIFFICULTY_ORDER: Record<string, number> = {
  intro: 0,
  easy: 1,
  medium: 2,
  hard: 3,
};

export class GalleryView {
  private root: HTMLElement;
  private grid: HTMLElement;
  private cards: GalleryCard[] = [];
  private observer: IntersectionObserver;
  private filter: GalleryFilter = { world_kind: "all", difficulty: "all" };
  private scenes: readonly SceneCatalogEntry[];

  constructor(
    container: HTMLElement,
    scenes: readonly SceneCatalogEntry[],
    private readonly onSelect: SceneSelectHandler
  ) {
    this.scenes = scenes;
    this.root = document.createElement("div");
    this.root.id = "simetro-gallery";
    this.root.style.cssText = `
      position: fixed; inset: 0; z-index: 1000; background: #0e1116;
      overflow-y: auto; display: none; padding: 40px;
    `;

    // Header.
    const header = document.createElement("div");
    header.style.cssText =
      "max-width: 1200px; margin: 0 auto 24px; display: flex; align-items: center; gap: 16px;";
    const title = document.createElement("h1");
    title.style.cssText = "color: #e6edf3; font-size: 24px; margin: 0;";
    title.textContent = "simetro";
    header.appendChild(title);

    // Filter chips.
    const chips = document.createElement("div");
    chips.style.cssText = "display: flex; gap: 8px;";
    chips.dataset.role = "filter-chips";
    for (const kind of ["all", "sl1_scenario", "transit_loop"] as const) {
      const chip = document.createElement("button");
      chip.type = "button";
      chip.dataset.filter = kind;
      chip.style.cssText = `
        padding: 4px 12px; border-radius: 16px; border: 1px solid #30363d;
        background: ${kind === "all" ? "#21262d" : "transparent"};
        color: #e6edf3; font-size: 12px; cursor: pointer;
      `;
      chip.textContent =
        kind === "all"
          ? "All"
          : kind === "sl1_scenario"
            ? "SL1 Scenarios"
            : "Transit Loops";
      chip.addEventListener("click", () => {
        this.setFilter({ ...this.filter, world_kind: kind });
        for (const c of chips.children) {
          (c as HTMLElement).style.background = "transparent";
        }
        chip.style.background = "#21262d";
      });
      chips.appendChild(chip);
    }
    header.appendChild(chips);
    this.root.appendChild(header);

    // Grid.
    this.grid = document.createElement("div");
    this.grid.style.cssText = `
      max-width: 1200px; margin: 0 auto;
      display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
      gap: 16px;
    `;
    this.root.appendChild(this.grid);

    container.appendChild(this.root);

    // IntersectionObserver for lazy thumbnail loading.
    this.observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            const card = this.cards.find((c) => c.element === entry.target);
            if (card) void card.loadThumbnail();
          }
        }
      },
      { root: this.root, rootMargin: "200px" }
    );

    this.render();
  }

  show(): void {
    this.root.style.display = "block";
    for (const card of this.cards) {
      this.observer.observe(card.element);
    }
  }

  hide(): void {
    this.root.style.display = "none";
    this.observer.disconnect();
  }

  /** Release all thumbnail canvas memory (call when entering sim view). */
  releaseMemory(): void {
    for (const card of this.cards) card.releaseMemory();
  }

  setFilter(filter: GalleryFilter): void {
    this.filter = filter;
    this.render();
  }

  private render(): void {
    this.observer.disconnect();
    this.grid.innerHTML = "";
    this.cards = [];

    const filtered = this.scenes.filter((s) => {
      if (s.status !== "ready") return false;
      if (
        this.filter.world_kind !== "all" &&
        s.world_kind !== this.filter.world_kind
      )
        return false;
      if (
        this.filter.difficulty !== "all" &&
        s.difficulty !== this.filter.difficulty
      )
        return false;
      return true;
    });

    // Sort: SL1 first, then transit; within each group sort by difficulty.
    const sorted = [...filtered].sort((a, b) => {
      const kindOrder = a.world_kind === "sl1_scenario" ? 0 : 1;
      const kindOrderB = b.world_kind === "sl1_scenario" ? 0 : 1;
      if (kindOrder !== kindOrderB) return kindOrder - kindOrderB;
      return (
        (DIFFICULTY_ORDER[a.difficulty] ?? 0) -
        (DIFFICULTY_ORDER[b.difficulty] ?? 0)
      );
    });

    if (sorted.length === 0) {
      const empty = document.createElement("div");
      empty.style.cssText =
        "color: #8b949e; text-align: center; padding: 40px; grid-column: 1/-1;";
      empty.textContent = "No scenes match the current filter.";
      this.grid.appendChild(empty);
      return;
    }

    // Section headers.
    let currentKind: string | null = null;
    for (const scene of sorted) {
      if (scene.world_kind !== currentKind && this.filter.world_kind === "all") {
        currentKind = scene.world_kind;
        const sectionHeader = document.createElement("div");
        sectionHeader.style.cssText =
          "grid-column: 1/-1; color: #8b949e; font-size: 14px; font-weight: 600; margin-top: 16px; padding-bottom: 8px; border-bottom: 1px solid #21262d;";
        sectionHeader.textContent =
          currentKind === "sl1_scenario" ? "SL1 Scenarios" : "Transit Loops";
        this.grid.appendChild(sectionHeader);
      }

      const card = new GalleryCard(scene, () => {
        this.onSelect({ kind: "SelectScene", scene_id: scene.id });
      });
      this.cards.push(card);
      this.grid.appendChild(card.element);
    }

    // Start observing if visible.
    if (this.root.style.display !== "none") {
      for (const card of this.cards) {
        this.observer.observe(card.element);
      }
    }
  }
}
