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
    this.root.setAttribute("role", "region");
    this.root.setAttribute("aria-label", "Scene gallery");
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
    chips.setAttribute("role", "radiogroup");
    chips.setAttribute("aria-label", "Filter scenes by kind");
    for (const kind of ["all", "sl1_scenario", "transit_loop"] as const) {
      const chip = document.createElement("button");
      chip.type = "button";
      chip.dataset.filter = kind;
      chip.setAttribute("role", "radio");
      chip.setAttribute("aria-checked", kind === "all" ? "true" : "false");
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
          const other = c as HTMLElement;
          other.style.background = "transparent";
          other.setAttribute("aria-checked", "false");
        }
        chip.style.background = "#21262d";
        chip.setAttribute("aria-checked", "true");
      });
      chips.appendChild(chip);
    }
    header.appendChild(chips);
    this.root.appendChild(header);

    // Grid.
    this.grid = document.createElement("div");
    this.grid.setAttribute("role", "list");
    this.grid.setAttribute("aria-label", "Available scenes");
    this.grid.style.cssText = `
      max-width: 1200px; margin: 0 auto;
      display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
      gap: 16px;
    `;
    // Arrow-key navigation between cards.
    this.grid.addEventListener("keydown", (ev) => this.handleGridKeydown(ev));
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
    this.grid.replaceChildren();
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

  /** Arrow-key navigation between gallery cards. Moves focus through cards. */
  private handleGridKeydown(ev: KeyboardEvent): void {
    if (
      ev.key !== "ArrowLeft" &&
      ev.key !== "ArrowRight" &&
      ev.key !== "ArrowUp" &&
      ev.key !== "ArrowDown" &&
      ev.key !== "Home" &&
      ev.key !== "End"
    ) {
      return;
    }
    if (this.cards.length === 0) return;
    const active = document.activeElement;
    const currentIdx = this.cards.findIndex((c) => c.element === active);
    if (currentIdx === -1) {
      // Nothing focused yet — focus the first card.
      this.cards[0]?.element.focus();
      ev.preventDefault();
      return;
    }
    // Estimate columns by comparing card offsetTop — cards in the same row
    // share an offsetTop. This gives correct grid-arrow semantics across
    // window widths without hardcoding column count.
    const currentCard = this.cards[currentIdx];
    if (currentCard === undefined) return;
    const rowTop = currentCard.element.offsetTop;
    let cols = 0;
    for (const c of this.cards) {
      if (c.element.offsetTop === rowTop) cols += 1;
    }
    cols = Math.max(1, cols);

    let nextIdx = currentIdx;
    switch (ev.key) {
      case "ArrowLeft":
        nextIdx = Math.max(0, currentIdx - 1);
        break;
      case "ArrowRight":
        nextIdx = Math.min(this.cards.length - 1, currentIdx + 1);
        break;
      case "ArrowUp":
        nextIdx = Math.max(0, currentIdx - cols);
        break;
      case "ArrowDown":
        nextIdx = Math.min(this.cards.length - 1, currentIdx + cols);
        break;
      case "Home":
        nextIdx = 0;
        break;
      case "End":
        nextIdx = this.cards.length - 1;
        break;
    }
    if (nextIdx !== currentIdx) {
      this.cards[nextIdx]?.element.focus();
      ev.preventDefault();
    }
  }
}
