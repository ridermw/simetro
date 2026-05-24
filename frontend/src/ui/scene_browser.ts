import type { SceneCatalogEntry } from "../catalog/scenes";

export interface SceneSelectIntent {
  readonly kind: "SelectScene";
  readonly scene_id: string;
}

export type SceneSelectHandler = (intent: SceneSelectIntent) => void;

export class SceneBrowser {
  private root: HTMLElement;
  private sceneButtons: Map<string, HTMLButtonElement> = new Map();
  private selectedSceneId: string | null = null;

  constructor(
    parent: HTMLElement,
    scenes: readonly SceneCatalogEntry[],
    handler: SceneSelectHandler,
    selectedSceneId: string | null = scenes[0]?.id ?? null
  ) {
    const built = buildDom(parent, scenes);
    this.root = built.root;
    this.sceneButtons = built.sceneButtons;
    this.selectedSceneId = selectedSceneId;

    for (const scene of scenes) {
      const button = this.sceneButtons.get(scene.id);
      button?.addEventListener("click", () => {
        handler({ kind: "SelectScene", scene_id: scene.id });
      });
    }

    this.refresh();
  }

  setSelected(scene_id: string | null): void {
    this.selectedSceneId = scene_id;
    this.refresh();
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
}

interface BuiltDom {
  root: HTMLElement;
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
    "padding: 10px",
    "background: rgba(14, 17, 22, 0.85)",
    "border: 1px solid rgba(232, 234, 237, 0.15)",
    "border-radius: 8px",
    "font: 12px ui-monospace, SFMono-Regular, monospace",
    "color: #e8eaed",
    "z-index: 10",
  ].join(";");

  const heading = document.createElement("div");
  heading.textContent = "scenes";
  heading.style.cssText = "opacity: 0.75; letter-spacing: 0.08em; text-transform: uppercase";
  root.appendChild(heading);

  const sceneButtons = new Map<string, HTMLButtonElement>();
  for (const scene of scenes) {
    const sceneButton = sceneCard(scene);
    sceneButtons.set(scene.id, sceneButton);
    root.appendChild(sceneButton);
  }

  parent.appendChild(root);
  return { root, sceneButtons };
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
