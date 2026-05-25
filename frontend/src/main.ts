// frontend/src/main.ts
//
//   ┌────────────────────────────────────────────────────────────┐
//   │                       FRONTEND BOOT                        │
//   │                                                            │
//   │   transport ──▶ store (snapshots) ──▶ renderer             │
//   │       │                                          ▲         │
//   │       ▼                                          │ rAF     │
//   │     audio                                        │         │
//   │       │                                          │         │
//   │       ▼                                          │         │
//   │   inspector ◀───────────── agent reports ────────┘         │
//   │                                                            │
//   └────────────────────────────────────────────────────────────┘
//
// frontend shell contract. frontend integration adds:
//   * a real Renderer (Path2D batching, pre-allocated buckets)
//   * a SnapshotBuffer with two-snapshot mover interpolation
//   * a requestAnimationFrame loop (paused while tab is hidden,
//     tab-refocus invariant: jump-cut on refocus)
//
// Steps 18-21 plug animations, audio, inspector, and UI shell into
// the slots already present in this wiring.

import { MockTransport, sl1ModeFromLocation, type Transport } from "./transport/mock";
import { TauriTransport } from "./transport/tauri";
import type { MoverState, NodeView, SimMessage, StaticPayload } from "./protocol/messages";
import { Renderer } from "./renderer/canvas";
import { DEFAULT_THEME, type Theme } from "./renderer/theme";
import { AnimationEngine } from "./renderer/animation_engine";
import { SnapshotBuffer } from "./store/snapshots";
import { AudioEngine } from "./audio/engine";
import { fallbackArrivalTone, toneForShape } from "./audio/mappings";
import { InspectorPanel } from "./inspector/panel";
import { HoverTooltip } from "./inspector/hover";
import { ControlsBar, type ControlIntent } from "./ui/controls";
import { SceneBrowser, type SceneSelectIntent } from "./ui/scene_browser";
import { FaultOverlay, HeartbeatBadge, PerfOverlay, WarningStrip } from "./ui/overlays";
import {
  applySl1HudStatic,
  createSl1Hud,
  type Sl1Hud,
} from "./ui/sl1_hud";
import { SCENE_CATALOG, findSceneById } from "./catalog/scenes";
import { invokeSetScene } from "./app/scene_commands";
import {
  applySceneStatic,
  preserveSceneAfterSwitchFailure,
  resetLocalSceneState,
  shouldResetSceneImmediatelyForControl,
} from "./app/scene_switch";

interface AppState {
  theme: Theme;
  /** Cached most-recent Static message; nodes/paths/names live here. */
  scene: StaticPayload | null;
  snapshots: SnapshotBuffer;
  animations: AnimationEngine;
  audio: AudioEngine;
  inspector: InspectorPanel | null;
  hover: HoverTooltip | null;
  controls: ControlsBar | null;
  sceneBrowser: SceneBrowser | null;
  selectedSceneId: string | null;
  fault: FaultOverlay | null;
  warnings: WarningStrip | null;
  heartbeat: HeartbeatBadge | null;
  perf: PerfOverlay | null;
  sl1: Sl1Hud | null;
  paused: boolean;
  speedFactor: number;
  lastSnapshotAt: number;
  /** Estimated ms between snapshots; refined as we receive more. */
  snapshotPeriodMs: number;
  /** Scratch buffer reused every frame for interpolated movers. */
  moverScratch: MoverState[];
  rafHandle: number | null;
}

const TARGET_SNAPSHOT_HZ = 20; // wire-protocol contract — snapshots at 20Hz

function createAppState(): AppState {
  return {
    theme: DEFAULT_THEME,
    scene: null,
    snapshots: new SnapshotBuffer(),
    animations: new AnimationEngine(),
    audio: new AudioEngine(),
    inspector: null,
    hover: null,
    controls: null,
    sceneBrowser: null,
    selectedSceneId: SCENE_CATALOG[0]?.id ?? null,
    fault: null,
    warnings: null,
    heartbeat: null,
    perf: null,
    sl1: null,
    paused: false,
    speedFactor: 1,
    lastSnapshotAt: 0,
    snapshotPeriodMs: 1000 / TARGET_SNAPSHOT_HZ,
    moverScratch: [],
    rafHandle: null,
  };
}

function findArrivalNode(scene: StaticPayload, nodeId: number): NodeView | undefined {
  for (const n of scene.nodes) if (n.id === nodeId) return n;
  return undefined;
}

function resetSnapshotState(state: AppState): void {
  // Per review feedback: a Reload that leaves stale
  // snapshot data + lastSnapshotAt around makes the heartbeat lie
  // and the interpolator extrapolate against pre-reload movers.
  resetLocalSceneState(state);
}

