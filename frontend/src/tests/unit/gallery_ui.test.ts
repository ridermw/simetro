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

  it("calls onSelect with the clicked scene id", () => {
    const parent = document.createElement("div");
    const onSelect = vi.fn();
    const scenes = READY_SCENES.slice(0, 3);
    const gallery = new GalleryView(parent, scenes, onSelect);

    gallery.show();
    parent.querySelector<HTMLButtonElement>(`button[data-scene-id="${scenes[1]?.id ?? ""}"]`)?.click();

    expect(onSelect).toHaveBeenCalledWith(scenes[1]?.id);
  });
});

describe("SceneSwitcher", () => {
  it("passes adjacent scene ids to prev/next callbacks", () => {
    const parent = document.createElement("div");
    const scenes = READY_SCENES.slice(0, 3);
    const handler = {
      onPrev: vi.fn<(sceneId: string) => void>(),
      onNext: vi.fn<(sceneId: string) => void>(),
      onGallery: vi.fn(),
    };
    const switcher = new SceneSwitcher(parent, scenes, handler);

    switcher.setSelected(scenes[1]?.id ?? "");
    switcher.show();
    const buttons = parent.querySelectorAll<HTMLButtonElement>("#simetro-switcher button");
    buttons[0]?.click();
    buttons[1]?.click();
    buttons[2]?.click();

    expect(handler.onPrev).toHaveBeenCalledWith(scenes[0]?.id);
    expect(handler.onNext).toHaveBeenCalledWith(scenes[2]?.id);
    expect(handler.onGallery).toHaveBeenCalledTimes(1);
  });
});
