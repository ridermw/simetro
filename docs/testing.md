# Testing

simetro tests at four layers; each guarantees something specific.

## Layer 1 — Rust unit tests (`cargo test --workspace`)

| Crate              | Count | Highlights                                   |
| ------------------ | ----- | -------------------------------------------- |
| `engine`           | 85    | Systems pipeline, snapshot encoding, faults  |
| `engine` (integ)   | 4     | Tick budget, zero-alloc, agent-log wiring    |
| `protocol`         | 17    | JSON roundtrip every message variant         |
| `loader`           | (in engine) | Bounds, dangling refs, palette       |
| `agent-bridge`     | 18    | Backend trait, Mock, tool specs, refusals    |

Run: `cargo test --workspace`. Every commit hooks `cargo fmt
--all` + `cargo clippy --workspace --all-targets -- -D warnings`
(see `.githooks/pre-commit`).

## Layer 2 — Headless CLI tests

`crates/headless/tests/cli.rs` (5 end-to-end binary tests):
`run`, `bench`, `hash`, `replay`, `export-session` — driven against
the actual built binary so the CLI surface is exercised as the user
sees it. Bad input always exits via `std::process::exit`, never a
panic, so this layer also serves as a panic-free contract test.

## Layer 3 — Determinism gate

`tests/baselines/demo-paths.hash` is the committed SHA-256 of
`hash_run(world, runner, ticks)` for the demo scene. The test
`crates/engine/tests/determinism_baseline.rs` re-runs and compares.
Any drift fails CI. To regenerate after an intentional change:

```bash
cargo run -p simetro-headless --release -- hash games/demo-paths.json > tests/baselines/demo-paths.hash
```

Add an ADR explaining the change.

## Layer 4 — Frontend tests

| Suite                                   | Count | Highlights                                    |
| --------------------------------------- | ----- | --------------------------------------------- |
| `frontend/src/tests/unit/*.test.ts`     | 52    | Protocol envelope, transport mock, renderer,  |
|                                         |       | snapshot interp, events queue, animation eng, |
|                                         |       | audio degradation, inspector + hover, ui      |
|                                         |       | shell + overlays.                             |
| `frontend/src/tests/e2e/smoke.spec.ts`  | 7     | Real Chromium: canvas paints, controls live,  |
|                                         |       | aria-label round-trip, perf overlay, pixel    |
|                                         |       | sample at node position is NOT background.    |

Run: `npm test` (unit) and `npm run test:e2e` (Playwright). E2E
boots `vite preview` on port 4173 against the production bundle.

## XSS regression tests

Two tests pin our `textContent`-only policy (PLAN §5.1 / §12):

- `inspector.test.ts`: feeds `<img src=x onerror=alert(1)>` as a
  rationale and asserts the literal characters survive AND no `<img>`
  element appears in the DOM.
- `ui.test.ts`: feeds `<script>evil</script>` as a fault message and
  asserts no `<script>` element appears.

The `no-unsanitized/method` + `no-unsanitized/property` ESLint
rules block `innerHTML`/`outerHTML`/`document.write`/eval-like
property writes at lint time.

## Zero-allocation invariants

`crates/engine/tests/zero_alloc.rs` runs an allocation-counting
allocator and asserts `tick()` allocates zero bytes after warm-up.
The frontend mirrors this in spirit by reusing `moverScratch`,
`eventScratch`, `pathBuckets`, the animation slot ring, and the
inspector timeline ring — see the explicit "reuses the supplied
output array" test in `snapshots.test.ts`.

## Local commands

```bash
# Rust
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# Frontend
cd frontend
npm run typecheck
npm run lint
npm test
npm run test:e2e
```