function handleControl(intent: ControlIntent, state: AppState): void {
  if (isTauri()) {
    // Route control intents to the Rust engine driver via Tauri commands.
    void routeControlToTauri(intent, state);
    return;
  }

  // Browser-only mock fallback: local state manipulation.
  switch (intent.kind) {
    case "TogglePause":
      setPaused(state, !state.paused);
      break;
    case "Step":
      console.info("simetro: step requested (mock — no backend)");
      break;
    case "Reload":
      if (shouldResetSceneImmediatelyForControl(intent, false)) resetSnapshotState(state);
      console.info("simetro: reload requested (mock — no backend)");
      break;
    case "SetSpeed":
      state.speedFactor = intent.factor;
      console.info(`simetro: speed ${intent.factor}× requested (mock — no backend)`);
      break;
  }
}

function handleSceneSelect(intent: SceneSelectIntent, state: AppState): void {
  const scene = findSceneById(intent.scene_id);
  if (scene === undefined) {
    state.fault?.show({
      kind: "load_error",
      message: `Unknown scene_id: ${intent.scene_id}`,
      line: null,
      col: null,
    });
    return;
  }

  const previousSceneId = state.selectedSceneId;
  state.sceneBrowser?.setSelected(intent.scene_id);
  state.selectedSceneId = intent.scene_id;

  if (previousSceneId === intent.scene_id) return;

  if (isTauri()) {
    void routeSceneToTauri(intent.scene_id, previousSceneId, state);
  } else {
    console.info(`simetro: scene ${scene.id} selected (mock — no backend switch)`);
  }
}

async function routeSceneToTauri(
  scene_id: string,
  previousSceneId: string | null,
  state: AppState
): Promise<void> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invokeSetScene(invoke, scene_id);
  } catch (error) {
    state.selectedSceneId = previousSceneId;
    state.sceneBrowser?.setSelected(previousSceneId);
    state.fault?.show({
      kind: "load_error",
      message: `Failed to switch scene: ${errorMessage(error)}`,
      line: null,
      col: null,
    });
  }
}

async function routeControlToTauri(intent: ControlIntent, state: AppState): Promise<void> {
  try {
    // Dynamic import to avoid bundling @tauri-apps/api in browser builds.
    const { invoke } = await import("@tauri-apps/api/core");
    switch (intent.kind) {
      case "TogglePause":
        await invoke("cmd_toggle_pause");
        setPaused(state, !state.paused);
        break;
      case "Step":
        await invoke("cmd_step");
        break;
      case "Reload":
        await invoke("cmd_reload");
        break;
      case "SetSpeed":
        await invoke("cmd_set_speed", { factor: intent.factor });
        state.speedFactor = intent.factor;
        state.controls?.setSpeed(intent.factor);
        break;
    }
  } catch (error) {
    console.error("simetro: failed to route Tauri control", error);
    state.controls?.setPaused(state.paused);
    state.controls?.setSpeed(state.speedFactor);
    if (state.fault !== null) state.fault.show({ kind: "transport_lost" });
  }
}

