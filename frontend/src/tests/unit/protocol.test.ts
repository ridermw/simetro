// frontend/src/tests/unit/protocol.test.ts
import { describe, it, expect } from "vitest";
import {
  SCHEMA_VERSION,
  isCurrentSchema,
  formatAction,
  formatActionTag,
  type Envelope,
} from "../../protocol/messages";

describe("protocol envelope", () => {
  it("accepts current schema version", () => {
    const env: Envelope<unknown> = {
      schema_version: SCHEMA_VERSION,
      seq: 0,
      payload: {},
    };
    expect(isCurrentSchema(env)).toBe(true);
  });

  it("rejects mismatched schema version", () => {
    const env: Envelope<unknown> = {
      schema_version: SCHEMA_VERSION + 1,
      seq: 0,
      payload: {},
    };
    expect(isCurrentSchema(env)).toBe(false);
  });
});

describe("Action formatting", () => {
  it("formats every Action kind without throwing", () => {
    expect(formatAction(null)).toBe("(none)");
    expect(formatAction({ kind: "no_op" })).toBe("NoOp");
    expect(formatAction({ kind: "set_speed", mover: 7, speed: 1.5 })).toBe(
      "SetSpeed(mover=7, speed=1.50)"
    );
    expect(
      formatAction({ kind: "place_piece", piece_kind: "node", pos: [10, 20] })
    ).toBe("PlacePiece(node, [10, 20])");
    expect(formatAction({ kind: "connect_pieces", from: 1, to: 2 })).toBe(
      "ConnectPieces(1 → 2)"
    );
    expect(formatAction({ kind: "remove_piece", id: 9 })).toBe("RemovePiece(9)");
  });

  it("formatActionTag returns display labels for every snake_case tag", () => {
    expect(formatActionTag("no_op")).toBe("NoOp");
    expect(formatActionTag("set_speed")).toBe("SetSpeed");
    expect(formatActionTag("place_piece")).toBe("PlacePiece");
    expect(formatActionTag("connect_pieces")).toBe("ConnectPieces");
    expect(formatActionTag("remove_piece")).toBe("RemovePiece");
  });
});
