import { describe, it, expect } from "vitest";
import {
  SCENE_CATALOG,
  catalogConventionErrors,
  findSceneById,
  isLocalScenePath,
  scenePathForId,
  type SceneCatalogEntry,
} from "../../catalog/scenes";

const REQUIRED_FIELDS = [
  "id",
  "title",
  "subtitle",
  "world_kind",
  "scene_path",
  "difficulty",
  "palette_name",
  "rules_summary",
  "visual_notes",
  "screenshot_target",
  "status",
] as const satisfies readonly (keyof SceneCatalogEntry)[];

const REQUIRED_STRING_FIELDS = [
  "id",
  "title",
  "subtitle",
  "world_kind",
  "scene_path",
  "difficulty",
  "palette_name",
  "status",
] as const satisfies readonly (keyof SceneCatalogEntry)[];

describe("scene catalog", () => {
  it("defines the first gallery slice with required static metadata", () => {
    const demo = findSceneById("demo-paths");
    expect(demo).toBeDefined();
    if (demo === undefined) return;

    for (const field of REQUIRED_FIELDS) {
      expect(demo[field]).toBeDefined();
    }

    expect(demo).toMatchObject({
      id: "demo-paths",
      title: "Demo Paths",
      world_kind: "transit_loop",
      scene_path: "games/demo-paths.json",
      difficulty: "intro",
      palette_name: "simetro_dark",
      status: "ready",
    });
  });

  it("keeps every catalog entry complete and reviewable", () => {
    expect(SCENE_CATALOG.length).toBeGreaterThan(0);

    for (const scene of SCENE_CATALOG) {
      for (const field of REQUIRED_FIELDS) {
        expect(scene[field]).toBeDefined();
      }

      for (const field of REQUIRED_STRING_FIELDS) {
        expect(scene[field]).toEqual(expect.any(String));
        expect(String(scene[field]).trim()).not.toHaveLength(0);
      }

      expect(scene.rules_summary.length).toBeGreaterThan(0);
      expect(scene.rules_summary.every((line) => line.trim().length > 0)).toBe(true);
      expect(scene.visual_notes.length).toBeGreaterThan(0);
      expect(scene.visual_notes.every((line) => line.trim().length > 0)).toBe(true);
      expect(scene.scene_path).toMatch(/^games\/.+\.json$/);
    }
  });

  it("keeps scene ids unique and scene paths local", () => {
    expect(catalogConventionErrors(SCENE_CATALOG)).toEqual([]);

    for (const scene of SCENE_CATALOG) {
      expect(scene.scene_path).toBe(scenePathForId(scene.id));
      expect(isLocalScenePath(scene.scene_path)).toBe(true);
    }

    expect(isLocalScenePath("https://example.com/world.json")).toBe(false);
    expect(isLocalScenePath("/games/demo-paths.json")).toBe(false);
    expect(isLocalScenePath("games/../secrets.json")).toBe(false);
    expect(isLocalScenePath("games/nested/demo-paths.json")).toBe(false);
  });

  it("describes screenshot capture without adding a live provider dependency", () => {
    const demo = findSceneById("demo-paths");
    expect(demo?.screenshot_target).toEqual({
      route: "/",
      selector: "#scene",
      wait_for: "first_frame",
      width: 960,
      height: 540,
    });
  });
});