function setPaused(state: AppState, paused: boolean): void {
  state.paused = paused;
  state.controls?.setPaused(paused);
  state.snapshots.markStale();
  if (!paused) state.lastSnapshotAt = 0;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function handleMessage(msg: SimMessage, state: AppState, renderer: Renderer): void {
  switch (msg.kind) {
    case "static": {
      applySceneStatic(state, renderer, msg.payload);
      if (state.sl1 !== null) {
        applySl1HudStatic(
          state.sl1,
          msg.payload.sl1_observability_dashboards,
          msg.payload.sl1_observability_alerts
        );
        // Static block carries no live state — clear status until the
        // first snapshot lands.
        state.sl1.status.update(undefined, undefined);
      }
      break;
    }
    case "snapshot": {
      const now = nowMs();
      if (state.paused) {
        state.lastSnapshotAt = now;
        state.snapshots.markStale();
        return;
      }
      if (state.lastSnapshotAt !== 0) {
        const dt = now - state.lastSnapshotAt;
        state.snapshotPeriodMs = Math.max(
          16,
          Math.min(500, state.snapshotPeriodMs * 0.8 + dt * 0.2)
        );
      }
      state.lastSnapshotAt = now;
      state.snapshots.push(msg.payload);
      if (state.hover !== null) {
        state.hover.setSnapshot(msg.payload);
      }
      if (state.sl1 !== null) {
        state.sl1.status.update(msg.payload.sl1_game_outcome, msg.payload.sl1_game_phase);
        if (msg.payload.sl1_dashboard_states !== undefined) {
          state.sl1.dashboards.updateStates(msg.payload.sl1_dashboard_states);
        }
        if (msg.payload.sl1_alert_states !== undefined) {
          state.sl1.alerts.updateStates(msg.payload.sl1_alert_states);
        }
      }
      break;
    }
    case "events": {
      if (state.paused) return;
      const now = nowMs();
      const scene = state.scene;
      for (const ev of msg.payload) {
        state.animations.spawn(ev, now);
        if (ev.kind === "mover_arrived" && scene !== null) {
          const node = findArrivalNode(scene, ev.at_node);
          const tone = node !== undefined ? toneForShape(node.shape) : fallbackArrivalTone();
          state.audio.play(tone);
        }
        if (state.sl1 !== null && ev.kind === "sl1_milestone_fired") {
          state.sl1.milestones.push({
            milestone_id: ev.milestone_id,
            label: ev.label,
            tick: ev.tick,
          });
        }
      }
      break;
    }
    case "agent_report":
      if (state.inspector !== null) {
        state.inspector.show(msg.payload);
      }
      return;
    case "fault":
      preserveSceneAfterSwitchFailure(state, msg.payload);
      return;
    case "warning":
      if (state.warnings !== null) state.warnings.push(msg.payload);
      return;
  }
}

function frame(state: AppState, renderer: Renderer): void {
  const cur = state.snapshots.current();
  const scene = state.scene;
  const now = nowMs();
  if (cur !== null && scene !== null) {
    const elapsed = now - state.lastSnapshotAt;
    const alpha =
      state.paused || state.snapshots.previous() === null
        ? 1
        : Math.max(0, Math.min(1, elapsed / state.snapshotPeriodMs));
    const movers = state.snapshots.interpolatedMovers(alpha, state.moverScratch);
    renderer.draw({
      theme: state.theme,
      scene,
      movers,
      overlay: state.paused
        ? undefined
        : (ctx) => state.animations.draw(ctx, now, state.theme, scene, cur),
    });
  }
  if (state.warnings !== null) state.warnings.tick(now);
  if (state.heartbeat !== null) state.heartbeat.update(state.lastSnapshotAt, now);
  if (state.perf !== null) state.perf.tick(now);
  state.rafHandle = requestAnimationFrame(() => frame(state, renderer));
}

function nowMs(): number {
  return typeof performance !== "undefined" ? performance.now() : Date.now();
}

/** Detect Tauri runtime. When present, use the real engine transport. */
function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function createTransport(): Transport {
  if (isTauri()) {
    return new TauriTransport();
  }
  // Non-Tauri runs use the browser-only MockTransport. SL1 demo mode
  // is opt-in via `?sl1demo=1` and only ever feeds the mock — there
  // is no live data path in this branch, so the query flag cannot
  // exfiltrate or spoof anything a real Tauri build would render.
  const search =
    typeof window !== "undefined" && window.location !== undefined
      ? window.location.search
      : undefined;
  return new MockTransport({ sl1Mode: sl1ModeFromLocation(search) });
}

function resize(canvas: HTMLCanvasElement): void {
  const dpr = (typeof window !== "undefined" && window.devicePixelRatio) || 1;
  const w = canvas.clientWidth;
  const h = canvas.clientHeight;
  canvas.width = Math.floor(w * dpr);
  canvas.height = Math.floor(h * dpr);
}

function boot(): void {
  const canvas = document.getElementById("scene");
  if (!(canvas instanceof HTMLCanvasElement)) {
    console.error("scene canvas missing");
    return;
  }
  resize(canvas);

  const renderer = new Renderer(canvas);
  renderer.warm(DEFAULT_THEME);
  const state = createAppState();

  const appRoot = document.getElementById("app");
  if (appRoot !== null) {
    state.inspector = new InspectorPanel(appRoot);
    state.hover = new HoverTooltip(appRoot);
    state.hover.attach(canvas);
    state.fault = new FaultOverlay(appRoot);
    state.warnings = new WarningStrip(appRoot);
    state.heartbeat = new HeartbeatBadge(appRoot);
    state.perf = new PerfOverlay(appRoot);
    state.sl1 = createSl1Hud(appRoot);
    state.controls = new ControlsBar(appRoot, (intent: ControlIntent) => {
      handleControl(intent, state);
    });
    state.sceneBrowser = new SceneBrowser(
      appRoot,
      SCENE_CATALOG,
      (intent: SceneSelectIntent) => handleSceneSelect(intent, state),
      state.selectedSceneId
    );

    // ?perf=1 turns on the perf overlay; 'P' key toggles.
    if (typeof window !== "undefined") {
      const params = new URLSearchParams(window.location.search);
      if (params.get("perf") === "1") state.perf.setEnabled(true);
      window.addEventListener("keydown", (ev) => {
        if (ev.key === "p" || ev.key === "P") {
          if (state.perf !== null) state.perf.toggle();
        }
      });
    }
  }

  window.addEventListener("resize", () => resize(canvas));

  // tab-refocus invariant: when tab regains focus, jump-cut to latest snapshot.
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) {
      if (state.rafHandle !== null) {
        cancelAnimationFrame(state.rafHandle);
        state.rafHandle = null;
      }
    } else {
      state.snapshots.markStale();
      state.lastSnapshotAt = nowMs();
      if (state.rafHandle === null) {
        state.rafHandle = requestAnimationFrame(() => frame(state, renderer));
      }
    }
  });

  const transport: Transport = createTransport();
  transport.connect((msg) => handleMessage(msg, state, renderer));

  // Tone.js / WebAudio cannot start without a user gesture; wire
  // the consent listener to the canvas + body so the first click or
  // key press initializes audio.
  state.audio.attachConsent(canvas);
  state.audio.attachConsent(document.body);

  state.rafHandle = requestAnimationFrame(() => frame(state, renderer));
}

if (typeof document !== "undefined") {
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    boot();
  }
}
