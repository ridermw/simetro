import { describe, it, expect, vi } from "vitest";
import { SCENE_CATALOG, type SceneCatalogEntry } from "../../catalog/scenes";
import { SceneBrowser, type SceneSelectIntent } from "../../ui/scene_browser";

describe("SceneBrowser", () => {
  it("renders the current demo as the first selectable scene without exposing paths", () => {
    const parent = document.createElement("div");
    const handler = vi.fn();
    const browser = new SceneBrowser(parent, SCENE_CATALOG, handler);

    const root = browser.__testRoot();
    const first = root.querySelector<HTMLButtonElement>("button");
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
});
