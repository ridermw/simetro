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
import { GalleryView } from "./ui/gallery_view";
import { FaultOverlay, HeartbeatBadge, PerfOverlay, WarningStrip } from "./ui/overlays";
import { SceneSwitcher } from "./ui/scene_switcher";
import {
  applySl1HudStatic,
  createSl1Hud,
  type Sl1Hud,
} from "./ui/sl1_hud";
import { Sl1RoleLegend, rolesInScene } from "./ui/sl1_legend";
import { themeFromStatic } from "./renderer/theme";
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
  gallery: GalleryView | null;
  switcher: SceneSwitcher | null;
  selectedSceneId: string | null;
  fault: FaultOverlay | null;
  warnings: WarningStrip | null;
  heartbeat: HeartbeatBadge | null;
  perf: PerfOverlay | null;
  sl1: Sl1Hud | null;
  sl1Legend: Sl1RoleLegend | null;
  paused: boolean;
  speedFactor: number;
  lastSnapshotAt: number;
  /** Estimated ms between snapshots; refined as we receive more. */
  snapshotPeriodMs: number;
  /** Scratch buffer reused every frame for interpolated movers. */
  moverScratch: MoverState[];
  rafHandle: number | null;
  /** Current view: "gallery" (landing) or "sim" (running scene). */
  currentView: "gallery" | "sim";
  /** Active transport — null in gallery view, created on scene entry. */
  transport: Transport | null;
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
    gallery: null,
    switcher: null,
    selectedSceneId: null,
    fault: null,
    warnings: null,
    heartbeat: null,
    perf: null,
    sl1: null,
    sl1Legend: null,
    paused: false,
    speedFactor: 1,
    lastSnapshotAt: 0,
    snapshotPeriodMs: 1000 / TARGET_SNAPSHOT_HZ,
    moverScratch: [],
    rafHandle: null,
    currentView: "gallery",
    transport: null,
  };
}

class ViewRouter {
  private canvas: HTMLCanvasElement;
  private state: AppState;
  private renderer: Renderer;
  private transitioning = false;
  /** Monotonic counter — each transition increments. Async callbacks
   *  capture the token at start and bail if a newer transition began. */
  private transitionToken = 0;

  constructor(
    canvas: HTMLCanvasElement,
    _appRoot: HTMLElement,
    state: AppState,
    renderer: Renderer
  ) {
    this.canvas = canvas;
    this.state = state;
    this.renderer = renderer;
  }

  /** Switch to gallery view — disconnect transport, show gallery. */
  showGallery(): void {
    this.transitionToken += 1;
    if (this.state.currentView === "gallery") return;
    this.state.currentView = "gallery";

    if (this.state.rafHandle !== null) {
      cancelAnimationFrame(this.state.rafHandle);
      this.state.rafHandle = null;
    }

    if (this.state.transport !== null) {
      this.state.transport.disconnect();
      this.state.transport = null;
    }

    this.canvas.style.display = "none";
    this.state.controls?.hide();
    this.state.switcher?.hide();
    this.state.inspector?.hide();
    this.state.fault?.hide();
    this.state.sl1Legend?.hide();
    this.state.gallery?.show();

    if (typeof window !== "undefined") {
      const url = new URL(window.location.href);
      url.searchParams.delete("scene");
      window.history.replaceState(null, "", url.toString());
    }
  }

