// frontend/src/tests/unit/mock_transport.test.ts
import { describe, it, expect, vi } from "vitest";
import { MockTransport, sl1ModeFromLocation } from "../../transport/mock";
import type { SimMessage } from "../../protocol/messages";

describe("MockTransport", () => {
  it("emits static then snapshot to the handler", async () => {
    const t = new MockTransport();
    const received: SimMessage[] = [];
    t.connect((m) => received.push(m));

    await new Promise((resolve) => setTimeout(resolve, 10));

    expect(received.length).toBe(2);
    expect(received[0]?.kind).toBe("static");
    expect(received[1]?.kind).toBe("snapshot");
    t.disconnect();
  });

  it("does not emit after disconnect", async () => {
    const t = new MockTransport();
    const cb = vi.fn();
    t.connect(cb);
    t.disconnect();

    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(cb).not.toHaveBeenCalled();
  });

  it("identifies itself as mock", () => {
    expect(new MockTransport().name).toBe("mock");
  });

  it("sl1Mode=false omits SL1 fields from static (legacy behavior)", async () => {
    const t = new MockTransport();
    const received: SimMessage[] = [];
    t.connect((m) => received.push(m));
    await new Promise((resolve) => setTimeout(resolve, 10));
    const stat = received[0];
    expect(stat?.kind).toBe("static");
    if (stat?.kind === "static") {
      expect(stat.payload.sl1_observability_dashboards).toBeUndefined();
      expect(stat.payload.sl1_observability_alerts).toBeUndefined();
      expect(stat.payload.sl1_milestones).toBeUndefined();
    }
    t.disconnect();
  });

  it("sl1Mode=true attaches SL1 metadata to static and SL1 state to snapshots", async () => {
    const t = new MockTransport({ sl1Mode: true });
    const received: SimMessage[] = [];
    t.connect((m) => received.push(m));
    // Let several ticks elapse so the scripted timeline emits at least
    // one milestone event and one outcome update.
    await new Promise((resolve) => setTimeout(resolve, 200));
    t.disconnect();

    const stat = received[0];
    expect(stat?.kind).toBe("static");
    if (stat?.kind === "static") {
      expect(stat.payload.sl1_observability_dashboards?.length).toBeGreaterThan(0);
      expect(stat.payload.sl1_observability_alerts?.length).toBeGreaterThan(0);
      expect(stat.payload.sl1_milestones?.length).toBeGreaterThan(0);
    }

    const milestoneEvent = received
      .filter((m): m is Extract<SimMessage, { kind: "events" }> => m.kind === "events")
      .flatMap((m) => m.payload)
      .find((ev) => ev.kind === "sl1_milestone_fired");
    expect(milestoneEvent).toBeDefined();
  });
});

describe("sl1ModeFromLocation", () => {
  it("returns false for undefined / empty / unrelated query", () => {
    expect(sl1ModeFromLocation(undefined)).toBe(false);
    expect(sl1ModeFromLocation("")).toBe(false);
    expect(sl1ModeFromLocation("?foo=bar")).toBe(false);
  });

  it("returns true only when sl1demo=1", () => {
    expect(sl1ModeFromLocation("?sl1demo=1")).toBe(true);
    expect(sl1ModeFromLocation("?sl1demo=0")).toBe(false);
    expect(sl1ModeFromLocation("?sl1demo=true")).toBe(false);
  });
});
