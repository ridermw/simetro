import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SCENE_CATALOG, type SceneCatalogEntry } from "../../catalog/scenes";
import { GalleryCard } from "../../ui/gallery_card";
import { GalleryView, type SceneSelectIntent } from "../../ui/gallery_view";
import { SceneSwitcher } from "../../ui/scene_switcher";

class FakeIntersectionObserver {
  observe(): void {}
  disconnect(): void {}
}

const baseScene = SCENE_CATALOG[0]!;

function makeScene(
  overrides: Partial<SceneCatalogEntry> & Pick<SceneCatalogEntry, "id">
): SceneCatalogEntry {
  const id = overrides.id;
  return {
    ...baseScene,
    ...overrides,
    scene_path: overrides.scene_path ?? `games/${id}.json`,
  };
}

function buttonWithText(parent: ParentNode, text: string): HTMLButtonElement {
  const button = Array.from(parent.querySelectorAll<HTMLButtonElement>("button")).find(
    (candidate) => candidate.textContent === text
  );
  expect(button).toBeDefined();
  return button!;
}

function installIntersectionObserver(): () => void {
  const originalIntersectionObserver = globalThis.IntersectionObserver;
  Object.defineProperty(globalThis, "IntersectionObserver", {
    configurable: true,
    writable: true,
    value: FakeIntersectionObserver,
  });

  return () => {
    if (originalIntersectionObserver === undefined) {
      delete (globalThis as { IntersectionObserver?: unknown }).IntersectionObserver;
      return;
    }
    Object.defineProperty(globalThis, "IntersectionObserver", {
      configurable: true,
      writable: true,
      value: originalIntersectionObserver,
    });
  };
}

describe("GalleryView DOM", () => {
  let parent: HTMLElement;
  let restoreIntersectionObserver: () => void;

  const sl1Scene = makeScene({
    id: "gpu-launch-week",
    title: "GPU Launch Week",
    subtitle: "Keep dashboards fresh while launch pressure rises.",
    world_kind: "sl1_scenario",
    difficulty: "medium",
    status: "ready",
  });
  const transitScene = makeScene({
    id: "demo-loop",
    title: "Demo Loop",
    subtitle: "A ready transit loop.",
    world_kind: "transit_loop",
    difficulty: "intro",
    status: "ready",
  });
  const draftScene = makeScene({
    id: "draft-loop",
    title: "Draft Loop",
    subtitle: "A draft scene that should not render.",
    world_kind: "transit_loop",
    status: "draft",
  });

  beforeEach(() => {
    restoreIntersectionObserver = installIntersectionObserver();
    parent = document.createElement("div");
    document.body.appendChild(parent);
  });

  afterEach(() => {
    restoreIntersectionObserver();
    document.body.replaceChildren();
    const hostileWindow = window as Window & {
      __hostile?: boolean;
      __hostile2?: boolean;
    };
    delete hostileWindow.__hostile;
    delete hostileWindow.__hostile2;
  });

  it("renders with the simetro header", () => {
    new GalleryView(parent, [sl1Scene], () => {});

    expect(parent.textContent).toContain("simetro");
  });

  it("renders all ready scenes as cards with data-scene-id", () => {
    new GalleryView(parent, [sl1Scene, transitScene, draftScene], () => {});

    expect(parent.querySelector(`[data-scene-id="${sl1Scene.id}"]`)).not.toBeNull();
    expect(parent.querySelector(`[data-scene-id="${transitScene.id}"]`)).not.toBeNull();
    expect(parent.querySelector(`[data-scene-id="${draftScene.id}"]`)).toBeNull();
  });

  it("renders filter chips for all supported scene groups", () => {
    new GalleryView(parent, [sl1Scene, transitScene], () => {});

    expect(buttonWithText(parent, "All")).not.toBeNull();
    expect(buttonWithText(parent, "SL1 Scenarios")).not.toBeNull();
    expect(buttonWithText(parent, "Transit Loops")).not.toBeNull();
  });

  it("clicking the SL1 filter shows only sl1_scenario cards", () => {
    new GalleryView(parent, [sl1Scene, transitScene], () => {});

    buttonWithText(parent, "SL1 Scenarios").click();

    expect(parent.querySelector(`[data-scene-id="${sl1Scene.id}"]`)).not.toBeNull();
    expect(parent.querySelector(`[data-scene-id="${transitScene.id}"]`)).toBeNull();
  });

  it("clicking the Transit filter shows only transit_loop cards", () => {
    new GalleryView(parent, [sl1Scene, transitScene], () => {});

    buttonWithText(parent, "Transit Loops").click();

    expect(parent.querySelector(`[data-scene-id="${sl1Scene.id}"]`)).toBeNull();
    expect(parent.querySelector(`[data-scene-id="${transitScene.id}"]`)).not.toBeNull();
  });

  it("renders an empty state for an empty catalog", () => {
    new GalleryView(parent, [], () => {});

    expect(parent.textContent).toContain("No scenes");
  });

  it("renders catalog text safely without executing hostile markup", () => {
    const hostile = makeScene({
      id: "hostile-scene",
      title: "<script>window.__hostile=true</script>",
      subtitle: "<img src=x onerror=window.__hostile2=true>",
      world_kind: "sl1_scenario",
      status: "ready",
    });

    new GalleryView(parent, [hostile], () => {});

    const hostileWindow = window as Window & {
      __hostile?: boolean;
      __hostile2?: boolean;
    };
    expect(parent.querySelector("script")).toBeNull();
    expect(parent.querySelector("img")).toBeNull();
    expect(hostileWindow.__hostile).toBeUndefined();
    expect(hostileWindow.__hostile2).toBeUndefined();
    expect(parent.textContent).toContain("<script>window.__hostile=true</script>");
  });

  it("fires onSelect with a SelectScene intent when a card is clicked", () => {
    const intents: SceneSelectIntent[] = [];
    new GalleryView(parent, [sl1Scene], (intent) => intents.push(intent));

    parent.querySelector<HTMLButtonElement>(`[data-scene-id="${sl1Scene.id}"]`)?.click();

    expect(intents).toEqual([{ kind: "SelectScene", scene_id: sl1Scene.id }]);
  });

  it("hide and show toggle the gallery display", () => {
    const gallery = new GalleryView(parent, [sl1Scene], () => {});
    const root = parent.querySelector<HTMLElement>("#simetro-gallery");

    gallery.show();
    expect(root?.style.display).toBe("block");

    gallery.hide();
    expect(root?.style.display).toBe("none");
  });

  it("releaseMemory is callable without throwing", () => {
    const gallery = new GalleryView(parent, [sl1Scene], () => {});

    expect(() => gallery.releaseMemory()).not.toThrow();
  });
});

