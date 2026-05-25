# Testing

simetro tests at four layers; each guarantees something specific.

## Layer 1 — Rust unit tests (`cargo test --workspace`)

| Crate            | Count       | Highlights                                        |
| ---------------- | ----------- | ------------------------------------------------- |
| `engine`         | 85          | Systems pipeline, snapshot encoding, faults       |
| `engine` (integ) | 5           | Tick budget, zero-alloc, agent-log, world quality |
| `protocol`       | 17          | JSON roundtrip every message variant              |
| `loader`         | (in engine) | Bounds, dangling refs, palette                    |
| `agent-bridge`   | 18          | Backend trait, Mock, tool specs, refusals         |

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

| Suite                                  | Count | Highlights                                    |
| -------------------------------------- | ----- | --------------------------------------------- |
| `frontend/src/tests/unit/*.test.ts`    | 52    | Protocol envelope, transport mock, renderer,  |
|                                        |       | snapshot interp, events queue, animation eng, |
|                                        |       | audio degradation, inspector + hover, ui      |
|                                        |       | shell + overlays.                             |
| `frontend/src/tests/e2e/smoke.spec.ts` | 7     | Real Chromium: canvas paints, controls live,  |
|                                        |       | aria-label round-trip, perf overlay, pixel    |
|                                        |       | sample at node position is NOT background.    |

Run: `npm test` (unit) and `npm run test:e2e` (Playwright). E2E
boots `vite preview` on port 4173 against the production bundle.

## XSS regression tests

Two tests pin our `textContent`-only policy (safe text-rendering policy):

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

## Continuous PR delivery loop

Use this as the source of truth for high-cadence PR delivery. The target is one
independently reviewable PR per hour where feasible; if a change cannot be
validated and reviewed at that size, split it or stop at the smallest green
slice.

### Scope and compressed paired-world mode

- Keep PRs narrow: one behavior, one integration seam, or one mechanical data
  update. Avoid mixing broad shared files (`PLAN.md`, root config, protocol
  schemas, baselines, renderer/engine entry points) with unrelated leaf changes.
- For logic plus world churn, use compressed paired-world mode: open the focused
  logic PR first, then a companion mechanical world PR in the same delivery
  window when feasible. The companion PR is limited to scene JSON, baselines,
  generated assets, or demo-world updates; it lists the generator command or
  manual edit recipe and carries no unrelated code.
- Each paired PR must be independently reviewable. If the world PR depends on
  the logic PR, say so in both PR descriptions and keep the dependency one-way.
- Treat fast CI as required signal: run the narrow local command set that
  matches the changed area, then let GitHub Actions confirm the full matrix.

### Validation command sets

Run every command set that matches the files changed before requesting review.
If a command is skipped, record the reason in the PR description.

```bash
# Rust-only changes
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets

# Determinism-affecting changes
cargo run --release -p simetro-headless -- hash --scene games/demo-paths.json --ticks 10000 --seed 42

# Frontend changes
(cd frontend && npm run lint)
(cd frontend && npm run typecheck)
(cd frontend && npm test)
(cd frontend && npm run build)

# Polished v1 world/catalog changes
cargo test -p simetro-engine --test world_quality_checklist
(cd frontend && npm test -- --run catalog scene_browser scene_commands)

# UI/animation changes
(cd frontend && npm run test:e2e)

# Tauri shell changes
(cd frontend && npm run build)
(cd src-tauri && cargo build)
```

For docs-only PRs, at minimum review the rendered diff and run:

```bash
git diff --check
```

### PR description checklist

Every PR body should include:

- Scope and intent, including whether this is a standalone PR or a paired
  mechanical world PR.
- Changed shared files, if any.
- Validation commands run and their result.
- Skipped checks with a reason.
- Known risks and rollback posture.
- Links to companion PRs or dependency order.
- Non-blocking follow-up items, or "None".

### Review feedback loop

- Triage review and CI feedback in the next delivery cycle.
- Blocking feedback names a failing validation command, correctness/safety bug,
  determinism drift, data-loss risk, or user-visible breakage. Fix it in the
  current PR, rerun the relevant command set, and reply with the evidence.
- Non-blocking feedback becomes a `TODOS.md` item, issue, or PR-body follow-up
  instead of expanding the current PR.
- After substantial changes, update the PR body checklist so reviewers do not
  have to reconstruct scope or validation history from comments.

### Non-blocking follow-up policy

Only correctness, safety, determinism, CI, data loss, or user-visible breakage
blocks merge. Refactors, polish, broader coverage, alternate UX, larger
world/catalog additions, and nice-to-have review ideas are follow-ups unless
they are required to make the current PR true and validated.
