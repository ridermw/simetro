import { describe, expect, it, vi } from "vitest";
import {
  applySceneStatic,
  preserveSceneAfterSwitchFailure,
  resetLocalSceneState,
  shouldResetSceneImmediatelyForControl,
  type SceneSwitchState,
} from "../../app/scene_switch";
import type { MoverState, SnapshotPayload, StaticPayload } from "../../protocol/messages";
import { AnimationEngine } from "../../renderer/animation_engine";
import { DEFAULT_THEME } from "../../renderer/theme";
import { SnapshotBuffer } from "../../store/snapshots";

function scene(name: string, nodeId: number): StaticPayload {
  return {
    name,
    palette: ["#000", "#fff", "#7aa2f7"],
    background_index: 0,
    nodes: [{ id: nodeId, pos: [100, 100], shape: "circle", color: 2 }],
    paths: [],
    node_names: { [nodeId]: name },
    path_names: {},
    mover_names: { 7: "mover" },
  };
}

function snap(tick: number, x: number): SnapshotPayload {
  return {
    tick,
    movers: [{ id: 7, pos: [x, 100], on_path: 0, speed: 1 }],
  };
}

function stateWithScene(currentScene: StaticPayload): SceneSwitchState {
  const snapshots = new SnapshotBuffer();
  snapshots.push(snap(1, 10));
  snapshots.push(snap(2, 20));
  const animations = new AnimationEngine();
  animations.spawn({ kind: "path_pulsed", path: 1 }, 0);
  const moverScratch: MoverState[] = [{ id: 7, pos: [20, 100], on_path: 0, speed: 1 }];

  return {
    theme: DEFAULT_THEME,
    scene: currentScene,
    snapshots,
    animations,
    inspector: { clear: vi.fn() },
    hover: { clear: vi.fn(), setScene: vi.fn() },
    fault: { hide: vi.fn(), show: vi.fn() },
    warnings: { clear: vi.fn() },
    sl1: { reset: vi.fn() },
    lastSnapshotAt: 1234,
    moverScratch,
  };
}

describe("scene-switch invariants", () => {
  it("commits a successful switch by clearing scene-scoped runtime and UI first", () => {
    const nextScene = scene("gallery-world-b", 2);
    const state = stateWithScene(scene("gallery-world-a", 1));
    const renderer = { warm: vi.fn(), setScene: vi.fn() };

    applySceneStatic(state, renderer, nextScene);

    expect(state.scene).toBe(nextScene);
    expect(state.snapshots.current()).toBeNull();
    expect(state.snapshots.previous()).toBeNull();
    expect(state.lastSnapshotAt).toBe(0);
    expect(state.moverScratch).toHaveLength(0);
    expect(state.animations.liveCount()).toBe(0);
    expect(state.inspector?.clear).toHaveBeenCalledTimes(1);
    expect(state.hover?.clear).toHaveBeenCalledTimes(1);
    expect(state.fault?.hide).toHaveBeenCalledTimes(1);
    expect(state.warnings?.clear).toHaveBeenCalledTimes(1);
  });

  it("updates scene metadata and renderer state exactly once per Static payload", () => {
    const nextScene = scene("gallery-world-c", 3);
    const state = stateWithScene(scene("gallery-world-a", 1));
    const renderer = { warm: vi.fn(), setScene: vi.fn() };

    applySceneStatic(state, renderer, nextScene);

    expect(renderer.warm).toHaveBeenCalledTimes(1);
    expect(renderer.setScene).toHaveBeenCalledTimes(1);
    expect(renderer.setScene).toHaveBeenCalledWith(nextScene);
    expect(state.hover?.setScene).toHaveBeenCalledTimes(1);
    expect(state.hover?.setScene).toHaveBeenCalledWith(nextScene);
  });

  it("keeps browser mock reload local and lets Tauri wait for Static before reset", () => {
    expect(shouldResetSceneImmediatelyForControl({ kind: "Reload" }, false)).toBe(true);
    expect(shouldResetSceneImmediatelyForControl({ kind: "Reload" }, true)).toBe(false);
    expect(shouldResetSceneImmediatelyForControl({ kind: "SetSpeed", factor: 2 }, false)).toBe(
      false
    );
  });

  it("preserves the previous running scene when a switch fails", () => {
    const currentScene = scene("gallery-world-a", 1);
    const state = stateWithScene(currentScene);
    const beforeSnapshot = state.snapshots.current();

    preserveSceneAfterSwitchFailure(state, {
      kind: "load_error",
      message: "bad scene",
      line: 1,
      col: 2,
    });

    expect(state.scene).toBe(currentScene);
    expect(state.snapshots.current()).toBe(beforeSnapshot);
    expect(state.lastSnapshotAt).toBe(1234);
    expect(state.fault?.show).toHaveBeenCalledTimes(1);
  });

  it("can reset browser-only mock state without loading an arbitrary path", () => {
    const state = stateWithScene(scene("demo-paths", 1));

    resetLocalSceneState(state);

    expect(state.scene).toBeNull();
    expect(state.snapshots.current()).toBeNull();
    expect(state.inspector?.clear).toHaveBeenCalledTimes(1);
    expect(state.hover?.clear).toHaveBeenCalledTimes(1);
  });
});