describe("GalleryCard DOM", () => {
  afterEach(() => {
    document.body.replaceChildren();
    const hostileWindow = window as Window & {
      __cardHostile?: boolean;
      __cardHostile2?: boolean;
    };
    delete hostileWindow.__cardHostile;
    delete hostileWindow.__cardHostile2;
  });

  it("renders title and subtitle via textContent", () => {
    const scene = makeScene({
      id: "card-scene",
      title: "Card Scene",
      subtitle: "Card subtitle",
    });
    const card = new GalleryCard(scene, () => {});

    expect(card.element.textContent).toContain("Card Scene");
    expect(card.element.textContent).toContain("Card subtitle");
  });

  it("sets a data-scene-id attribute matching the scene id", () => {
    const scene = makeScene({ id: "card-scene" });
    const card = new GalleryCard(scene, () => {});

    expect(card.element.dataset.sceneId).toBe("card-scene");
  });

  it("renders hostile strings safely without creating executable markup", () => {
    const scene = makeScene({
      id: "hostile-card",
      title: "<script>window.__cardHostile=true</script>",
      subtitle: "<img src=x onerror=window.__cardHostile2=true>",
    });
    const card = new GalleryCard(scene, () => {});
    document.body.appendChild(card.element);

    const hostileWindow = window as Window & {
      __cardHostile?: boolean;
      __cardHostile2?: boolean;
    };
    expect(card.element.querySelector("script")).toBeNull();
    expect(card.element.querySelector("img")).toBeNull();
    expect(hostileWindow.__cardHostile).toBeUndefined();
    expect(hostileWindow.__cardHostile2).toBeUndefined();
    expect(card.element.textContent).toContain("<script>window.__cardHostile=true</script>");
  });

  it("releaseMemory removes thumbnail canvas children", () => {
    const scene = makeScene({ id: "canvas-card" });
    const card = new GalleryCard(scene, () => {});
    const thumbnail = card.element.firstElementChild;
    const canvas = document.createElement("canvas");
    thumbnail?.appendChild(canvas);

    card.releaseMemory();

    expect(thumbnail?.querySelector("canvas")).toBeNull();
    expect(thumbnail?.childElementCount).toBe(0);
  });
});

