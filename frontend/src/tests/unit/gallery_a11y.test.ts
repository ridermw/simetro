// frontend/src/tests/unit/gallery_a11y.test.ts
//
// Accessibility tests for gallery UI components.
// Verifies ARIA roles, labels, keyboard navigation, and focus indicators.

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { GalleryView, type SceneSelectIntent } from "../../ui/gallery_view";
import { GalleryCard } from "../../ui/gallery_card";
import { SceneSwitcher } from "../../ui/scene_switcher";
import { SCENE_CATALOG, type SceneCatalogEntry } from "../../catalog/scenes";

class FakeIntersectionObserver {
  observe(): void {}
  disconnect(): void {}
  unobserve(): void {}
}

let originalIO: typeof globalThis.IntersectionObserver | undefined;

beforeEach(() => {
  originalIO = globalThis.IntersectionObserver;
  Object.defineProperty(globalThis, "IntersectionObserver", {
    configurable: true,
    writable: true,
    value: FakeIntersectionObserver,
  });
});

afterEach(() => {
  if (originalIO !== undefined) {
    Object.defineProperty(globalThis, "IntersectionObserver", {
      configurable: true,
      writable: true,
      value: originalIO,
    });
  }
});

function makeScene(id: string, overrides: Partial<SceneCatalogEntry> = {}): SceneCatalogEntry {
  const base = SCENE_CATALOG[0]!;
  return { ...base, id, ...overrides };
}

