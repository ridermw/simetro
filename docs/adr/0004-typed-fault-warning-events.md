# ADR-004: Typed Fault and Warning events on the wire

**Status:** Accepted.

## Context

Personal-use software hides errors at its peril. A simulation
platform that silently drops an action, swallows a load failure,
or fails to render a stuck agent is worse than one that crashes —
you can't even tell something is wrong.

The mega plan review (Sections 1A, 2A, 11A) made this a legacy
requirement: every failure mode must surface visibly, with enough
context to act on.

## Decision

`SimMessage::Fault(EngineFault)` and `SimMessage::Warning(
EngineWarning)` are first-class wire variants. They carry typed
payloads, not free-text strings:

```rust
enum EngineFault {
    LoadError { field: String, message: String },
    AgentCrashed { agent_id: String, message: String },
    NumericDrift { tick: u64, mover: u64 },
    ChannelSaturated { lag_frames: u32 },
    SystemPanic { system: String, message: String },
    SchemaMismatch { found: u32, supported: u32 },
}

enum EngineWarning {
    InvalidAction { agent_id: String, reason: String },
    Behind { lag_frames: u32 },
    TickOverBudget { ms: u32 },
    AgentLogSlow,
}
```

The frontend has a `FaultOverlay` (full-bleed, blocking) and a
`WarningStrip` (pill list, auto-expiring, see stale-channel detection). Every
variant has a dedicated formatter in
`frontend/src/ui/overlays.ts`. The runbook
(`docs/runbook.md`) prescribes the operational response per
variant.

Every protocol envelope also carries `schema_version`; mismatches
raise `Fault::SchemaMismatch` and freeze the renderer — never
animate stale or unknown data.

## Consequences

- (+) No silent failures. Anything that's wrong reaches the user
  with structured context.
- (+) Tests can assert on variant + field, not on string contents.
- (+) The runbook is a real thing, not aspirational.
- (-) Every error path in the engine has to choose a variant. We
  accept this discipline; the typed enum makes the cost finite.
