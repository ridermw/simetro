import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SCENE_CATALOG } from "../../catalog/scenes";
import { GalleryView } from "../../ui/gallery_view";
import { SceneSwitcher } from "../../ui/scene_switcher";

const READY_SCENES = SCENE_CATALOG.filter((scene) => scene.status === "ready");

class FakeIntersectionObserver {
  observe(): void {}
  disconnect(): void {}
}

describe("GalleryView", () => {
  let originalIntersectionObserver: typeof globalThis.IntersectionObserver | undefined;

  beforeEach(() => {
    originalIntersectionObserver = globalThis.IntersectionObserver;
    Object.defineProperty(globalThis, "IntersectionObserver", {
      configurable: true,
      writable: true,
      value: FakeIntersectionObserver,
    });
  });

  afterEach(() => {
    if (originalIntersectionObserver === undefined) {
      delete (globalThis as { IntersectionObserver?: unknown }).IntersectionObserver;
      return;
    }
    Object.defineProperty(globalThis, "IntersectionObserver", {
      configurable: true,
      writable: true,
      value: originalIntersectionObserver,
    });
  });

  it("calls onSelect with a SelectScene intent", () => {
    const parent = document.createElement("div");
    const onSelect = vi.fn();
    const scenes = READY_SCENES.slice(0, 3);
    const gallery = new GalleryView(parent, scenes, onSelect);

    gallery.show();
    parent.querySelector<HTMLButtonElement>(`button[data-scene-id="${scenes[1]?.id ?? ""}"]`)?.click();

    expect(onSelect).toHaveBeenCalledWith({
      kind: "SelectScene",
      scene_id: scenes[1]?.id ?? "",
    });
  });
});

describe("SceneSwitcher", () => {
  it("invokes no-arg callbacks and leaves adjacent id lookup to callers", () => {
    const parent = document.createElement("div");
    const scenes = READY_SCENES.slice(0, 3);
    const handler = {
      onPrev: vi.fn(),
      onNext: vi.fn(),
      onGallery: vi.fn(),
    };
    const switcher = new SceneSwitcher(parent, scenes, handler);

    switcher.setSelected(scenes[1]?.id ?? "");
    expect(switcher.getAdjacentId("prev")).toBe(scenes[0]?.id ?? null);
    expect(switcher.getAdjacentId("next")).toBe(scenes[2]?.id ?? null);

    switcher.show();
    const buttons = parent.querySelectorAll<HTMLButtonElement>("#simetro-switcher button");
    buttons[0]?.click();
    buttons[1]?.click();
    buttons[2]?.click();

    expect(handler.onPrev).toHaveBeenCalledWith();
    expect(handler.onNext).toHaveBeenCalledWith();
    expect(handler.onGallery).toHaveBeenCalledTimes(1);
  });
});
