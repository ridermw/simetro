// frontend/src/tests/unit/inspector.test.ts
import { describe, it, expect } from "vitest";
import { InspectorPanel } from "../../inspector/panel";
import { hitTestPiece, summarizeNode } from "../../inspector/hover";
import type {
  AgentReport,
  SnapshotPayload,
  StaticPayload,
} from "../../protocol/messages";

function report(over: Partial<AgentReport> = {}): AgentReport {
  return {
    agent_id: "speed_tuner_0",
    tick: 42,
    confidence: 0.8,
    rationale: "test rationale",
    chosen: { kind: "set_speed", mover: 12, speed: 1.6 },
    considered: [
      { action: { kind: "set_speed", mover: 12, speed: 1.6 }, confidence: 0.8 },
      { action: { kind: "no_op" }, confidence: 0.3 },
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
    const html = root.innerHTML;
    expect(html).not.toContain("<script");
    expect(root.textContent).toContain("AGENT speed_tuner_0");
    expect(root.textContent).toContain("tick 42");
    expect(root.textContent).toContain("confidence 0.80");
    expect(root.textContent).toContain("SetSpeed(mover=12, speed=1.60)");
    expect(root.textContent).toContain("test rationale");
  });

  it("treats untrusted rationale as plain text", () => {
    const parent = document.createElement("div");
    const panel = new InspectorPanel(parent);
    const attack = "<img src=x onerror=alert(1)>";
    panel.show(report({ rationale: attack }));
    const root = panel.__testRoot();
    expect(root.textContent).toContain(attack);
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
    const tl = root.textContent ?? "";
    const match = tl.match(/TIMELINE\s+([▌▎▏]+)/);
    expect(match).not.toBeNull();
    expect(match?.[1]?.length).toBe(16);
  });

  it("marks the chosen action in the considered list", () => {
    const parent = document.createElement("div");
    const panel = new InspectorPanel(parent);
    panel.show(report());
    const root = panel.__testRoot();
    // Chosen row should carry the ● dot.
    const text = root.textContent ?? "";
    const chosenLine = text
      .split("\n")
      .find((l) => l.includes("SetSpeed(mover=12, speed=1.60)") && l.includes("0.80"));
    expect(chosenLine).toBeDefined();
    expect(chosenLine).toContain("●");
  });
});

describe("hover hit-testing", () => {
  const scene: StaticPayload = {
    name: "test",
    palette: ["#000", "#fff", "#7aa2f7", "#bb9af7"],
    background_index: 0,
    nodes: [
      { id: 1, pos: [100, 100], shape: "circle", color: 2 },
      { id: 2, pos: [400, 400], shape: "square", color: 3 },
    ],
    paths: [],
    node_names: { 1: "alpha", 2: "beta" },
    path_names: {},
    mover_names: { 7: "m1" },
  };
  const snap: SnapshotPayload = {
    tick: 0,
    movers: [{ id: 7, pos: [250, 250], on_path: 0, speed: 1 }],
  };

  it("hits a node within its radius", () => {
    const hit = hitTestPiece(scene, snap, 102, 98);
    expect(hit?.kind).toBe("node");
    expect(hit?.id).toBe(1);
    expect(hit?.label).toBe("alpha");
  });

  it("hits a mover and prefers movers over nodes when overlapping", () => {
    const overlap: SnapshotPayload = {
      ...snap,
      movers: [{ id: 7, pos: [100, 100], on_path: 0, speed: 1 }],
    };
    const hit = hitTestPiece(scene, overlap, 100, 100);
    expect(hit?.kind).toBe("mover");
    expect(hit?.label).toBe("m1");
  });

  it("returns null when nothing under cursor", () => {
    expect(hitTestPiece(scene, snap, 50, 50)).toBeNull();
  });

  it("falls back to numeric id when names lookup misses", () => {
    const sceneNoNames: StaticPayload = { ...scene, node_names: {} };
    const hit = hitTestPiece(sceneNoNames, snap, 100, 100);
    expect(hit?.label).toBe("node#1");
  });

  it("summarizeNode returns label + shape + color", () => {
    const node = scene.nodes[0]!;
    expect(summarizeNode(node, scene)).toBe("alpha  shape=circle  color=2");
  });
});
