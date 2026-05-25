// frontend/src/audio/engine.ts
//
// Tone.js voice pool for semantic simulation events.
//
//   ┌──────────────────────────────────────────────────────────────┐
//   │   SimEvent (MoverArrived) ──▶ mappings.toneFor(shape)        │
//   │                                       │                      │
//   │                                       ▼                      │
//   │   AudioEngine.play(tone)  ── pre-allocated PolySynth voices  │
//   │                                                              │
//   │   Browser autoplay policy: AudioContext starts only after a  │
//   │   user gesture. We gate Tone.start() behind first click;     │
//   │   until then, play() is a no-op (no errors, no console spam).│
//   └──────────────────────────────────────────────────────────────┘
//
// tick-budget invariant: tick budget — voice triggers are constant-time and
// never allocate beyond initial setup, so audio never contributes to
// `Warning::TickOverBudget`.

import * as Tone from "tone";

const POOL_SIZE = 8;
const VOLUME_DB = -18;

export class AudioEngine {
  private started = false;
  private synth: Tone.PolySynth | null = null;
  private startCalled = false;

  /**
   * Wire one-time autoplay consent. Safe to call multiple times.
   * After the first user gesture, AudioContext resumes and play()
   * becomes effectful.
   */
  attachConsent(target: EventTarget): void {
    const handler = async () => {
      target.removeEventListener("click", handler);
      target.removeEventListener("keydown", handler);
      await this.start();
    };
    target.addEventListener("click", handler, { once: false });
    target.addEventListener("keydown", handler, { once: false });
  }

  async start(): Promise<void> {
    if (this.startCalled) return;
    this.startCalled = true;
    try {
      await Tone.start();
      const synth = new Tone.PolySynth(Tone.Synth, {
        oscillator: { type: "triangle" },
        envelope: { attack: 0.005, decay: 0.18, sustain: 0.0, release: 0.18 },
      });
      synth.maxPolyphony = POOL_SIZE;
      synth.volume.value = VOLUME_DB;
      synth.toDestination();
      this.synth = synth;
      this.started = true;
    } catch (err) {
      // Audio is a delight, not a requirement; silently degrade.
      console.warn("simetro: audio init failed", err);
      this.started = false;
    }
  }

  isReady(): boolean {
    return this.started && this.synth !== null;
  }

  /**
   * Trigger a short note at the given pitch (e.g. "C5"). No-op when
   * audio hasn't been consented yet or is unavailable.
   */
  play(note: string, durationSec = 0.18, velocity = 0.7): void {
    if (!this.started || this.synth === null) return;
    try {
      this.synth.triggerAttackRelease(note, durationSec, undefined, velocity);
    } catch (err) {
      console.warn("simetro: audio trigger failed", err);
    }
  }

  dispose(): void {
    if (this.synth !== null) {
      this.synth.dispose();
      this.synth = null;
    }
    this.started = false;
    this.startCalled = false;
  }
}