  /** Switch to sim view — create transport for scene, show canvas. */
  showSim(sceneId: string): void {
    if (this.transitioning) return;
    this.transitioning = true;
    this.transitionToken += 1;
    const myToken = this.transitionToken;

    try {
      const previousSceneId = this.state.selectedSceneId;
      this.state.currentView = "sim";
      this.state.selectedSceneId = sceneId;

      this.state.gallery?.hide();
      this.state.gallery?.releaseMemory();

      this.canvas.style.display = "";
      this.state.controls?.show();
      this.state.inspector?.show();
      this.state.switcher?.setSelected(sceneId);
      this.state.switcher?.show();

      resetLocalSceneState(this.state);
      this.state.scene = null;
      this.state.lastSnapshotAt = 0;
      this.state.paused = false;
      this.state.controls?.setPaused(false);

      if (this.state.transport !== null) {
        this.state.transport.disconnect();
        this.state.transport = null;
      }

      const transport = createTransport(sceneId);
      this.state.transport = transport;
      transport.connect((msg) => handleMessage(msg, this.state, this.renderer));

      if (isTauri() && previousSceneId !== sceneId) {
        void routeSceneToTauri(sceneId).then((result) => {
          // Guard against stale callback — if a newer transition has
          // started, do not mutate state on behalf of this stale one.
          if (myToken !== this.transitionToken) return;
          if (!result.ok) {
            this.state.selectedSceneId = previousSceneId;
            this.state.switcher?.setSelected(previousSceneId ?? "");
            this.state.fault?.show({
              kind: "load_error",
              message: `Failed to switch scene: ${result.error}`,
              line: null,
              col: null,
            });
            if (previousSceneId === null) this.showGallery();
          }
        });
      }

      if (this.state.rafHandle === null) {
        this.state.rafHandle = requestAnimationFrame(() => frame(this.state, this.renderer));
      }

      if (typeof window !== "undefined") {
        const url = new URL(window.location.href);
        url.searchParams.set("scene", sceneId);
        window.history.replaceState(null, "", url.toString());
      }
    } catch (error) {
      console.error("simetro: failed to enter sim view", error);
      this.state.fault?.show({
        kind: "load_error",
        message: `Failed to load scene: ${errorMessage(error)}`,
        line: null,
        col: null,
      });
      this.showGallery();
    } finally {
      setTimeout(() => {
        this.transitioning = false;
      }, 300);
    }
  }
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


async function routeSceneToTauri(
  scene_id: string
): Promise<{ ok: true } | { ok: false; error: string }> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invokeSetScene(invoke, scene_id);
    return { ok: true };
  } catch (error) {
    return { ok: false, error: errorMessage(error) };
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
          msg.payload.sl1_observability_alerts,
          msg.payload.sl1_objectives
        );
        // Static block carries no live state — clear status until the
        // first snapshot lands.
        state.sl1.status.update(undefined, undefined);
      }
      if (state.sl1Legend !== null) {
        const places = msg.payload.sl1_places ?? [];
        if (places.length > 0) {
          state.sl1Legend.show(themeFromStatic(msg.payload), rolesInScene(places));
        } else {
          state.sl1Legend.hide();
        }
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
        if (msg.payload.sl1_objective_states !== undefined) {
          state.sl1.objectives.updateStates(msg.payload.sl1_objective_states);
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

function createTransport(sceneId: string | null): Transport {
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
  return new MockTransport({
    sl1Mode: sl1ModeFromLocation(search),
    ...(sceneId !== null ? { sceneId } : {}),
  });
}

function sceneFromLocation(): string | null {
  if (typeof window === "undefined") return null;
  const params = new URLSearchParams(window.location.search);
  return params.get("scene");
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
  renderer.attachViewportControls();
  // Expose renderer for e2e viewport inspection (non-Tauri builds only).
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).__simetroRenderer = renderer;
  const state = createAppState();

  const appRoot = document.getElementById("app");
  if (appRoot === null) {
    console.error("app root missing");
    return;
  }

  state.inspector = new InspectorPanel(appRoot);
  state.hover = new HoverTooltip(appRoot);
  state.hover.attach(canvas, (x, y) => renderer.screenToWorld(x, y));
  state.fault = new FaultOverlay(appRoot);
  state.warnings = new WarningStrip(appRoot);
  state.heartbeat = new HeartbeatBadge(appRoot);
  state.perf = new PerfOverlay(appRoot);
  state.sl1 = createSl1Hud(appRoot);
  state.sl1Legend = new Sl1RoleLegend(appRoot);
  state.controls = new ControlsBar(appRoot, (intent: ControlIntent) => {
    handleControl(intent, state);
  });

  let router: ViewRouter;
  const readyCatalog = SCENE_CATALOG.filter((scene) => scene.status === "ready");
  state.gallery = new GalleryView(appRoot, readyCatalog, (intent) => {
    router.showSim(intent.scene_id);
  });
  state.switcher = new SceneSwitcher(appRoot, readyCatalog, {
    onPrev: () => {
      const id = state.switcher?.getAdjacentId("prev");
      if (id !== null && id !== undefined) router.showSim(id);
    },
    onNext: () => {
      const id = state.switcher?.getAdjacentId("next");
      if (id !== null && id !== undefined) router.showSim(id);
    },
    onGallery: () => router.showGallery(),
  });
  router = new ViewRouter(canvas, appRoot, state, renderer);

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

  window.addEventListener("resize", () => {
    resize(canvas);
    renderer.refitViewport();
  });

  // tab-refocus invariant: when tab regains focus, jump-cut to latest snapshot.
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) {
      if (state.rafHandle !== null) {
        cancelAnimationFrame(state.rafHandle);
        state.rafHandle = null;
      }
    } else if (state.currentView === "sim") {
      state.snapshots.markStale();
      state.lastSnapshotAt = nowMs();
      if (state.rafHandle === null) {
        state.rafHandle = requestAnimationFrame(() => frame(state, renderer));
      }
    }
  });

  // Tone.js / WebAudio cannot start without a user gesture; wire
  // the consent listener to the canvas + body so the first click or
  // key press initializes audio.
  state.audio.attachConsent(canvas);
  state.audio.attachConsent(document.body);

  const requestedScene = sceneFromLocation();
  const requestedSceneEntry =
    requestedScene !== null ? findSceneById(requestedScene) : undefined;
  if (requestedScene !== null && requestedSceneEntry === undefined) {
    console.error(
      `simetro: unknown scene "${requestedScene}" in URL param, ignoring`
    );
  }

  if (requestedSceneEntry !== undefined && requestedScene !== null) {
    router.showSim(requestedScene);
  } else {
    canvas.style.display = "none";
    state.controls?.hide();
    state.switcher?.hide();
    state.inspector?.hide();
    state.fault?.hide();
    state.sl1Legend?.hide();
    state.gallery?.show();
  }
}

if (typeof document !== "undefined") {
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    boot();
  }
}
