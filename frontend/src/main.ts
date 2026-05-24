// frontend/src/main.ts
//
//   ┌────────────────────────────────────────────────────────────┐
//   │                       FRONTEND BOOT                        │
//   │                                                            │
//   │   transport ──▶ store (snapshots, events) ──▶ renderer     │
//   │       │                                          ▲         │
//   │       ▼                                          │         │
//   │     audio                                        ui        │
//   │       │                                          ▲         │
//   │       ▼                                          │         │
//   │   inspector ◀───────────── agent reports ────────┘         │
//   │                                                            │
//   └────────────────────────────────────────────────────────────┘
//
// PLAN §4 calls out this file as the entry that owns the wiring
// diagram. Step 16's contract is narrow: pull one `Static` + one
// `Snapshot` from the mock transport and prove the canvas paints
// something deterministic. Steps 17-21 fill in the systems above.

import { MockTransport, type Transport } from "./transport/mock";
import type { SimMessage } from "./protocol/messages";
import { renderStaticFrame } from "./renderer/canvas";
import type { SceneState } from "./renderer/canvas";

const sceneState: SceneState = {
  theme: null,
  snapshot: null,
};

function handleMessage(msg: SimMessage, canvas: HTMLCanvasElement): void {
  switch (msg.type) {
    case "Static":
      sceneState.theme = msg.payload.theme;
      break;
    case "Snapshot":
      sceneState.snapshot = msg.payload;
      break;
    case "Events":
    case "AgentReport":
    case "Fault":
    case "Warning":
      // Wired up in later steps; ignore in Step 16 scaffold.
      return;
  }
  if (sceneState.theme !== null && sceneState.snapshot !== null) {
    renderStaticFrame(canvas, sceneState);
  }
}

function boot(): void {
  const canvas = document.getElementById("scene");
  if (!(canvas instanceof HTMLCanvasElement)) {
    console.error("scene canvas missing");
    return;
  }
  resize(canvas);
  window.addEventListener("resize", () => {
    resize(canvas);
    if (sceneState.theme !== null && sceneState.snapshot !== null) {
      renderStaticFrame(canvas, sceneState);
    }
  });

  const transport: Transport = new MockTransport();
  transport.connect((msg) => handleMessage(msg, canvas));
}

function resize(canvas: HTMLCanvasElement): void {
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth;
  const h = canvas.clientHeight;
  canvas.width = Math.floor(w * dpr);
  canvas.height = Math.floor(h * dpr);
}

if (typeof document !== "undefined") {
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    boot();
  }
}
