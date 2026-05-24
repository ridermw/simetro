import { findSceneById, type SceneCatalogId } from "../catalog/scenes";

export type TauriInvoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

export class UnknownSceneError extends Error {
  constructor(scene_id: string) {
    super(`Unknown scene_id: ${scene_id}`);
    this.name = "UnknownSceneError";
  }
}

export function requireCatalogScene(scene_id: string): SceneCatalogId {
  if (findSceneById(scene_id) === undefined) {
    throw new UnknownSceneError(scene_id);
  }
  return scene_id as SceneCatalogId;
}

export async function invokeSetScene(invoke: TauriInvoke, scene_id: string): Promise<void> {
  const checkedSceneId = requireCatalogScene(scene_id);
  await invoke("set_scene", { scene_id: checkedSceneId });
}
