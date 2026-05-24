// frontend/src/renderer/theme.ts
//
// PLAN §4 — theme module. Single source of truth for palette
// resolution, typography, and eased curves. Renderer + Inspector +
// UI all import from here so a palette swap in the JSON scene file
// re-themes the entire app.
//
// PLAN §5.1 — palette indices are bounds-checked (background_index
// must be < palette.len()); the loader rejects bad scenes before
// they reach the frontend. We still defensively fall back to the
// default dark theme if a renderer ever sees an out-of-range index
// at runtime — no crashes, no flicker.

import type { ThemePayload } from "../protocol/messages";

// Mini-Metro-ish dark theme. Used as the fallback whenever a scene
// has not yet loaded or the palette is malformed.
export const DEFAULT_THEME: ThemePayload = {
  palette: [
    "#0e1116", // 0 - background
    "#e8eaed", // 1 - foreground / outlines / movers
    "#7aa2f7", // 2 - accent blue
    "#bb9af7", // 3 - accent purple
    "#9ece6a", // 4 - accent green
    "#f7768e", // 5 - accent red (errors)
    "#e0af68", // 6 - accent amber (warnings)
  ],
  background_index: 0,
  font: "system-ui",
};

export function paletteColor(theme: ThemePayload, index: number): string {
  return theme.palette[index] ?? DEFAULT_THEME.palette[1]!;
}

export function backgroundColor(theme: ThemePayload): string {
  return theme.palette[theme.background_index] ?? DEFAULT_THEME.palette[0]!;
}

export function foregroundColor(theme: ThemePayload): string {
  // Index 1 is conventionally the foreground in our palette layout.
  return theme.palette[1] ?? DEFAULT_THEME.palette[1]!;
}

// Easing curves. Step 18 swaps these into the animation table by
// reference; renderer keeps them here so theme.ts owns "visual feel"
// holistically (color + motion).
export const easings = {
  linear: (t: number) => t,
  easeOutQuad: (t: number) => 1 - (1 - t) * (1 - t),
  easeInOutQuad: (t: number) => (t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2),
  easeOutCubic: (t: number) => 1 - Math.pow(1 - t, 3),
  easeInOutCubic: (t: number) =>
    t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2,
};
