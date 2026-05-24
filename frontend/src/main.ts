// frontend/src/main.ts
//
//   ┌────────────────────────────────────────────────────────────┐
//   │                       FRONTEND BOOT                        │
//   │                                                            │
//   │   transport ──▶ store (snapshots, events) ──▶ renderer     │
//   │       │                                          ▲         │
//   │       ▼                                          │ rAF     │
//   │     audio                                        │         │
//   │       │                                          │         │
//   │       ▼                                          │         │
//   │   inspector ◀───────────── agent reports ────────┘         │
//   │                                                            │
//   └────────────────────────────────────────────────────────────┘
//
// PLAN §4. Step 17 adds:
//   * a real Renderer (Path2D batching, pre-allocated buckets)
//   * a SnapshotBuffer with two-snapshot mover interpolation
//   * a requestAnimationFrame loop (paused while tab is hidden,
//     PLAN §13 #5: jump-cut on refocus)
//
// Steps 18-21 plug animations, audio, inspector, and UI shell into
// the slots already present in this wiring.

import { MockTransport, type Transport } from "./transport/mock";
import type {
  MoverSnapshot,
  NodeSnapshot,
  SimEvent,
  SimMessage,
  SnapshotPayload,
  ThemePayload,
} from "./protocol/messages";
import { Renderer } from "./renderer/canvas";
import { DEFAULT_THEME } from "./renderer/theme";
import { AnimationEngine } from "./renderer/animation_engine";
import { SnapshotBuffer } from "./store/snapshots";
import { EventQueue } from "./store/events";
import { AudioEngine } from "./audio/engine";
import { fallbackArrivalTone, toneForShape } from "./audio/mappings";

interface AppState {
  theme: ThemePayload;
  snapshots: SnapshotBuffer;
  events: EventQueue;
  animations: AnimationEngine;
  audio: AudioEngine;
  lastSnapshotAt: number;
  /** Estimated ms between snapshots; refined as we receive more. */
  snapshotPeriodMs: number;
  /** Scratch buffer reused every frame for interpolated movers. */
  moverScratch: MoverSnapshot[];
  /** Scratch buffer reused every frame when draining events. */
  eventScratch: SimEvent[];
  rafHandle: number | null;
}

const TARGET_SNAPSHOT_HZ = 20; // PLAN §6 — snapshots at 20Hz

function createAppState(): AppState {
  return {
    theme: DEFAULT_THEME,
    snapshots: new SnapshotBuffer(),
    events: new EventQueue(),
    animations: new AnimationEngine(),
    audio: new AudioEngine(),
    lastSnapshotAt: 0,
    snapshotPeriodMs: 1000 / TARGET_SNAPSHOT_HZ,
    moverScratch: [],
    eventScratch: [],
    rafHandle: null,
  };
}

function findArrivalNode(snap: SnapshotPayload, nodeId: number): NodeSnapshot | undefined {
  for (const n of snap.nodes) if (n.id === nodeId) return n;
  return undefined;
}

function handleMessage(msg: SimMessage, state: AppState, renderer: Renderer): void {
  switch (msg.type) {
    case "Static":
      state.theme = msg.payload.theme;
      renderer.warm(state.theme);
      break;
    case "Snapshot": {
      const now = nowMs();
      if (state.lastSnapshotAt !== 0) {
        const dt = now - state.lastSnapshotAt;
        state.snapshotPeriodMs = Math.max(
          16,
          Math.min(500, state.snapshotPeriodMs * 0.8 + dt * 0.2)
        );
      }
      state.lastSnapshotAt = now;
      state.snapshots.push(msg.payload);
      break;
    }
    case "Events": {
      const now = nowMs();
      const snap = state.snapshots.current();
      for (const ev of msg.payload) {
        state.events.enqueue(ev);
        state.animations.spawn(ev, now);
        // Audio: ping on arrivals, pitched by the destination node's shape.
        if (ev.tag === "MoverArrived" && snap !== null) {
          const node = findArrivalNode(snap, ev.at_node);
          const tone = node !== undefined ? toneForShape(node.shape) : fallbackArrivalTone();
          state.audio.play(tone);
        }
      }
      break;
    }
    case "AgentReport":
    case "Fault":
    case "Warning":
      // Wired in Steps 20-21.
      return;
  }
}

function frame(state: AppState, renderer: Renderer): void {
  const cur = state.snapshots.current();
  if (cur !== null) {
    const now = nowMs();
    const elapsed = now - state.lastSnapshotAt;
    const alpha =
      state.snapshots.previous() === null
        ? 1
        : Math.max(0, Math.min(1, elapsed / state.snapshotPeriodMs));
    const movers = state.snapshots.interpolatedMovers(alpha, state.moverScratch);
    renderer.draw({
      theme: state.theme,
      snapshot: cur,
      movers,
      overlay: (ctx) => state.animations.draw(ctx, now, state.theme, cur),
    });
  }
  state.rafHandle = requestAnimationFrame(() => frame(state, renderer));
}

function nowMs(): number {
  return typeof performance !== "undefined" ? performance.now() : Date.now();
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

  window.addEventListener("resize", () => resize(canvas));

  // PLAN §13 #5: when tab regains focus, jump-cut to latest snapshot.
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

  const transport: Transport = new MockTransport();
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
