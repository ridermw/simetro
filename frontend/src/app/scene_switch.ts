import type { FaultPayload, MoverState, StaticPayload } from "../protocol/messages";
import { AnimationEngine } from "../renderer/animation_engine";
import { SnapshotBuffer } from "../store/snapshots";
import { themeFromStatic, type Theme } from "../renderer/theme";
import { synthesizeSl1Geometry } from "../renderer/sl1_synth";
import type { ControlIntent } from "../ui/controls";

export interface SceneSwitchState {
  theme: Theme;
  scene: StaticPayload | null;
  snapshots: SnapshotBuffer;
  animations: AnimationEngine;
  inspector: { clear(): void } | null;
  hover: { clear(): void; setScene(scene: StaticPayload | null): void } | null;
  fault: { hide(): void; show(fault: FaultPayload): void } | null;
  warnings: { clear(): void } | null;
  sl1: { reset(): void } | null;
  lastSnapshotAt: number;
  moverScratch: MoverState[];
}

export interface SceneRenderer {
  warm(theme: Theme): void;
  setScene(scene: StaticPayload): void;
}

export function shouldResetSceneImmediatelyForControl(
  intent: ControlIntent,
  tauriRuntime: boolean
): boolean {
  return intent.kind === "Reload" && !tauriRuntime;
}

export function resetLocalSceneState(state: SceneSwitchState): void {
  clearSceneScopedUi(state);
  clearSceneScopedRuntime(state);
  state.scene = null;
}

export function applySceneStatic(
  state: SceneSwitchState,
  renderer: SceneRenderer,
  scene: StaticPayload
): void {
  clearSceneScopedUi(state);
  clearSceneScopedRuntime(state);
  // Project SL1 places/links into legacy nodes/paths so the renderer
  // can draw SL1 scenes that don't (yet) carry a transport runtime.
  const projected = synthesizeSl1Geometry(scene);
  state.scene = projected;
  state.theme = themeFromStatic(projected);
  renderer.warm(state.theme);
  renderer.setScene(projected);
  state.hover?.setScene(projected);
}

export function preserveSceneAfterSwitchFailure(
  state: SceneSwitchState,
  fault: FaultPayload
): void {
  state.fault?.show(fault);
}

function clearSceneScopedUi(state: SceneSwitchState): void {
  state.inspector?.clear();
  state.hover?.clear();
  state.fault?.hide();
  state.warnings?.clear();
  state.sl1?.reset();
}

function clearSceneScopedRuntime(state: SceneSwitchState): void {
  state.snapshots = new SnapshotBuffer();
  state.animations.clear();
  state.lastSnapshotAt = 0;
  state.moverScratch.length = 0;
}
