# ADR-002: Canvas2D over WebGL

**Status:** Accepted.

## Context

The visual target is Mini Metro / Shapez — flat shapes, palette
colors, crisp lines, occasional pulses and trails. We render up to
~1,000 movers, ~200 paths, ~200 nodes per frame at 60fps.

WebGL is the more performant choice in the limit. Canvas2D is
dramatically easier to author, debug, and hot-reload.

## Decision

Use Canvas2D with two specific structural choices that close most
of the perf gap:

1. **Pre-warmed `Path2D` buckets per palette color.** Rendering a
   batch of same-colored lines becomes one `ctx.stroke(path)` call.
2. **No per-frame allocations.** Snapshot interpolation, mover
   scratch arrays, event queue ring, and animation slots are all
   pre-sized at boot.

Animations live in `frontend/src/renderer/animations.ts` as pure
functions that take `(ctx, theme, payload, t)` — the HMR target.

## Consequences

- (+) Hot reload of animation code is < 300 ms; we can A/B easing
  curves and timings live (a core delight goal).
- (+) `textContent`-only DOM means we never have XSS surface, even
  from JSON-derived content.
- (+) A new visual effect = a new pure function; trivial to test.
- (-) Stress scenes > 1,000 movers will eventually push us to
  WebGL. When that happens, the renderer module is the only crate
  that needs to change — the protocol and animations are decoupled
  from the painting strategy.
