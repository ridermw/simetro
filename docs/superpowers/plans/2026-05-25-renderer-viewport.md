# Renderer Viewport Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make large rendered scenarios fit the canvas by default and support drag-pan, wheel zoom, and double-click reset-to-fit.

**Architecture:** Add a small renderer-owned viewport transform that maps world coordinates to canvas coordinates. `Renderer.setScene()` computes a fit transform from legacy node/path bounds, `Renderer.draw()` applies it around all scene geometry and overlay animation drawing, and `Renderer.attachViewportControls()` wires pointer/wheel/double-click interactions to the canvas. The scene JSON stays unchanged.

**Tech Stack:** TypeScript, Canvas2D, Vitest/jsdom renderer tests.

---

## Task 1: Add renderer viewport state and fit calculation

**Files:**
- Modify: `frontend/src/renderer/canvas.ts`
- Test: `frontend/src/tests/unit/renderer.test.ts`

- [ ] **Step 1: Add failing renderer tests**

Add tests that create a large scene with node positions outside the canvas, call `setScene`, and assert the renderer's test-only viewport has a scale below 1 and a non-zero translation. Also test that an empty scene keeps the identity viewport.

- [ ] **Step 2: Implement viewport model**

In `Renderer`, add `viewport = { scale: 1, offsetX: 0, offsetY: 0 }`, compute bounds from `scene.nodes` and `scene.paths`, include node/mover padding, and set the viewport so the world bounds fit inside the CSS canvas size. For empty scenes, keep identity.

- [ ] **Step 3: Apply viewport during draw**

After filling the background and before drawing paths/nodes/movers/overlays, call `ctx.translate(offsetX, offsetY)` and `ctx.scale(scale, scale)`. Keep background fill in screen space. Overlay animations should draw in world space with the same transform.

- [ ] **Step 4: Run renderer tests**

Run:

```bash
cd frontend && npm test -- --run renderer
```

Expected: renderer tests pass.

---

## Task 2: Add mouse interaction controls

**Files:**
- Modify: `frontend/src/renderer/canvas.ts`
- Modify: `frontend/src/main.ts`
- Test: `frontend/src/tests/unit/renderer.test.ts`

- [ ] **Step 1: Add failing interaction tests**

Add tests for:

- `panBy(dx, dy)` changes viewport offsets.
- `zoomAt(screenX, screenY, factor)` changes scale while keeping the world point under the cursor stable.
- `resetViewport()` returns to the scene fit transform.

- [ ] **Step 2: Implement public viewport methods**

Add:

```ts
panBy(dx: number, dy: number): void
zoomAt(screenX: number, screenY: number, factor: number): void
resetViewport(): void
attachViewportControls(): void
```

Clamp zoom to a sane range, for example `0.15..=8`. In `attachViewportControls`, use pointer events for dragging, `wheel` for zoom with `preventDefault`, and `dblclick` for reset. Use canvas-local coordinates from `getBoundingClientRect()`.

- [ ] **Step 3: Wire controls in boot**

In `frontend/src/main.ts`, after constructing `Renderer`, call `renderer.attachViewportControls()`.

- [ ] **Step 4: Run renderer tests**

Run:

```bash
cd frontend && npm test -- --run renderer
```

Expected: renderer tests pass.

---

## Task 3: Verify frontend and update PR

**Files:**
- Create: `docs/superpowers/plans/2026-05-25-renderer-viewport.md`
- Modify: PR body if needed.

- [ ] **Step 1: Run focused checks**

Run:

```bash
cd frontend && npm test -- --run renderer scene_browser catalog
cd frontend && npm run typecheck
```

Expected: all commands pass.

- [ ] **Step 2: Commit and push**

Run:

```bash
git add frontend/src/renderer/canvas.ts frontend/src/main.ts frontend/src/tests/unit/renderer.test.ts docs/superpowers/plans/2026-05-25-renderer-viewport.md
git commit -m "feat: add renderer pan and zoom viewport" -m "Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"
git push
```

Expected: commit and push succeed.

- [ ] **Step 3: Update PR body**

Add a note to PR #44 that rendered scenarios now auto-fit, support drag-pan, wheel zoom, and double-click reset.

## Self-review

- Spec coverage: fixes off-screen rendered scenarios and adds manual navigation.
- Scope control: does not change scene JSON, agent behavior, SL1 data model, or visibility model.
- Red-flag scan: no placeholders or open-ended tasks remain.
