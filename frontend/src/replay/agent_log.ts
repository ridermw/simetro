import type { Action, ActionTag, AgentReport, SimEvent, SimMessage } from "../protocol/messages";

export interface AgentLogEntry {
  tick: number;
  agent_id: string;
  observation_hash: number;
  raw_response: string | null;
  parsed_action: Action | null;
  considered_count: number;
  rationale: string;
}

export interface ReplayScrub {
  fromTick?: number;
  toTick?: number;
  agentId?: string;
}

export interface ReplayCorrelationIssue {
  kind: "duplicate_decision" | "out_of_order";
  index: number;
  tick: number;
  agent_id: string;
  message: string;
}

export function scrubAgentLog(
  entries: readonly AgentLogEntry[],
  scrub: ReplayScrub = {}
): AgentLogEntry[] {
  return entries.filter((entry) => {
    if (scrub.fromTick !== undefined && entry.tick < scrub.fromTick) return false;
    if (scrub.toTick !== undefined && entry.tick > scrub.toTick) return false;
    if (scrub.agentId !== undefined && entry.agent_id !== scrub.agentId) return false;
    return true;
  });
}

export function correlateAgentLog(entries: readonly AgentLogEntry[]): ReplayCorrelationIssue[] {
  const issues: ReplayCorrelationIssue[] = [];
  const seen = new Set<string>();
  let previousTick: number | null = null;

  entries.forEach((entry, index) => {
    if (previousTick !== null && entry.tick < previousTick) {
      issues.push({
        kind: "out_of_order",
        index,
        tick: entry.tick,
        agent_id: entry.agent_id,
        message: `tick ${entry.tick} appears after tick ${previousTick}`,
      });
    }
    previousTick = entry.tick;

    const key = `${entry.tick}\u0000${entry.agent_id}`;
    if (seen.has(key)) {
      issues.push({
        kind: "duplicate_decision",
        index,
        tick: entry.tick,
        agent_id: entry.agent_id,
        message: "duplicate decision for tick+agent_id",
      });
    } else {
      seen.add(key);
    }
  });

  return issues;
}

export function agentLogEntryToMessages(entry: AgentLogEntry): SimMessage[] {
  return [
    {
      kind: "events",
      payload: [agentLogEntryToEvent(entry)],
    },
    {
      kind: "agent_report",
      payload: agentLogEntryToReport(entry),
    },
  ];
}

export function agentLogEntryToEvent(entry: AgentLogEntry): SimEvent {
  return {
    kind: "agent_decided",
    agent_id: entry.agent_id,
    action: actionTagFor(entry.parsed_action),
  };
}

export function agentLogEntryToReport(entry: AgentLogEntry): AgentReport {
  return {
    tick: entry.tick,
    agent_id: entry.agent_id,
    considered:
      entry.parsed_action === null ? [] : [{ action: entry.parsed_action, confidence: 1 }],
    chosen: entry.parsed_action,
    rationale: entry.rationale,
    confidence: entry.parsed_action === null ? 0 : 1,
  };
}

export function actionTagFor(action: Action | null): ActionTag {
  if (action === null) return "no_op";
  switch (action.kind) {
    case "no_op":
      return "no_op";
    case "set_speed":
      return "set_speed";
    case "place_piece":
      return "place_piece";
    case "connect_pieces":
      return "connect_pieces";
    case "remove_piece":
      return "remove_piece";
  }
}
