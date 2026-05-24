// frontend/src/tests/unit/inspector.test.ts
import { describe, it, expect } from "vitest";
import { InspectorPanel } from "../../inspector/panel";
import { hitTestPiece, summarizeNode } from "../../inspector/hover";
import type { AgentReport, SnapshotPayload } from "../../protocol/messages";

function report(over: Partial<AgentReport> = {}): AgentReport {
  return {
    agent_id: 7,
    tick: 42,
    confidence: 0.8,
    rationale: "test rationale",
    considered: [
      { action: "SetSpeed(1.6)", confidence: 0.8, chosen: true },
      { action: "NoOp", confidence: 0.3, chosen: false },
    ],
    ...over,
  };
}

describe("InspectorPanel", () => {
  it("renders an AgentReport via textContent (no innerHTML)", () => {
    const parent = document.createElement("div");
    const panel = new InspectorPanel(parent);
    panel.show(report());
    const root = panel.__testRoot();
    // PLAN §5.1 / §12: panel should NEVER use innerHTML directly.
    // The whole subtree under the panel must use text content; the
    // following snapshot will reveal any HTML markup that slipped in.
    const html = root.innerHTML;
    expect(html).not.toContain("<script");
    expect(root.textContent).toContain("AGENT 7");
    expect(root.textContent).toContain("tick 42");
    expect(root.textContent).toContain("confidence 0.80");
    expect(root.textContent).toContain("SetSpeed(1.6)");
    expect(root.textContent).toContain("test rationale");
  });

  it("treats untrusted rationale as plain text", () => {
    const parent = document.createElement("div");
    const panel = new InspectorPanel(parent);
    const attack = "<img src=x onerror=alert(1)>";
    panel.show(report({ rationale: attack }));
    const root = panel.__testRoot();
    // The literal angle brackets must survive — proves textContent path.
    expect(root.textContent).toContain(attack);
    // And the panel must NOT have an actual <img> child.
    expect(root.querySelector("img")).toBeNull();
  });

  it("setVisible toggles display", () => {
    const parent = document.createElement("div");
    const panel = new InspectorPanel(parent);
    panel.setVisible(false);
    expect(panel.__testRoot().style.display).toBe("none");
    panel.setVisible(true);
    expect(panel.__testRoot().style.display).toBe("block");
  });

  it("timeline grows up to the cap then shifts oldest out", () => {
    const parent = document.createElement("div");
    const panel = new InspectorPanel(parent);
    for (let i = 0; i < 32; i++) panel.show(report({ tick: i }));
    const root = panel.__testRoot();
    // 16 glyphs in the cap, each a single char.
    const tl = root.textContent ?? "";
    const match = tl.match(/TIMELINE\s+([▌▎▏]+)/);
    expect(match).not.toBeNull();
    expect(match?.[1]?.length).toBe(16);
  });
});

describe("hover hit-testing", () => {
  const snap: SnapshotPayload = {
    tick: 0,
    nodes: [
      { id: 1, pos: [100, 100], shape: "circle", color: 2 },
      { id: 2, pos: [400, 400], shape: "square", color: 3 },
    ],
    paths: [],
    movers: [{ id: 7, pos: [250, 250], on_path: 0, speed: 1 }],
  };

  it("hits a node within its radius", () => {
    const hit = hitTestPiece(snap, { 1: "alpha", 2: "beta" }, 102, 98);
    expect(hit?.kind).toBe("node");
    expect(hit?.id).toBe(1);
    expect(hit?.label).toBe("alpha");
  });

  it("hits a mover and prefers movers over nodes when overlapping", () => {
    const overlap: SnapshotPayload = {
      ...snap,
      movers: [{ id: 7, pos: [100, 100], on_path: 0, speed: 1 }],
    };
    const hit = hitTestPiece(overlap, { 7: "m1" }, 100, 100);
    expect(hit?.kind).toBe("mover");
    expect(hit?.label).toBe("m1");
  });

  it("returns null when nothing under cursor", () => {
    expect(hitTestPiece(snap, {}, 50, 50)).toBeNull();
  });

  it("falls back to numeric id when id_map is missing the piece", () => {
    const hit = hitTestPiece(snap, {}, 100, 100);
    expect(hit?.label).toBe("node#1");
  });

  it("summarizeNode returns label + shape + color", () => {
    const node = snap.nodes[0]!;
    expect(summarizeNode(node, { 1: "alpha" })).toBe("alpha  shape=circle  color=2");
  });
});
