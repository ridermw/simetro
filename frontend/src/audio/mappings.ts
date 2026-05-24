// frontend/src/audio/mappings.ts
//
// PLAN §21 delight #7 — tone per shape on arrival. Each node shape
// is associated with a distinct musical interval; nodes with the
// same shape sing in unison, different shapes harmonize.
//
// Pitches are chosen from a C major pentatonic — five notes that
// always sound consonant in any combination — so the soundtrack
// never grates even at high event rates.

import type { NodeShapeTag } from "../protocol/messages";

const SHAPE_TONES: Record<NodeShapeTag, string> = {
  circle: "C5",
  square: "E5",
  triangle: "G5",
  diamond: "A5",
  hexagon: "D5",
};

const FALLBACK_TONE = "C5";

export function toneForShape(shape: NodeShapeTag): string {
  return SHAPE_TONES[shape] ?? FALLBACK_TONE;
}

/**
 * For a generic arrival event when the node lookup might miss
 * (race between snapshot and event), pick a neutral root note.
 */
export function fallbackArrivalTone(): string {
  return FALLBACK_TONE;
}
