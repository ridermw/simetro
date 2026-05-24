// frontend/src/tests/unit/mock_transport.test.ts
import { describe, it, expect, vi } from "vitest";
import { MockTransport } from "../../transport/mock";
import type { SimMessage } from "../../protocol/messages";

describe("MockTransport", () => {
  it("emits Static then Snapshot to the handler", async () => {
    const t = new MockTransport();
    const received: SimMessage[] = [];
    t.connect((m) => received.push(m));

    await new Promise((resolve) => setTimeout(resolve, 10));

    expect(received.length).toBe(2);
    expect(received[0]?.type).toBe("Static");
    expect(received[1]?.type).toBe("Snapshot");
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
});
