import { describe, it, expect, vi } from "vitest";
import { UnknownSceneError, invokeSetScene, requireCatalogScene } from "../../app/scene_commands";

describe("scene command helpers", () => {
  it("accepts only registered catalog scene ids", () => {
    expect(requireCatalogScene("demo-paths")).toBe("demo-paths");
    expect(() => requireCatalogScene("../secrets")).toThrow(UnknownSceneError);
  });

  it("invokes Tauri set_scene with scene_id rather than a path", async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);

    await invokeSetScene(invoke, "demo-paths");

    expect(invoke).toHaveBeenCalledWith("set_scene", { scene_id: "demo-paths" });
  });

  it("does not invoke Tauri for unknown scene ids", async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);

    await expect(invokeSetScene(invoke, "games/demo-paths.json")).rejects.toThrow(
      UnknownSceneError
    );
    expect(invoke).not.toHaveBeenCalled();
  });
});
