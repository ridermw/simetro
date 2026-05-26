// frontend/src/tests/unit/gallery.test.ts
//
// Gallery catalog and filter logic tests.

import { describe, test, expect } from "vitest";
import { SCENE_CATALOG } from "../../catalog/scenes";

describe("scene catalog", () => {
  test("no SL1 scenes remain in draft status", () => {
    const draftSl1 = SCENE_CATALOG.filter(
      (s) => s.world_kind === "sl1_scenario" && s.status === "draft"
    );
    expect(draftSl1).toEqual([]);
  });

  test("all 59 scenes are present", () => {
    expect(SCENE_CATALOG.length).toBe(59);
  });

  test("all SL1 scenes are ready", () => {
    const sl1Scenes = SCENE_CATALOG.filter(
      (s) => s.world_kind === "sl1_scenario"
    );
    expect(sl1Scenes.length).toBe(28);
    for (const scene of sl1Scenes) {
      expect(scene.status).toBe("ready");
    }
  });

  test("ready scenes include both world kinds", () => {
    const ready = SCENE_CATALOG.filter((s) => s.status === "ready");
    const kinds = new Set(ready.map((s) => s.world_kind));
    expect(kinds.has("sl1_scenario")).toBe(true);
    expect(kinds.has("transit_loop")).toBe(true);
  });
});

describe("gallery filter logic", () => {
  const ready = SCENE_CATALOG.filter((s) => s.status === "ready");

  test("filter by sl1_scenario returns only SL1", () => {
    const filtered = ready.filter((s) => s.world_kind === "sl1_scenario");
    expect(filtered.length).toBe(28);
    for (const s of filtered) {
      expect(s.world_kind).toBe("sl1_scenario");
    }
  });

  test("filter by transit_loop returns only transit", () => {
    const filtered = ready.filter((s) => s.world_kind === "transit_loop");
    expect(filtered.length).toBe(31);
  });

  test("filter by difficulty=hard returns subset", () => {
    const filtered = ready.filter((s) => s.difficulty === "hard");
    expect(filtered.length).toBeGreaterThan(0);
    expect(filtered.length).toBeLessThan(ready.length);
  });
});

describe("catalog/file alignment", () => {
  test("catalog ids are unique", () => {
    const ids = SCENE_CATALOG.map((s) => s.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  test("every catalog entry has scene_path matching id convention", () => {
    for (const scene of SCENE_CATALOG) {
      expect(scene.scene_path).toBe(`games/${scene.id}.json`);
    }
  });
});
