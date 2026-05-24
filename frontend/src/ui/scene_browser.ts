import type { SceneCatalogEntry } from "../catalog/scenes";

export interface SceneSelectIntent {
  readonly kind: "SelectScene";
  readonly scene_id: string;
}

export type SceneSelectHandler = (intent: SceneSelectIntent) => void;

const LIST_ID = "simetro-scene-list";
const TOGGLE_ID = "simetro-scene-toggle";

export class SceneBrowser {
  private root: HTMLElement;
  private toggle: HTMLButtonElement;
  private list: HTMLElement;
  private sceneButtons: Map<string, HTMLButtonElement> = new Map();
  private selectedSceneId: string | null = null;
  private collapsed = false;

  constructor(
    parent: HTMLElement,
    scenes: readonly SceneCatalogEntry[],
    handler: SceneSelectHandler,
    selectedSceneId: string | null = scenes[0]?.id ?? null
  ) {
    const built = buildDom(parent, scenes);
    this.root = built.root;
    this.toggle = built.toggle;
    this.list = built.list;
    this.sceneButtons = built.sceneButtons;
    this.selectedSceneId = selectedSceneId;

    for (const scene of scenes) {
      const button = this.sceneButtons.get(scene.id);
      button?.addEventListener("click", () => {
        handler({ kind: "SelectScene", scene_id: scene.id });
      });
    }

    this.toggle.addEventListener("click", () => {
      this.setCollapsed(!this.collapsed);
    });

    this.refresh();
    this.applyCollapsed();
  }

  setSelected(scene_id: string | null): void {
    this.selectedSceneId = scene_id;
    this.refresh();
  }

  /** Collapse (hide list, keep header) or expand the panel. */
  setCollapsed(collapsed: boolean): void {
    this.collapsed = collapsed;
    this.applyCollapsed();
  }

  /** Current collapse state — useful for tests and persistence. */
  isCollapsed(): boolean {
    return this.collapsed;
  }

  __testRoot(): HTMLElement {
    return this.root;
  }

  private refresh(): void {
    for (const [scene_id, button] of this.sceneButtons) {
      const selected = scene_id === this.selectedSceneId;
      button.setAttribute("aria-pressed", selected ? "true" : "false");
      button.style.borderColor = selected
        ? "rgba(122, 162, 247, 0.85)"
        : "rgba(232, 234, 237, 0.15)";
    }
  }

  private applyCollapsed(): void {
    this.toggle.setAttribute("aria-expanded", this.collapsed ? "false" : "true");
    // Chevron indicator: ▾ expanded, ▸ collapsed
    const indicator = this.toggle.querySelector<HTMLSpanElement>(
      "[data-role='scene-toggle-indicator']"
    );
    if (indicator) {
      indicator.textContent = this.collapsed ? "▸" : "▾";
    }
    this.list.style.display = this.collapsed ? "none" : "flex";
  }
}

interface BuiltDom {
  root: HTMLElement;
  toggle: HTMLButtonElement;
  list: HTMLElement;
  sceneButtons: Map<string, HTMLButtonElement>;
}

function buildDom(parent: HTMLElement, scenes: readonly SceneCatalogEntry[]): BuiltDom {
  const root = document.createElement("section");
  root.id = "simetro-scene-browser";
  root.setAttribute("aria-label", "Scene browser");
  root.style.cssText = [
    "position: absolute",
    "top: 12px",
    "right: 12px",
    "display: flex",
    "flex-direction: column",
    "gap: 8px",
    "width: min(320px, calc(100vw - 24px))",
    // Constrain to viewport so the scrollable list has somewhere to
    // overflow into. 24px = top offset + breathing room at the bottom.
    "max-height: calc(100vh - 24px)",
    "padding: 10px",
    "background: rgba(14, 17, 22, 0.85)",
    "border: 1px solid rgba(232, 234, 237, 0.15)",
    "border-radius: 8px",
    "font: 12px ui-monospace, SFMono-Regular, monospace",
    "color: #e8eaed",
    "z-index: 10",
    // Allow the inner list to shrink-and-scroll inside the flex column
    // even when the page itself has fixed sizing.
    "min-height: 0",
    "box-sizing: border-box",
  ].join(";");

  const toggle = document.createElement("button");
  toggle.id = TOGGLE_ID;
  toggle.type = "button";
  toggle.setAttribute("aria-controls", LIST_ID);
  toggle.setAttribute("aria-expanded", "true");
  toggle.style.cssText = [
    "display: flex",
    "align-items: center",
    "justify-content: space-between",
    "gap: 8px",
    "padding: 2px 0",
    "background: transparent",
    "color: inherit",
    "border: none",
    "font: inherit",
    "letter-spacing: 0.08em",
    "text-transform: uppercase",
    "cursor: pointer",
    "text-align: left",
  ].join(";");

  const heading = document.createElement("span");
  heading.textContent = "scenes";
  heading.style.cssText = "opacity: 0.75";

  const indicator = document.createElement("span");
  indicator.dataset.role = "scene-toggle-indicator";
  indicator.textContent = "▾";
  indicator.setAttribute("aria-hidden", "true");
  indicator.style.cssText = "opacity: 0.75";

  toggle.appendChild(heading);
  toggle.appendChild(indicator);

  const list = document.createElement("div");
  list.id = LIST_ID;
  list.setAttribute("role", "group");
  list.setAttribute("aria-label", "Scenes");
  list.style.cssText = [
    "display: flex",
    "flex-direction: column",
    "gap: 8px",
    // Scrollable when content exceeds available height.
    "overflow-y: auto",
    "min-height: 0",
    // Stay within the panel's max-height; flex shrink handles the rest.
    "flex: 1 1 auto",
    // Thin scrollbar so it doesn't dominate the panel visually.
    "scrollbar-width: thin",
    "scrollbar-color: rgba(232, 234, 237, 0.25) transparent",
    // Small right padding so cards don't touch the scrollbar.
    "padding-right: 4px",
  ].join(";");

  const sceneButtons = new Map<string, HTMLButtonElement>();
  for (const scene of scenes) {
    const sceneButton = sceneCard(scene);
    sceneButtons.set(scene.id, sceneButton);
    list.appendChild(sceneButton);
  }

  root.appendChild(toggle);
  root.appendChild(list);
  parent.appendChild(root);
  return { root, toggle, list, sceneButtons };
}

function sceneCard(scene: SceneCatalogEntry): HTMLButtonElement {
  const button = document.createElement("button");
  button.id = `simetro-scene-${scene.id}`;
  button.type = "button";
  button.setAttribute("aria-label", `Select scene ${scene.title}`);
  button.style.cssText = [
    "display: flex",
    "flex-direction: column",
    "gap: 4px",
    "padding: 8px",
    "text-align: left",
    "background: rgba(255, 255, 255, 0.03)",
    "color: inherit",
    "border: 1px solid rgba(232, 234, 237, 0.15)",
    "border-radius: 6px",
    "font: inherit",
    "cursor: pointer",
    // Don't allow individual cards to shrink — they must keep their
    // natural height, and the parent .list scrolls instead.
    "flex: 0 0 auto",
  ].join(";");

  const title = document.createElement("strong");
  title.textContent = scene.title;

  const meta = document.createElement("span");
  meta.textContent = `${scene.status} · ${scene.difficulty} · ${scene.world_kind}`;
  meta.style.cssText = "opacity: 0.7";

  const subtitle = document.createElement("span");
  subtitle.textContent = scene.subtitle;

  button.appendChild(title);
  button.appendChild(meta);
  button.appendChild(subtitle);
  return button;
}
