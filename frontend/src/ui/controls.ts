// frontend/src/ui/controls.ts
//
// simulation controls UI — sim controls bar.
//
//   ┌───────────────────────────────────────────────────────────┐
//   │  ▶/⏸   ⏭ step   ↻ reload   speed: 0.5×  1×  2×  4×        │
//   └───────────────────────────────────────────────────────────┘
//
// Controls do NOT mutate sim state directly; they emit intents over a
// callback so the transport layer can decide whether to send them over
// the wire (Tauri) or to a local mock (browser dev).

export type ControlIntent =
  | { kind: "TogglePause" }
  | { kind: "Step" }
  | { kind: "Reload" }
  | { kind: "SetSpeed"; factor: number };

export type ControlHandler = (intent: ControlIntent) => void;

const SPEED_FACTORS = [0.5, 1, 2, 4];

export class ControlsBar {
  private root: HTMLElement;
  private playPauseBtn: HTMLButtonElement;
  private isPaused = false;
  private speedFactor = 1;
  private handler: ControlHandler;
  private speedButtons: Map<number, HTMLButtonElement> = new Map();

  constructor(parent: HTMLElement, handler: ControlHandler) {
    this.handler = handler;
    const built = buildDom(parent);
    this.root = built.root;
    this.playPauseBtn = built.playPause;
    this.speedButtons = built.speedButtons;
    this.wire(built);
    this.refresh();
  }

  setPaused(paused: boolean): void {
    this.isPaused = paused;
    this.refresh();
  }

  setSpeed(factor: number): void {
    this.speedFactor = factor;
    this.refresh();
  }

  __testRoot(): HTMLElement {
    return this.root;
  }

  private wire(built: BuiltDom): void {
    built.playPause.addEventListener("click", () => {
      this.isPaused = !this.isPaused;
      this.refresh();
      this.handler({ kind: "TogglePause" });
    });
    built.step.addEventListener("click", () => this.handler({ kind: "Step" }));
    built.reload.addEventListener("click", () => this.handler({ kind: "Reload" }));
    for (const [factor, btn] of built.speedButtons) {
      btn.addEventListener("click", () => {
        this.speedFactor = factor;
        this.refresh();
        this.handler({ kind: "SetSpeed", factor });
      });
    }
  }

  private refresh(): void {
    this.playPauseBtn.textContent = this.isPaused ? "▶" : "⏸";
    this.playPauseBtn.setAttribute("aria-label", this.isPaused ? "Play" : "Pause");
    for (const [factor, btn] of this.speedButtons) {
      btn.style.borderColor =
        factor === this.speedFactor ? "rgba(232, 234, 237, 0.6)" : "rgba(232, 234, 237, 0.15)";
      btn.setAttribute("aria-pressed", factor === this.speedFactor ? "true" : "false");
    }
  }
}

interface BuiltDom {
  root: HTMLElement;
  playPause: HTMLButtonElement;
  step: HTMLButtonElement;
  reload: HTMLButtonElement;
  speedButtons: Map<number, HTMLButtonElement>;
}

function buildDom(parent: HTMLElement): BuiltDom {
  const root = document.createElement("div");
  root.id = "simetro-controls";
  root.setAttribute("role", "toolbar");
  root.setAttribute("aria-label", "Simulation controls");
  root.style.cssText = [
    "position: absolute",
    "top: 12px",
    "left: 12px",
    "display: flex",
    "gap: 8px",
    "align-items: center",
    "padding: 8px 10px",
    "background: rgba(14, 17, 22, 0.85)",
    "border: 1px solid rgba(232, 234, 237, 0.15)",
    "border-radius: 8px",
    "font: 12px ui-monospace, SFMono-Regular, monospace",
    "color: #e8eaed",
    "z-index: 10",
  ].join(";");

  const playPause = button("⏸", "Pause", "simetro-play-pause");
  const step = button("⏭", "Step one tick", "simetro-step");
  const reload = button("↻", "Reload scene", "simetro-reload");

  const speedLabel = document.createElement("span");
  speedLabel.textContent = "speed";
  speedLabel.style.cssText = "opacity: 0.7; margin-left: 8px";

  const speedButtons = new Map<number, HTMLButtonElement>();
  for (const f of SPEED_FACTORS) {
    const b = button(`${f}×`, `Set speed ${f}×`, `simetro-speed-${f}`);
    speedButtons.set(f, b);
  }

  root.appendChild(playPause);
  root.appendChild(step);
  root.appendChild(reload);
  root.appendChild(speedLabel);
  for (const b of speedButtons.values()) root.appendChild(b);
  parent.appendChild(root);

  return { root, playPause, step, reload, speedButtons };
}

function button(label: string, aria: string, id: string): HTMLButtonElement {
  const b = document.createElement("button");
  b.id = id;
  b.type = "button";
  b.textContent = label;
  b.setAttribute("aria-label", aria);
  b.style.cssText = [
    "padding: 4px 8px",
    "background: transparent",
    "color: inherit",
    "border: 1px solid rgba(232, 234, 237, 0.15)",
    "border-radius: 4px",
    "font: inherit",
    "cursor: pointer",
  ].join(";");
  return b;
}
