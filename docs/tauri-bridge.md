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
│  │  TickRunner          │◀────│  cmd_pause/resume/    │ │
│  │  World state         │ mpsc│  speed/reload/        │ │
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
│  → invoke("cmd_pause") etc.                            │
└─────────────────────────────────────────────────────────┘
```

## Message Flow

1. **Startup:** Tauri `setup()` calls `spawn_driver()` which loads
   `games/demo-paths.json`, creates the `TickRunner`, and starts the
   driver task. The driver waits for a `Subscribe` command before emitting.

2. **Subscribe handshake:** When TauriTransport connects, it invokes
   `cmd_subscribe`. The driver sends the cached `Static` payload
   followed by an initial `Snapshot`, then begins the tick loop.

3. **Tick loop:** The driver runs at 60 Hz. Every 3rd tick (~20 Hz)
   it encodes the world into a `Snapshot` message and emits it via
   `app.emit("sim", &envelope)`.

4. **Events:** Non-empty event batches (mover_arrived, departed,
   path_pulsed, etc.) are emitted immediately. Tick-only batches
   are filtered out.

5. **Control:** Frontend control buttons invoke Tauri commands
   (`cmd_pause`, `cmd_resume`, `cmd_speed`, `cmd_reload`). These
   send `DriverCommand` variants over the mpsc channel.

6. **Reload:** `cmd_reload` re-reads the JSON file from disk,
   rebuilds the world, and emits fresh Static + Snapshot.

7. **Faults:** If `tick_once` panics inside `catch_unwind`, the
   driver transitions to `Faulted` state and emits a `Fault` message.
   It does not resume (corrupted state cannot be trusted).

## Browser-only Mode

When running without Tauri (`npm run dev`), the frontend detects
the absence of `__TAURI_INTERNALS__` and falls back to
`MockTransport`, which animates 3 movers along the demo-paths
triangle at 50ms intervals. Control intents are handled locally.

## Files

| File | Role |
|------|------|
| `src-tauri/src/driver.rs` | Engine driver task |
| `src-tauri/src/main.rs` | Tauri command handlers + setup |
| `frontend/src/transport/tauri.ts` | TauriTransport |
| `frontend/src/transport/mock.ts` | Animated MockTransport (browser dev) |
| `frontend/src/main.ts` | Transport factory + control routing |
| `src-tauri/capabilities/default.json` | Tauri v2 permissions |
