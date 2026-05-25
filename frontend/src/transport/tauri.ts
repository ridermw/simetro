// frontend/src/transport/tauri.ts
//
// real transport — Real transport that receives SimMessages from the Rust
// engine driver via Tauri events. Used when the app is running inside
// the Tauri desktop shell (detected by __TAURI_INTERNALS__ on window).
//
//   ┌─────────────────────────────────────────────────────────────┐
//   │  Rust driver ──emit("sim", Envelope<SimMessage>)──▶ listen  │
//   │                                                      │      │
//   │                  handler(msg) ◀──────────────────────┘      │
//   └─────────────────────────────────────────────────────────────┘
//
// On connect, this transport:
//   1. Registers a Tauri event listener on the "sim" channel.
//   2. Invokes the "cmd_subscribe" command so the driver knows the
//      frontend is ready and begins emitting (handshake — no race).
//   3. Validates each inbound envelope's schema_version; on mismatch,
//      synthesizes a Fault::SchemaMismatch to the handler.

import type { Transport, MessageHandler } from "./mock";
import { SCHEMA_VERSION, type SimMessage } from "../protocol/messages";

interface SimEnvelope {
  schema_version: number;
  seq: number;
  payload: SimMessage;
}

export class TauriTransport implements Transport {
  readonly name = "tauri";
  private handler: MessageHandler | null = null;
  private unlisten: (() => void) | null = null;

  connect(handler: MessageHandler): void {
    this.handler = handler;
    void this.setupListener().catch((error: unknown) => {
      console.error("simetro: failed to initialize Tauri transport", error);
      this.emitTransportLost();
    });
  }

  disconnect(): void {
    if (this.unlisten !== null) {
      this.unlisten();
      this.unlisten = null;
    }
    this.handler = null;
  }

  private async setupListener(): Promise<void> {
    let registeredUnlisten: (() => void) | null = null;

    try {
      // Dynamic import so the Tauri API is not bundled in browser-only builds.
      const { listen } = await import("@tauri-apps/api/event");
      const { invoke } = await import("@tauri-apps/api/core");

      // Register listener first, then subscribe — guarantees no missed messages.
      registeredUnlisten = await listen<SimEnvelope>("sim", (event) => {
        if (this.handler === null) return;

        const env = event.payload;
        if (env.schema_version !== SCHEMA_VERSION) {
          this.handler({
            kind: "fault",
            payload: {
              kind: "schema_mismatch",
              expected: SCHEMA_VERSION,
              found: env.schema_version,
            },
          });
          return;
        }

        this.handler(env.payload);
      });

      // Store unlisten for disconnect(). If disconnect() was called during
      // async setup, immediately unlisten and bail.
      if (this.handler === null) {
        registeredUnlisten();
        return;
      }
      this.unlisten = registeredUnlisten;

      // Signal the Rust driver that the frontend is ready.
      await invoke("cmd_subscribe");
    } catch (error) {
      if (registeredUnlisten !== null && this.unlisten === registeredUnlisten) {
        registeredUnlisten();
        this.unlisten = null;
      }
      throw error;
    }
  }

  private emitTransportLost(): void {
    this.handler?.({ kind: "fault", payload: { kind: "transport_lost" } });
  }
}
