import { describe, it, expect, vi } from "vitest";
import { SCENE_CATALOG, type SceneCatalogEntry } from "../../catalog/scenes";
import { SceneBrowser, type SceneSelectIntent } from "../../ui/scene_browser";

describe("SceneBrowser", () => {
  it("renders the current demo as the first selectable scene without exposing paths", () => {
    const parent = document.createElement("div");
    const handler = vi.fn();
    const browser = new SceneBrowser(parent, SCENE_CATALOG, handler);

    const root = browser.__testRoot();
    const list = root.querySelector<HTMLElement>("#simetro-scene-list");
    const first = list?.querySelector<HTMLButtonElement>("button");
    expect(first?.id).toBe("simetro-scene-demo-paths");
    expect(first?.textContent).toContain("Demo Paths");
    expect(root.textContent).not.toContain("games/demo-paths.json");
    expect(first?.getAttribute("aria-pressed")).toBe("true");
  });

  it("emits scene_id only when a scene is selected", () => {
    const parent = document.createElement("div");
    const intents: SceneSelectIntent[] = [];
    new SceneBrowser(parent, SCENE_CATALOG, (intent) => intents.push(intent));

    parent.querySelector<HTMLButtonElement>("#simetro-scene-demo-paths")?.click();

    expect(intents).toEqual([{ kind: "SelectScene", scene_id: "demo-paths" }]);
    expect(Object.keys(intents[0] ?? {})).toEqual(["kind", "scene_id"]);
  });

  it("renders catalog strings with textContent", () => {
    const parent = document.createElement("div");
    const malicious = [
      {
        ...SCENE_CATALOG[0],
        id: "demo-paths",
        title: "<script>evil()</script>",
        subtitle: "<img src=x onerror=evil()>",
      },
    ] as const satisfies readonly SceneCatalogEntry[];

    new SceneBrowser(parent, malicious, () => {});

    expect(parent.textContent).toContain("<script>evil()</script>");
    expect(parent.querySelector("script")).toBeNull();
    expect(parent.querySelector("img")).toBeNull();
  });

  it("updates the selected scene affordance", () => {
    const parent = document.createElement("div");
    const browser = new SceneBrowser(parent, SCENE_CATALOG, () => {});
    const button = parent.querySelector<HTMLButtonElement>("#simetro-scene-demo-paths");

    browser.setSelected(null);
    expect(button?.getAttribute("aria-pressed")).toBe("false");

    browser.setSelected("demo-paths");
    expect(button?.getAttribute("aria-pressed")).toBe("true");
  });

  it("renders the panel expanded by default with the list visible", () => {
    const parent = document.createElement("div");
    const browser = new SceneBrowser(parent, SCENE_CATALOG, () => {});

    const toggle = parent.querySelector<HTMLButtonElement>("#simetro-scene-toggle");
    const list = parent.querySelector<HTMLElement>("#simetro-scene-list");

    expect(toggle?.getAttribute("aria-expanded")).toBe("true");
    expect(toggle?.getAttribute("aria-controls")).toBe("simetro-scene-list");
    expect(list?.style.display).toBe("flex");
    expect(browser.isCollapsed()).toBe(false);
  });

  it("toggles collapse state when the heading button is clicked", () => {
    const parent = document.createElement("div");
    const browser = new SceneBrowser(parent, SCENE_CATALOG, () => {});

    const toggle = parent.querySelector<HTMLButtonElement>("#simetro-scene-toggle");
    const list = parent.querySelector<HTMLElement>("#simetro-scene-list");
    expect(toggle).not.toBeNull();

    toggle?.click();
    expect(browser.isCollapsed()).toBe(true);
    expect(toggle?.getAttribute("aria-expanded")).toBe("false");
    expect(list?.style.display).toBe("none");

    toggle?.click();
    expect(browser.isCollapsed()).toBe(false);
    expect(toggle?.getAttribute("aria-expanded")).toBe("true");
    expect(list?.style.display).toBe("flex");
  });

  it("exposes a programmatic setCollapsed for callers that want to persist state", () => {
    const parent = document.createElement("div");
    const browser = new SceneBrowser(parent, SCENE_CATALOG, () => {});

    browser.setCollapsed(true);
    expect(browser.isCollapsed()).toBe(true);
    const list = parent.querySelector<HTMLElement>("#simetro-scene-list");
    expect(list?.style.display).toBe("none");

    browser.setCollapsed(false);
    expect(browser.isCollapsed()).toBe(false);
    expect(list?.style.display).toBe("flex");
  });

  it("makes the list scrollable and caps the panel to viewport height", () => {
    const parent = document.createElement("div");
    new SceneBrowser(parent, SCENE_CATALOG, () => {});

    const root = parent.querySelector<HTMLElement>("#simetro-scene-browser");
    const list = parent.querySelector<HTMLElement>("#simetro-scene-list");

    // The panel itself must not be allowed to grow past the viewport,
    // otherwise the inner list has no overflow target to scroll into.
    expect(root?.style.maxHeight).toContain("100vh");
    // The list scrolls vertically when content exceeds available height.
    expect(list?.style.overflowY).toBe("auto");
    // Flex shrinking required so the list yields height to the toggle
    // (jsdom serializes the zero-length value without a unit).
    expect(list?.style.minHeight).toBe("0");
  });

  it("does not emit a select intent when only the toggle is clicked", () => {
    const parent = document.createElement("div");
    const intents: SceneSelectIntent[] = [];
    new SceneBrowser(parent, SCENE_CATALOG, (intent) => intents.push(intent));

    parent.querySelector<HTMLButtonElement>("#simetro-scene-toggle")?.click();
    parent.querySelector<HTMLButtonElement>("#simetro-scene-toggle")?.click();

    expect(intents).toEqual([]);
  });

  it("updates the indicator glyph to reflect collapse state", () => {
    const parent = document.createElement("div");
    const browser = new SceneBrowser(parent, SCENE_CATALOG, () => {});

    const indicator = parent.querySelector<HTMLElement>("[data-role='scene-toggle-indicator']");
    expect(indicator?.textContent).toBe("▾");
    // Indicator is decorative — must be hidden from assistive tech to
    // avoid double-announcing the toggle state already on aria-expanded.
    expect(indicator?.getAttribute("aria-hidden")).toBe("true");

    browser.setCollapsed(true);
    expect(indicator?.textContent).toBe("▸");

    browser.setCollapsed(false);
    expect(indicator?.textContent).toBe("▾");
  });
});
