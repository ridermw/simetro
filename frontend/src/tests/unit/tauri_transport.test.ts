import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { TauriTransport } from "../../transport/tauri";
import type { SimMessage } from "../../protocol/messages";

const mocks = vi.hoisted(() => ({
  listen: vi.fn(),
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: mocks.listen,
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: mocks.invoke,
}));

describe("TauriTransport", () => {
  let consoleError: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    mocks.listen.mockReset();
    mocks.invoke.mockReset();
    consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    consoleError.mockRestore();
  });

  it("emits transport_lost and cleans up the listener when subscribe fails", async () => {
    const unlisten = vi.fn();
    mocks.listen.mockResolvedValue(unlisten);
    mocks.invoke.mockRejectedValue(new Error("not permitted"));

    const transport = new TauriTransport();
    const received: SimMessage[] = [];
    transport.connect((msg) => received.push(msg));

    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(mocks.listen).toHaveBeenCalledWith("sim", expect.any(Function));
    expect(mocks.invoke).toHaveBeenCalledWith("cmd_subscribe");
    expect(unlisten).toHaveBeenCalledTimes(1);
    expect(received).toEqual([{ kind: "fault", payload: { kind: "transport_lost" } }]);
  });
});
