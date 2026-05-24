// frontend/src/tests/unit/protocol.test.ts
import { describe, it, expect } from "vitest";
import {
  SCHEMA_VERSION,
  isCurrentSchema,
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
