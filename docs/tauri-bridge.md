# Tauri Bridge Architecture

How the Tauri desktop shell connects the Rust simulation engine to the
TypeScript frontend.

## Overview

```
┌─────────────────────────────────────────────────────────┐
│  Tauri Process                                          │
│                                                         │
│  ┌─────────────────────┐     ┌───────────────────────┐ │
│  │  driver.rs           │     │  main.rs              │ │
│  │  (tokio task)        │     │  (tauri commands)     │ │
│  │                      │     │                       │ │
│  │  TickRunner          │◀────│  cmd_toggle_pause/    │ │
│  │  World state         │ mpsc│  step/set_speed/      │ │
│  │  60Hz tick loop      │     │  subscribe            │ │
│  │  20Hz snapshot emit  │     │                       │ │
│  └──────────┬───────────┘     └───────────────────────┘ │
│             │                                           │
│             │ app.emit("sim", envelope)                  │
│             ▼                                           │
│  ┌──────────────────────────────────────────────────┐   │
│  │  Tauri event bus ("sim" channel)                  │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
              │
              │ IPC (JSON serialized SimEnvelope)
              ▼
┌─────────────────────────────────────────────────────────┐
│  WebView (frontend)                                     │
│                                                         │
│  ┌────────────────────┐     ┌───────────────────────┐  │
│  │  TauriTransport    │     │  main.ts              │  │
│  │  listen("sim")     │────▶│  handleMessage()      │  │
│  │                    │     │  render loop           │  │
│  └────────────────────┘     └───────────────────────┘  │
│                                                         │
│  Control intents (pause/resume/speed/reload)            │
│  → invoke("cmd_toggle_pause") etc.                     │
└─────────────────────────────────────────────────────────┘
```

## Message Flow

1. **Startup:** Tauri `setup()` calls `spawn_driver()` which loads
   `games/demo-paths.json`, creates the `TickRunner`, and starts the
   driver task. The driver waits for a `Subscribe` command before emitting.

2. **Subscribe handshake:** When TauriTransport connects, it invokes
   `cmd_subscribe`. The driver sends the cached `Static` payload
   followed by an initial `Snapshot`, then begins the tick loop.

3. **Tick loop:** The driver runs at 60 Hz at 1×, scaled by the
   current speed factor. Every 3rd simulation tick (~20 Hz at 1×) it
   encodes the world into a `Snapshot` message and emits it via
   `app.emit("sim", &envelope)`.

4. **Events:** Non-empty event batches (mover_arrived, departed,
   path_pulsed, etc.) are emitted immediately. Tick-only batches
   are filtered out.

5. **Control:** Frontend control buttons invoke Tauri commands
   (`cmd_toggle_pause`, `cmd_step`, `cmd_set_speed`, `cmd_reload`). These
   send `DriverCommand` variants over the mpsc channel.

6. **Reload:** `cmd_reload` re-reads the JSON file from disk,
   builds the replacement world off to the side, and emits fresh
   Static + Snapshot only after validation succeeds. If reading or
   loading fails, the driver emits the typed fault and keeps the
   previous scene running.

7. **Live file watch:** The desktop driver also watches the same
   scene path with a short debounce. If the file changes again before
   the window closes, the debounce deadline resets. Stable file changes
   enqueue the same reload command as the UI button, so successful
   reloads and load faults follow the exact same message path.

8. **Faults:** If `tick_once` panics inside `catch_unwind`, the
   driver transitions to `Faulted` state and emits a `Fault` message.
   It does not resume (corrupted state cannot be trusted).

## Future Scene Selection Policy

The next scene-switching implementation should preserve the current
manual-reload safety guarantees and add selection without interactive
blocking:

- The frontend or CLI sends a stable `scene_id`, not a filesystem path.
- Tauri resolves `scene_id` through a local registry of known scenes
  (static table or deterministic scan of `games/`), producing a
  repo-relative JSON path.
- Unknown ids, read errors, and `LoadError`s surface as typed faults;
  none of them clear or pause the old scene.
- Scene replacement is atomic: create the new `World`, `TickRunner`,
  metadata, static payload, and first snapshot before swapping state.
- Do not add new dependencies or binary assets for the picker/registry
  unless the task explicitly requires them.
- If labels, artwork, or UX copy are blocked, implement/test the
  registry and atomic swap first, then record the polish as follow-up
  work in `TODOS.md`.

## Browser-only Mode

When running without Tauri (`npm run dev`), the frontend detects
the absence of `__TAURI_INTERNALS__` and falls back to
`MockTransport`, which animates 3 movers along the demo-paths
triangle at 50ms intervals. Control intents are handled locally.

## Frontend scene-switch invariants

The fixed gallery (10 polished worlds plus `demo-paths`) must switch
only among the catalog; it must not accept arbitrary filesystem paths
from the WebView. The current manual reload path uses the same frontend
invariant helper:

- A successful `Static` payload is the commit point. It clears the
  snapshot buffer, animation slots, hover tooltip, inspector report,
  warnings, stale fault overlay, heartbeat timestamp, and mover scratch
  buffer before installing the new scene metadata.
- Renderer metadata (`warm` + `setScene`) is updated exactly once per
  committed `Static` payload.
- Browser mock reloads may reset locally because there is no backend
  load operation. Tauri reloads do not clear frontend state when the
  command is invoked; they wait for the driver to emit `Static`.
- A failed Tauri reload or future catalog switch emits a `Fault` and
  preserves the previously running scene/snapshot until a valid `Static`
  arrives.

## Files

| File | Role |
|------|------|
| `src-tauri/src/driver.rs` | Engine driver task |
| `src-tauri/src/main.rs` | Tauri command handlers + setup |
| `frontend/src/transport/tauri.ts` | TauriTransport |
| `frontend/src/transport/mock.ts` | Animated MockTransport (browser dev) |
| `frontend/src/main.ts` | Transport factory + control routing |
| `src-tauri/capabilities/default.json` | Tauri v2 permissions |
