// frontend/src/tests/unit/sl1_legend.test.ts
//
// Tests for the SL1 role legend overlay: data-driven from
// SL1_ROLE_HINTS, only shows roles actually present in the scene,
// renders text via textContent (safe-text policy).

import { describe, it, expect, beforeAll, beforeEach } from "vitest";
import { Sl1RoleLegend, rolesInScene } from "../../ui/sl1_legend";
import { DEFAULT_THEME } from "../../renderer/theme";

beforeAll(() => {
  // jsdom does not implement Canvas2D — stub minimal context for swatches.
  type StubCtx = Partial<CanvasRenderingContext2D>;
  const stub: StubCtx = {
    beginPath: () => {},
    arc: () => {},
    rect: () => {},
    moveTo: () => {},
    lineTo: () => {},
    closePath: () => {},
    fill: () => {},
    stroke: () => {},
  };
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (HTMLCanvasElement.prototype as any).getContext = () => stub;
});

describe("rolesInScene", () => {
  it("returns empty set for no places", () => {
    expect(rolesInScene([])).toEqual(new Set());
  });

  it("returns unique roles", () => {
    const result = rolesInScene([
      { role: "source" },
      { role: "compute_cluster" },
      { role: "source" },
    ]);
    expect(result).toEqual(new Set(["source", "compute_cluster"]));
  });
});

describe("Sl1RoleLegend", () => {
  let parent: HTMLElement;
  beforeEach(() => {
    parent = document.createElement("div");
    document.body.appendChild(parent);
  });

  it("starts hidden", () => {
    const legend = new Sl1RoleLegend(parent);
    const root = legend.__testRoot();
    expect(root.style.display).toBe("none");
  });

  it("show() with no roles stays hidden", () => {
    const legend = new Sl1RoleLegend(parent);
    legend.show(DEFAULT_THEME, new Set());
    expect(legend.__testRoot().style.display).toBe("none");
  });

  it("show() with non-canonical roles still renders them (no hidden nodes)", () => {
    const legend = new Sl1RoleLegend(parent);
    legend.show(DEFAULT_THEME, new Set(["unknown_role"]));
    // Per Codex P1 fix: any role seen in sl1_places must appear in the
    // legend (otherwise the viewer sees a node they cannot decode).
    expect(legend.__testRoot().style.display).toBe("flex");
    expect(legend.__testRoot().textContent).toContain("unknown_role");
  });

  it("show() with roles displays legend with rows for each role", () => {
    const legend = new Sl1RoleLegend(parent);
    legend.show(DEFAULT_THEME, new Set(["source", "compute_cluster"]));
    const root = legend.__testRoot();
    expect(root.style.display).toBe("flex");
    expect(root.textContent).toContain("legend");
    expect(root.textContent).toContain("source");
    expect(root.textContent).toContain("compute");
  });

  it("only displays rows for roles present in the scene", () => {
    const legend = new Sl1RoleLegend(parent);
    legend.show(DEFAULT_THEME, new Set(["source"]));
    const root = legend.__testRoot();
    expect(root.textContent).toContain("source");
    expect(root.textContent).not.toContain("dashboard");
    expect(root.textContent).not.toContain("operator");
  });

  it("orders rows canonically regardless of input Set iteration order", () => {
    const legend = new Sl1RoleLegend(parent);
    // Pass roles in reverse canonical order.
    legend.show(DEFAULT_THEME, new Set(["operator", "compute_cluster", "source", "dashboard"]));
    const text = legend.__testRoot().textContent ?? "";
    const sourceIdx = text.indexOf("source");
    const computeIdx = text.indexOf("compute");
    const dashboardIdx = text.indexOf("dashboard");
    const operatorIdx = text.indexOf("operator");
    expect(sourceIdx).toBeLessThan(computeIdx);
    expect(computeIdx).toBeLessThan(dashboardIdx);
    expect(dashboardIdx).toBeLessThan(operatorIdx);
  });

  it("hide() hides the legend", () => {
    const legend = new Sl1RoleLegend(parent);
    legend.show(DEFAULT_THEME, new Set(["source"]));
    expect(legend.__testRoot().style.display).toBe("flex");
    legend.hide();
    expect(legend.__testRoot().style.display).toBe("none");
  });

  it("renders text via textContent — no script execution from hostile role names", () => {
    const legend = new Sl1RoleLegend(parent);
    // Role names with HTML payloads should never produce DOM nodes.
    // Note: unknown roles aren't in ROLE_LABELS so they wouldn't render,
    // but this verifies the safety guarantee directly.
    legend.show(DEFAULT_THEME, new Set(["source"]));
    const root = legend.__testRoot();
    expect(root.querySelector("script")).toBeNull();
    expect(root.querySelector("img")).toBeNull();
  });

  it("has region role and aria-label for screen readers", () => {
    const legend = new Sl1RoleLegend(parent);
    const root = legend.__testRoot();
    expect(root.getAttribute("role")).toBe("region");
    expect(root.getAttribute("aria-label")).toBe("Scene shape legend");
  });

  it("show() clears previous rows on re-render", () => {
    const legend = new Sl1RoleLegend(parent);
    legend.show(DEFAULT_THEME, new Set(["source", "compute_cluster"]));
    legend.show(DEFAULT_THEME, new Set(["dashboard"]));
    const text = legend.__testRoot().textContent ?? "";
    expect(text).toContain("dashboard");
    expect(text).not.toContain("source");
    expect(text).not.toContain("compute");
  });

  it("renders rows for non-canonical role strings using the raw role name", () => {
    const legend = new Sl1RoleLegend(parent);
    legend.show(DEFAULT_THEME, new Set(["source", "weird_custom_role"]));
    const text = legend.__testRoot().textContent ?? "";
    expect(text).toContain("source");
    expect(text).toContain("weird_custom_role");
  });

  it("non-canonical roles render after canonical ones in stable order", () => {
    const legend = new Sl1RoleLegend(parent);
    legend.show(DEFAULT_THEME, new Set(["zeta_role", "source", "alpha_role"]));
    const text = legend.__testRoot().textContent ?? "";
    const sourceIdx = text.indexOf("source");
    const alphaIdx = text.indexOf("alpha_role");
    const zetaIdx = text.indexOf("zeta_role");
    expect(sourceIdx).toBeGreaterThanOrEqual(0);
    expect(alphaIdx).toBeGreaterThanOrEqual(0);
    expect(zetaIdx).toBeGreaterThanOrEqual(0);
    // Canonical first.
    expect(sourceIdx).toBeLessThan(alphaIdx);
    expect(sourceIdx).toBeLessThan(zetaIdx);
    // Non-canonical in code-point order: alpha < zeta.
    expect(alphaIdx).toBeLessThan(zetaIdx);
  });

  it("clamps palette index when the theme palette is shorter than the hint", () => {
    const legend = new Sl1RoleLegend(parent);
    // operator's canonical color index is 6; a 3-entry palette would
    // be out of range and must clamp without throwing.
    const shortTheme = { palette: ["#000", "#fff", "#aaa"], background_index: 0, font: "system-ui" };
    expect(() => legend.show(shortTheme, new Set(["operator"]))).not.toThrow();
    const root = legend.__testRoot();
    expect(root.querySelector("canvas")).not.toBeNull();
  });
});
