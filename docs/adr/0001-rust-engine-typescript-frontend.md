# ADR-001: Rust engine + TypeScript frontend

**Status:** Accepted.

## Context

simetro needs (1) a deterministic, allocation-free simulation core
that runs at ≥10kHz on a thread, and (2) a snappy, juicy 60fps UI
with Mini-Metro-class motion. These are two different problems with
two different language ecosystems.

A single-language stack (e.g. pure TypeScript) struggles to hit the
engine's determinism and throughput targets without WASM gymnastics.
A single-language stack the other direction (Rust + e.g. egui)
sacrifices the UI ergonomics that motivated the project in the
first place.

## Decision

Split at the protocol boundary:

- **Rust** crates own engine, protocol, loader, headless CLI, and
  agent-bridge.
- **TypeScript** owns the renderer, animations, audio, inspector, UI.
- They speak versioned JSON envelopes (see ADR-004 and
  `docs/protocol.md`).

## Consequences

- (+) Engine determinism is enforceable via cargo benchmarks +
  baseline hash without any frontend involvement.
- (+) The same engine binary serves the desktop app, headless CI,
  and (P2) any other surface that speaks the protocol.
- (+) Frontend can be iterated against a `MockTransport` with no
  Rust toolchain in the loop.
- (-) Two build systems, two test runners, two lint configs. We
  accept the duplication; it pays for itself by isolating concerns.
- (-) Any wire change requires touching both sides — the schema
  version check (ADR-004) makes the mismatch loud rather than
  silent.
