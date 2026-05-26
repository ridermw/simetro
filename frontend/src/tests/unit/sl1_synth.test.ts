// frontend/src/tests/unit/sl1_synth.test.ts
//
// Tests for SL1 geometry synthesis: projecting sl1_places + sl1_links
// into legacy nodes + paths so the renderer can draw SL1 scenes.

import { describe, it, expect } from "vitest";
import { synthesizeSl1Geometry } from "../../renderer/sl1_synth";
import type { StaticPayload } from "../../protocol/messages";

function emptyPayload(over: Partial<StaticPayload> = {}): StaticPayload {
  return {
    name: "test",
    palette: ["#000", "#fff", "#7aa2f7", "#f7768e", "#9ece6a", "#e0af68", "#bb9af7"],
    background_index: 0,
    nodes: [],
    paths: [],
    node_names: {},
    path_names: {},
    mover_names: {},
    ...over,
  };
}

describe("synthesizeSl1Geometry", () => {
  it("returns input unchanged for non-SL1 scenes (no sl1_places)", () => {
    const payload = emptyPayload({
      nodes: [{ id: 1, pos: [10, 20], shape: "circle", color: 2 }],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(result).toBe(payload);
  });

  it("returns input unchanged for SL1 scenes that already have legacy nodes", () => {
    const payload = emptyPayload({
      nodes: [{ id: 1, pos: [10, 20], shape: "circle", color: 2 }],
      sl1_places: [{ id: "p1", role: "source", pos: [0, 0] }],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(result).toBe(payload);
  });

  it("returns input unchanged for SL1 scenes with empty places", () => {
    const payload = emptyPayload({ sl1_places: [] });
    const result = synthesizeSl1Geometry(payload);
    expect(result).toBe(payload);
  });

  it("synthesizes nodes from sl1_places using role-derived shape and color", () => {
    const payload = emptyPayload({
      sl1_places: [
        { id: "src", role: "source", pos: [-10, -10] },
        { id: "cpu", role: "compute_cluster", pos: [0, 0] },
        { id: "dash", role: "dashboard", pos: [10, -10] },
        { id: "ops", role: "operator", pos: [0, 10] },
      ],
    });
    const result = synthesizeSl1Geometry(payload);

    expect(result.nodes).toHaveLength(4);

    // Sorted alphabetically by id → cpu, dash, ops, src
    const byShape = Object.fromEntries(
      result.nodes.map((n) => [n.shape, n])
    );
    expect(byShape.hexagon).toBeDefined();
    expect(byShape.square).toBeDefined();
    expect(byShape.diamond).toBeDefined();
    expect(byShape.circle).toBeDefined();
  });

  it("synthesizes paths from sl1_links connecting place positions", () => {
    const payload = emptyPayload({
      sl1_places: [
        { id: "a", role: "source", pos: [0, 0] },
        { id: "b", role: "dashboard", pos: [100, 50] },
      ],
      sl1_links: [
        { id: "l1", from: "a", to: "b", direction: "forward" },
      ],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(result.paths).toHaveLength(1);
    const path = result.paths[0]!;
    expect(path.from_pos).toEqual([0, 0]);
    expect(path.to_pos).toEqual([100, 50]);
  });

  it("skips links referencing unknown place ids", () => {
    const payload = emptyPayload({
      sl1_places: [{ id: "a", role: "source", pos: [0, 0] }],
      sl1_links: [
        { id: "l1", from: "a", to: "nonexistent", direction: "forward" },
        { id: "l2", from: "missing", to: "a", direction: "forward" },
      ],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(result.paths).toHaveLength(0);
  });

  it("clamps out-of-range colors to palette length", () => {
    const payload = emptyPayload({
      palette: ["#000", "#fff", "#aaa"],
      sl1_places: [
        { id: "a", role: "source", pos: [0, 0], color: 999 },
      ],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(result.nodes[0]!.color).toBeLessThan(3);
    expect(result.nodes[0]!.color).toBeGreaterThanOrEqual(0);
  });

  it("honors explicit shape override on a place", () => {
    const payload = emptyPayload({
      sl1_places: [
        { id: "a", role: "source", pos: [0, 0], shape: "triangle" },
      ],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(result.nodes[0]!.shape).toBe("triangle");
  });

  it("ignores invalid shape strings and falls back to role hint", () => {
    const payload = emptyPayload({
      sl1_places: [
        { id: "a", role: "compute_cluster", pos: [0, 0], shape: "bogus-shape" },
      ],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(result.nodes[0]!.shape).toBe("hexagon");
  });

  it("uses fallback hint for unknown roles", () => {
    const payload = emptyPayload({
      sl1_places: [
        { id: "a", role: "weird_unrecognized_role", pos: [0, 0] },
      ],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(result.nodes[0]!.shape).toBe("circle");
  });

  it("synthesis is deterministic — same input produces same node ids", () => {
    const payload = emptyPayload({
      sl1_places: [
        { id: "z", role: "source", pos: [1, 1] },
        { id: "a", role: "dashboard", pos: [2, 2] },
        { id: "m", role: "operator", pos: [3, 3] },
      ],
    });
    const r1 = synthesizeSl1Geometry(payload);
    const r2 = synthesizeSl1Geometry(payload);
    expect(r1.nodes.map((n) => n.id)).toEqual(r2.nodes.map((n) => n.id));
    expect(r1.nodes.map((n) => n.pos)).toEqual(r2.nodes.map((n) => n.pos));
  });

  it("path color matches source place color so flow direction is visible", () => {
    const payload = emptyPayload({
      sl1_places: [
        { id: "a", role: "source", pos: [0, 0] },
        { id: "b", role: "dashboard", pos: [100, 0] },
      ],
      sl1_links: [
        { id: "l1", from: "a", to: "b", direction: "forward" },
      ],
    });
    const result = synthesizeSl1Geometry(payload);
    const sourceNode = result.nodes.find((n) => n.pos[0] === 0)!;
    expect(result.paths[0]!.color).toBe(sourceNode.color);
  });

  it("preserves all original metadata fields when synthesizing", () => {
    const payload = emptyPayload({
      sl1_places: [{ id: "a", role: "source", pos: [0, 0] }],
      sl1_observability_dashboards: [
        { id: "d1", title: "Test", description: "x" } as never,
      ],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(result.sl1_observability_dashboards).toEqual(
      payload.sl1_observability_dashboards
    );
    expect(result.palette).toEqual(payload.palette);
    expect(result.name).toEqual(payload.name);
  });

  it("populates node_names with SL1 place ids so hover shows real names", () => {
    const payload = emptyPayload({
      sl1_places: [
        { id: "mycroft-telemetry", role: "source", pos: [0, 0] },
        { id: "gpu-platform", role: "compute_cluster", pos: [100, 0] },
      ],
    });
    const result = synthesizeSl1Geometry(payload);
    const namesByValue = Object.values(result.node_names);
    expect(namesByValue).toContain("mycroft-telemetry");
    expect(namesByValue).toContain("gpu-platform");
  });

  it("populates path_names with SL1 link ids", () => {
    const payload = emptyPayload({
      sl1_places: [
        { id: "a", role: "source", pos: [0, 0] },
        { id: "b", role: "dashboard", pos: [100, 0] },
      ],
      sl1_links: [
        { id: "telemetry-feed", from: "a", to: "b", direction: "forward" },
      ],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(Object.values(result.path_names)).toContain("telemetry-feed");
  });

  it("preserves pre-existing entries in node_names / path_names", () => {
    const payload = emptyPayload({
      node_names: { 99: "preexisting" },
      path_names: { 88: "preexisting-path" },
      sl1_places: [{ id: "a", role: "source", pos: [0, 0] }],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(result.node_names[99]).toBe("preexisting");
    expect(result.path_names[88]).toBe("preexisting-path");
  });

  it("skips duplicate place ids — first occurrence wins", () => {
    const payload = emptyPayload({
      sl1_places: [
        { id: "dup", role: "source", pos: [0, 0] },
        { id: "dup", role: "dashboard", pos: [100, 100] },
      ],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(result.nodes).toHaveLength(1);
    // First by sorted id ('dup' === 'dup') — first appearance after
    // sort wins; both have same id so result is just one node.
    expect(result.nodes[0]!.pos).toEqual([0, 0]);
  });

  it("uses locale-independent comparator (id ordering not affected by locale)", () => {
    // Non-ASCII ids that would compare differently under some locale
    // collations. Plain code-point comparison is what we want.
    const payload = emptyPayload({
      sl1_places: [
        { id: "Z", role: "source", pos: [0, 0] },
        { id: "a", role: "dashboard", pos: [100, 0] },
      ],
    });
    const result = synthesizeSl1Geometry(payload);
    // ASCII code-point ordering: "Z" (0x5A) < "a" (0x61), so "Z" gets id 1.
    expect(result.node_names[1]).toBe("Z");
    expect(result.node_names[2]).toBe("a");
  });

  it("handles self-loop link (place linked to itself) by drawing degenerate path", () => {
    const payload = emptyPayload({
      sl1_places: [{ id: "a", role: "source", pos: [50, 50] }],
      sl1_links: [{ id: "self", from: "a", to: "a", direction: "forward" }],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(result.paths).toHaveLength(1);
    expect(result.paths[0]!.from_pos).toEqual([50, 50]);
    expect(result.paths[0]!.to_pos).toEqual([50, 50]);
  });

  it("works with empty palette (clamps colors to 0)", () => {
    const payload = emptyPayload({
      palette: [],
      sl1_places: [{ id: "a", role: "source", pos: [0, 0] }],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(result.nodes[0]!.color).toBe(0);
  });

  it("sets show_node_labels=true so the renderer draws place names", () => {
    const payload = emptyPayload({
      sl1_places: [{ id: "a", role: "source", pos: [0, 0] }],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(result.show_node_labels).toBe(true);
  });

  it("does not set show_node_labels on pass-through (non-SL1) scenes", () => {
    const payload = emptyPayload({
      nodes: [{ id: 1, pos: [10, 20], shape: "circle", color: 2 }],
    });
    const result = synthesizeSl1Geometry(payload);
    // Pass-through returns the original payload, so the flag is
    // unchanged (undefined here).
    expect(result.show_node_labels).toBeUndefined();
  });

  it("handles large coordinates without truncation or precision loss", () => {
    const payload = emptyPayload({
      sl1_places: [
        { id: "a", role: "source", pos: [1e6, -1e6] },
        { id: "b", role: "dashboard", pos: [2e6, 3e6] },
      ],
      sl1_links: [{ id: "l1", from: "a", to: "b", direction: "forward" }],
    });
    const result = synthesizeSl1Geometry(payload);
    expect(result.nodes[0]!.pos).toEqual([1e6, -1e6]);
    expect(result.paths[0]!.to_pos).toEqual([2e6, 3e6]);
  });
});