describe("GalleryView accessibility", () => {
  let parent: HTMLElement;
  beforeEach(() => {
    parent = document.createElement("div");
    document.body.appendChild(parent);
  });

  it("root has region role and aria-label", () => {
    new GalleryView(parent, [makeScene("s1")], () => {});
    const root = parent.querySelector("#simetro-gallery");
    expect(root?.getAttribute("role")).toBe("region");
    expect(root?.getAttribute("aria-label")).toBe("Scene gallery");
  });

  it("grid has group role with descriptive label (not list — buttons keep native role)", () => {
    new GalleryView(parent, [makeScene("s1")], () => {});
    const grid = parent.querySelector("[role='group']");
    expect(grid).not.toBeNull();
    expect(grid?.getAttribute("aria-label")).toBe("Available scenes");
    // No role="list" — that would force role="listitem" on buttons
    // and override their native button role.
    expect(parent.querySelector("[role='list']")).toBeNull();
  });

  it("filter chips form a radio group", () => {
    new GalleryView(parent, [makeScene("s1")], () => {});
    const chips = parent.querySelector("[role='radiogroup']");
    expect(chips).not.toBeNull();
    expect(chips?.getAttribute("aria-label")).toBe("Filter scenes by kind");
    const radios = chips?.querySelectorAll("[role='radio']");
    expect(radios?.length).toBe(3);
  });

  it("initial filter has aria-checked=true on 'All' chip", () => {
    new GalleryView(parent, [makeScene("s1")], () => {});
    const all = parent.querySelector("[data-filter='all']");
    expect(all?.getAttribute("aria-checked")).toBe("true");
    const sl1 = parent.querySelector("[data-filter='sl1_scenario']");
    expect(sl1?.getAttribute("aria-checked")).toBe("false");
  });

  it("clicking a chip updates aria-checked", () => {
    new GalleryView(parent, [makeScene("s1")], () => {});
    const sl1Chip = parent.querySelector<HTMLButtonElement>("[data-filter='sl1_scenario']");
    sl1Chip?.click();
    expect(sl1Chip?.getAttribute("aria-checked")).toBe("true");
    expect(parent.querySelector("[data-filter='all']")?.getAttribute("aria-checked")).toBe("false");
  });

  it("ArrowRight moves focus to the next card", () => {
    const scenes = [makeScene("a"), makeScene("b"), makeScene("c")];
    new GalleryView(parent, scenes, () => {});
    // Show the gallery so cards are visible (GalleryView starts hidden).
    const gallery = parent.querySelector<HTMLElement>("#simetro-gallery");
    if (gallery) gallery.style.display = "block";

    const first = parent.querySelector<HTMLButtonElement>("[data-scene-id='a']");
    first?.focus();
    const grid = parent.querySelector<HTMLElement>("[role='group']");
    grid?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));

    const focused = document.activeElement;
    expect((focused as HTMLElement)?.dataset.sceneId).toBe("b");
  });

  it("ArrowLeft from first card stays on first card", () => {
    const scenes = [makeScene("a"), makeScene("b")];
    new GalleryView(parent, scenes, () => {});
    const gallery = parent.querySelector<HTMLElement>("#simetro-gallery");
    if (gallery) gallery.style.display = "block";

    const first = parent.querySelector<HTMLButtonElement>("[data-scene-id='a']");
    first?.focus();
    const grid = parent.querySelector<HTMLElement>("[role='group']");
    grid?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowLeft", bubbles: true }));

    expect((document.activeElement as HTMLElement)?.dataset.sceneId).toBe("a");
  });

  it("Home key focuses the first card", () => {
    const scenes = [makeScene("a"), makeScene("b"), makeScene("c")];
    new GalleryView(parent, scenes, () => {});
    const gallery = parent.querySelector<HTMLElement>("#simetro-gallery");
    if (gallery) gallery.style.display = "block";

    const last = parent.querySelector<HTMLButtonElement>("[data-scene-id='c']");
    last?.focus();
    const grid = parent.querySelector<HTMLElement>("[role='group']");
    grid?.dispatchEvent(new KeyboardEvent("keydown", { key: "Home", bubbles: true }));

    expect((document.activeElement as HTMLElement)?.dataset.sceneId).toBe("a");
  });

  it("End key focuses the last card", () => {
    const scenes = [makeScene("a"), makeScene("b"), makeScene("c")];
    new GalleryView(parent, scenes, () => {});
    const gallery = parent.querySelector<HTMLElement>("#simetro-gallery");
    if (gallery) gallery.style.display = "block";

    const first = parent.querySelector<HTMLButtonElement>("[data-scene-id='a']");
    first?.focus();
    const grid = parent.querySelector<HTMLElement>("[role='group']");
    grid?.dispatchEvent(new KeyboardEvent("keydown", { key: "End", bubbles: true }));

    expect((document.activeElement as HTMLElement)?.dataset.sceneId).toBe("c");
  });

  it("Arrow key without focus focuses the first card", () => {
    const scenes = [makeScene("a"), makeScene("b")];
    new GalleryView(parent, scenes, () => {});
    const gallery = parent.querySelector<HTMLElement>("#simetro-gallery");
    if (gallery) gallery.style.display = "block";

    (document.activeElement as HTMLElement | null)?.blur();
    const grid = parent.querySelector<HTMLElement>("[role='group']");
    grid?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));

    expect((document.activeElement as HTMLElement)?.dataset.sceneId).toBe("a");
  });

  it("non-arrow keys do not change focus", () => {
    const scenes = [makeScene("a"), makeScene("b")];
    new GalleryView(parent, scenes, () => {});
    const gallery = parent.querySelector<HTMLElement>("#simetro-gallery");
    if (gallery) gallery.style.display = "block";

    const first = parent.querySelector<HTMLButtonElement>("[data-scene-id='a']");
    first?.focus();
    const grid = parent.querySelector<HTMLElement>("[role='group']");
    grid?.dispatchEvent(new KeyboardEvent("keydown", { key: "a", bubbles: true }));

    expect((document.activeElement as HTMLElement)?.dataset.sceneId).toBe("a");
  });
});

