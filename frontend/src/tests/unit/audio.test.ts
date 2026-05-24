// frontend/src/tests/unit/audio.test.ts
import { describe, it, expect, vi, beforeEach } from "vitest";
import { AudioEngine } from "../../audio/engine";
import { toneForShape, fallbackArrivalTone } from "../../audio/mappings";

// Tone.js needs WebAudio which jsdom does not provide; we verify the
// public no-audio degradation contract: play() before start() is a
// silent no-op, and start() does not throw even when Tone fails.

describe("AudioEngine no-audio degradation", () => {
  beforeEach(() => {
    // Swallow expected warnings during tests.
    vi.spyOn(console, "warn").mockImplementation(() => {});
  });

  it("play() before start() is a no-op (no throw)", () => {
    const a = new AudioEngine();
    expect(() => a.play("C5")).not.toThrow();
    expect(a.isReady()).toBe(false);
  });

  it("start() does not throw even if Tone init fails", async () => {
    const a = new AudioEngine();
    await expect(a.start()).resolves.toBeUndefined();
  });

  it("dispose() is safe before start()", () => {
    const a = new AudioEngine();
    expect(() => a.dispose()).not.toThrow();
  });

  it("attachConsent does not throw on a synthetic target", () => {
    const a = new AudioEngine();
    const target = new EventTarget();
    expect(() => a.attachConsent(target)).not.toThrow();
  });
});

describe("audio mappings", () => {
  it("returns a distinct tone for each shape", () => {
    const tones = new Set([
      toneForShape("circle"),
      toneForShape("square"),
      toneForShape("triangle"),
      toneForShape("diamond"),
    ]);
    expect(tones.size).toBe(4);
  });

  it("fallbackArrivalTone is in the pentatonic scale (root C)", () => {
    expect(fallbackArrivalTone()).toBe("C5");
  });
});
