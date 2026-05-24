import { describe, expect, it } from "vitest";
import {
  agentLogEntryToMessages,
  correlateAgentLog,
  scrubAgentLog,
  type AgentLogEntry,
} from "../../replay/agent_log";

function entry(over: Partial<AgentLogEntry> = {}): AgentLogEntry {
  return {
    tick: 10,
    agent_id: "speed_tuner_0",
    observation_hash: 123,
    raw_response: null,
    parsed_action: { kind: "set_speed", mover: 7, speed: 1.5 },
    considered_count: 1,
    rationale: "recorded decision",
    ...over,
  };
}

describe("AgentLog replay helpers", () => {
  it("scrubs entries by tick range and agent id", () => {
    const entries = [
      entry({ tick: 10, agent_id: "a" }),
      entry({ tick: 11, agent_id: "b" }),
      entry({ tick: 12, agent_id: "a" }),
    ];

    expect(scrubAgentLog(entries, { fromTick: 11, toTick: 12, agentId: "a" })).toEqual([
      entries[2],
    ]);
  });

  it("correlates duplicate and out-of-order decisions", () => {
    const issues = correlateAgentLog([
      entry({ tick: 10, agent_id: "a" }),
      entry({ tick: 10, agent_id: "a" }),
      entry({ tick: 9, agent_id: "b" }),
    ]);

    expect(issues.map((issue) => issue.kind)).toEqual(["duplicate_decision", "out_of_order"]);
    expect(issues[0]).toMatchObject({ index: 1, tick: 10, agent_id: "a" });
    expect(issues[1]).toMatchObject({ index: 2, tick: 9, agent_id: "b" });
  });

  it("converts one AgentLog entry into UI-consumable protocol messages", () => {
    const messages = agentLogEntryToMessages(entry());

    expect(messages).toHaveLength(2);
    expect(messages[0]).toEqual({
      kind: "events",
      payload: [
        {
          kind: "agent_decided",
          agent_id: "speed_tuner_0",
          action: "set_speed",
        },
      ],
    });
    expect(messages[1]).toMatchObject({
      kind: "agent_report",
      payload: {
        tick: 10,
        agent_id: "speed_tuner_0",
        confidence: 1,
        chosen: { kind: "set_speed", mover: 7, speed: 1.5 },
      },
    });
  });
});