describe("GalleryCard accessibility", () => {
  let parent: HTMLElement;
  beforeEach(() => {
    parent = document.createElement("div");
    document.body.appendChild(parent);
  });

  it("card preserves native button role (no role override)", () => {
    const scene = makeScene("test", { title: "Test", subtitle: "Sub", difficulty: "medium" });
    const card = new GalleryCard(scene, () => {});
    parent.appendChild(card.element);
    // Should NOT have a role override — keeps native button role
    // so screen readers announce it as an actionable button.
    expect(card.element.tagName).toBe("BUTTON");
    expect(card.element.getAttribute("role")).toBeNull();
  });

  it("card has descriptive aria-label combining title + subtitle + difficulty", () => {
    const scene = makeScene("test", {
      title: "GPU Launch Week",
      subtitle: "Operate the launch",
      difficulty: "hard",
    });
    const card = new GalleryCard(scene, () => {});
    parent.appendChild(card.element);
    const label = card.element.getAttribute("aria-label");
    expect(label).toContain("GPU Launch Week");
    expect(label).toContain("Operate the launch");
    expect(label).toContain("hard");
  });

  it("focus event applies visible focus styles", () => {
    const card = new GalleryCard(makeScene("test"), () => {});
    parent.appendChild(card.element);
    card.element.dispatchEvent(new FocusEvent("focus"));
    expect(card.element.style.boxShadow).toContain("#58a6ff");
  });

  it("blur event removes visible focus styles", () => {
    const card = new GalleryCard(makeScene("test"), () => {});
    parent.appendChild(card.element);
    card.element.dispatchEvent(new FocusEvent("focus"));
    card.element.dispatchEvent(new FocusEvent("blur"));
    expect(card.element.style.boxShadow).toBe("none");
  });

  it("mouseleave does not clear focused border color", () => {
    const card = new GalleryCard(makeScene("test"), () => {});
    parent.appendChild(card.element);
    card.element.focus();
    card.element.dispatchEvent(new MouseEvent("mouseleave"));
    expect(card.element.style.borderColor).toBe("rgb(88, 166, 255)");
  });
});

describe("SceneSwitcher accessibility", () => {
  let parent: HTMLElement;
  beforeEach(() => {
    parent = document.createElement("div");
    document.body.appendChild(parent);
  });

  it("root has toolbar role and aria-label", () => {
    new SceneSwitcher(parent, [makeScene("a")], {
      onPrev: () => {},
      onNext: () => {},
      onGallery: () => {},
    });
    const root = parent.querySelector("#simetro-switcher");
    expect(root?.getAttribute("role")).toBe("toolbar");
    expect(root?.getAttribute("aria-label")).toBe("Scene navigation");
  });

  it("prev/next/gallery buttons have descriptive aria-labels", () => {
    new SceneSwitcher(parent, [makeScene("a")], {
      onPrev: () => {},
      onNext: () => {},
      onGallery: () => {},
    });
    const buttons = parent.querySelectorAll<HTMLButtonElement>("#simetro-switcher button");
    const labels = Array.from(buttons).map((b) => b.getAttribute("aria-label"));
    expect(labels).toContain("Previous scene");
    expect(labels).toContain("Next scene");
    expect(labels).toContain("Return to gallery");
  });

  it("title element is aria-live=polite for scene change announcements", () => {
    new SceneSwitcher(parent, [makeScene("a")], {
      onPrev: () => {},
      onNext: () => {},
      onGallery: () => {},
    });
    const title = parent.querySelector("#simetro-switcher span");
    expect(title?.getAttribute("aria-live")).toBe("polite");
  });

  it("clicking buttons fires the expected handlers", () => {
    const onPrev = vi.fn();
    const onNext = vi.fn();
    const onGallery = vi.fn();
    new SceneSwitcher(parent, [makeScene("a")], { onPrev, onNext, onGallery });
    const buttons = parent.querySelectorAll<HTMLButtonElement>("#simetro-switcher button");
    for (const b of Array.from(buttons)) {
      const label = b.getAttribute("aria-label");
      if (label === "Previous scene") b.click();
      else if (label === "Next scene") b.click();
      else if (label === "Return to gallery") b.click();
    }
    expect(onPrev).toHaveBeenCalledTimes(1);
    expect(onNext).toHaveBeenCalledTimes(1);
    expect(onGallery).toHaveBeenCalledTimes(1);
  });
});

describe("GalleryView onSelect contract", () => {
  it("emits SceneSelectIntent shape via onSelect callback", () => {
    const parent = document.createElement("div");
    const intents: SceneSelectIntent[] = [];
    new GalleryView(parent, [makeScene("foo")], (i) => intents.push(i));
    const card = parent.querySelector<HTMLButtonElement>("[data-scene-id='foo']");
    card?.click();
    expect(intents).toEqual([{ kind: "SelectScene", scene_id: "foo" }]);
  });
});