describe("SceneSwitcher DOM", () => {
  let parent: HTMLElement;
  const firstScene = makeScene({
    id: "first-scene",
    title: "First Scene",
    world_kind: "sl1_scenario",
    status: "ready",
  });
  const secondScene = makeScene({
    id: "second-scene",
    title: "Second Scene",
    world_kind: "transit_loop",
    status: "ready",
  });
  const thirdScene = makeScene({
    id: "third-scene",
    title: "Third Scene",
    world_kind: "transit_loop",
    status: "ready",
  });

  beforeEach(() => {
    vi.useFakeTimers();
    parent = document.createElement("div");
    document.body.appendChild(parent);
  });

  afterEach(() => {
    vi.useRealTimers();
    document.body.replaceChildren();
    const hostileWindow = window as Window & { __switcherHostile?: boolean };
    delete hostileWindow.__switcherHostile;
  });

  it("renders prev, next, and gallery buttons", () => {
    new SceneSwitcher(parent, [firstScene], {
      onPrev: () => {},
      onNext: () => {},
      onGallery: () => {},
    });

    const buttons = parent.querySelectorAll<HTMLButtonElement>("#simetro-switcher button");
    expect(buttons).toHaveLength(3);
    expect(buttons[0]?.textContent).toBe("◀");
    expect(buttons[1]?.textContent).toBe("▶");
    expect(buttons[2]?.title).toBe("Gallery");
  });

  it("show and hide toggle display", () => {
    const switcher = new SceneSwitcher(parent, [firstScene], {
      onPrev: () => {},
      onNext: () => {},
      onGallery: () => {},
    });
    const root = parent.querySelector<HTMLElement>("#simetro-switcher");

    switcher.show();
    expect(root?.style.display).toBe("flex");

    switcher.hide();
    expect(root?.style.display).toBe("none");
  });

  it("setSelected updates the title text", () => {
    const switcher = new SceneSwitcher(parent, [firstScene, secondScene], {
      onPrev: () => {},
      onNext: () => {},
      onGallery: () => {},
    });

    switcher.setSelected(secondScene.id);

    expect(parent.querySelector("#simetro-switcher span")?.textContent).toBe("Second Scene");
  });

  it("getAdjacentId returns the previous scene id and wraps around", () => {
    const switcher = new SceneSwitcher(parent, [firstScene, secondScene, thirdScene], {
      onPrev: () => {},
      onNext: () => {},
      onGallery: () => {},
    });

    switcher.setSelected(firstScene.id);
    expect(switcher.getAdjacentId("prev")).toBe(thirdScene.id);

    switcher.setSelected(secondScene.id);
    expect(switcher.getAdjacentId("prev")).toBe(firstScene.id);
  });

  it("getAdjacentId returns the next scene id and wraps around", () => {
    const switcher = new SceneSwitcher(parent, [firstScene, secondScene, thirdScene], {
      onPrev: () => {},
      onNext: () => {},
      onGallery: () => {},
    });

    switcher.setSelected(thirdScene.id);
    expect(switcher.getAdjacentId("next")).toBe(firstScene.id);

    switcher.setSelected(secondScene.id);
    expect(switcher.getAdjacentId("next")).toBe(thirdScene.id);
  });

  it("returns null for adjacent ids with an empty catalog", () => {
    const switcher = new SceneSwitcher(parent, [], {
      onPrev: () => {},
      onNext: () => {},
      onGallery: () => {},
    });

    expect(switcher.getAdjacentId("prev")).toBeNull();
    expect(switcher.getAdjacentId("next")).toBeNull();
  });

  it("fires callbacks when buttons are clicked", () => {
    const handler = {
      onPrev: vi.fn(),
      onNext: vi.fn(),
      onGallery: vi.fn(),
    };
    new SceneSwitcher(parent, [firstScene], handler);

    const buttons = parent.querySelectorAll<HTMLButtonElement>("#simetro-switcher button");
    buttons[0]?.click();
    buttons[1]?.click();
    buttons[2]?.click();

    expect(handler.onPrev).toHaveBeenCalledOnce();
    expect(handler.onNext).toHaveBeenCalledOnce();
    expect(handler.onGallery).toHaveBeenCalledOnce();
  });

  it("renders the title via textContent without executing hostile markup", () => {
    const hostileScene = makeScene({
      id: "hostile-switcher",
      title: "<script>window.__switcherHostile=true</script>",
      world_kind: "sl1_scenario",
      status: "ready",
    });
    const switcher = new SceneSwitcher(parent, [hostileScene], {
      onPrev: () => {},
      onNext: () => {},
      onGallery: () => {},
    });

    switcher.setSelected(hostileScene.id);

    const hostileWindow = window as Window & { __switcherHostile?: boolean };
    expect(parent.querySelector("script")).toBeNull();
    expect(hostileWindow.__switcherHostile).toBeUndefined();
    expect(parent.querySelector("#simetro-switcher span")?.textContent).toBe(
      "<script>window.__switcherHostile=true</script>"
    );
  });
});
