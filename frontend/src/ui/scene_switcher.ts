// frontend/src/ui/scene_switcher.ts
//
// Compact floating pill for simulation view. Shows current scene title +
// prev/next arrows + gallery button. Auto-fades after 3s inactivity;
// reappears when mouse enters the top-right corner.
//
//   ┌──────────────────────────────────────────┐
//   │  ◀  GPU Launch Week  ▶  ⊞               │
//   └──────────────────────────────────────────┘

import type { SceneCatalogEntry } from "../catalog/scenes";

export interface SwitcherHandler {
  onPrev(sceneId: string): void;
  onNext(sceneId: string): void;
  onGallery(): void;
}

export class SceneSwitcher {
  private root: HTMLElement;
  private titleEl: HTMLElement;
  private hideTimer: number | null = null;
  private scenes: readonly SceneCatalogEntry[];
  private currentIndex = 0;
  private abortController = new AbortController();

  constructor(
    parent: HTMLElement,
    scenes: readonly SceneCatalogEntry[],
    private readonly handler: SwitcherHandler
  ) {
    this.scenes = scenes.filter((s) => s.status === "ready");

    this.root = document.createElement("div");
    this.root.id = "simetro-switcher";
    this.root.style.cssText = `
      position: fixed; top: 12px; right: 12px; z-index: 900;
      display: none; align-items: center; gap: 8px;
      background: #161b22ee; border: 1px solid #30363d;
      border-radius: 20px; padding: 6px 12px;
      font-size: 13px; color: #e6edf3;
      transition: opacity 0.3s;
    `;

    const prevBtn = document.createElement("button");
    prevBtn.type = "button";
    prevBtn.textContent = "\u25C0";
    prevBtn.style.cssText =
      "background: none; border: none; color: #e6edf3; cursor: pointer; font-size: 14px;";
    prevBtn.addEventListener("click", () => {
      const sceneId = this.getAdjacentId("prev");
      if (sceneId !== null) this.handler.onPrev(sceneId);
    });
    this.root.appendChild(prevBtn);

    this.titleEl = document.createElement("span");
    this.titleEl.style.cssText =
      "max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;";
    this.root.appendChild(this.titleEl);

    const nextBtn = document.createElement("button");
    nextBtn.type = "button";
    nextBtn.textContent = "\u25B6";
    nextBtn.style.cssText =
      "background: none; border: none; color: #e6edf3; cursor: pointer; font-size: 14px;";
    nextBtn.addEventListener("click", () => {
      const sceneId = this.getAdjacentId("next");
      if (sceneId !== null) this.handler.onNext(sceneId);
    });
    this.root.appendChild(nextBtn);

    const galleryBtn = document.createElement("button");
    galleryBtn.type = "button";
    galleryBtn.textContent = "\u229E";
    galleryBtn.title = "Gallery";
    galleryBtn.style.cssText =
      "background: none; border: none; color: #8b949e; cursor: pointer; font-size: 16px; margin-left: 8px;";
    galleryBtn.addEventListener("click", () => this.handler.onGallery());
    this.root.appendChild(galleryBtn);

    parent.appendChild(this.root);

    // Show on mouse near top-right.
    document.addEventListener(
      "mousemove",
      (e) => {
        if (this.root.style.display === "none") return;
        if (e.clientX > window.innerWidth - 300 && e.clientY < 80) {
          this.root.style.opacity = "1";
          this.resetHideTimer();
        }
      },
      { signal: this.abortController.signal }
    );
  }

  show(): void {
    this.root.style.display = "flex";
    this.root.style.opacity = "1";
    this.resetHideTimer();
  }

  hide(): void {
    this.root.style.display = "none";
    if (this.hideTimer !== null) {
      clearTimeout(this.hideTimer);
      this.hideTimer = null;
    }
  }

  /** Remove from the DOM and release all event listeners. */
  destroy(): void {
    this.hide();
    this.abortController.abort();
    this.root.remove();
  }

  setSelected(sceneId: string): void {
    const idx = this.scenes.findIndex((s) => s.id === sceneId);
    if (idx >= 0) {
      this.currentIndex = idx;
      const scene = this.scenes[idx];
      if (scene !== undefined) this.titleEl.textContent = scene.title;
    }
  }

  getAdjacentId(direction: "prev" | "next"): string | null {
    if (this.scenes.length === 0) return null;
    const newIdx =
      direction === "prev"
        ? (this.currentIndex - 1 + this.scenes.length) % this.scenes.length
        : (this.currentIndex + 1) % this.scenes.length;
    const scene = this.scenes[newIdx];
    return scene !== undefined ? scene.id : null;
  }

  private resetHideTimer(): void {
    if (this.hideTimer !== null) clearTimeout(this.hideTimer);
    this.hideTimer = window.setTimeout(() => {
      this.root.style.opacity = "0.3";
    }, 3000);
  }
}
