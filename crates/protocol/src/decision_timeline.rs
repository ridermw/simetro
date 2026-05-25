//! # DecisionTimeline (DecisionTimeline)
//!
//! Promotes per-agent decision history from an ad-hoc engine field to
//! a **first-class, addressable, version-pinned object** that replay,
//! the inspector, and the session-bundle exporter can all consume by
//! stable ID.
//!
//! The ID space used here is the
//! source of truth for `RequestId::timeline_id` in the engine's
//! `lifecycle` module: when the lifecycle re-issues a request,
//! `timeline_id` is **preserved** and `attempts` on the
//! `DecisionTimeline` entry is bumped instead of allocating a new id.
//!
//! # Layering invariant
//!
//! `DecisionTimeline` is a pure in-memory ledger. Persistence is
//! handled by `AgentLog v2`; entries here carry an optional
//! [`RawResponseRef`] pointing into the on-disk log. The timeline
//! itself is a bounded sliding window (default 4096 entries) — old
//! entries are evicted in FIFO order, but [`TimelineId`] values are
//! **never reused** even after eviction (the internal `next_id`
//! counter is monotonic for the entire engine run).
//!
//! # Schema version
//!
//! Independent of [`crate::SCHEMA_VERSION`] (wire envelope). Bumped
//! when on-disk or exported timeline JSON breaks compatibility.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::{Action, WarningPayload};

/// Current DecisionTimeline schema. Bump on incompatible shape
/// changes to [`DecisionEntry`] / [`DecisionTimeline`] /
/// [`DecisionTimelineSnapshot`].
pub const DECISION_TIMELINE_SCHEMA_VERSION: u32 = 1;

/// Default sliding-window capacity. ~4096 entries ≈ several hours of
/// decisions at `interval_ticks=600` and `dt=1/60`.
pub const DEFAULT_TIMELINE_CAPACITY: usize = 4096;

/// Monotonic-per-engine-run identifier. NEVER reused — even after the
/// owning entry has been evicted from the sliding window.
#[derive(
    Debug, Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct TimelineId(pub u64);

impl std::fmt::Display for TimelineId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Status of a decision in its lifecycle. Tick fields are bundled
/// into the variant that produced them so a `Pending` entry can never
/// accidentally carry an `applied_tick`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DecisionStatus {
    /// Emitted to the bridge; awaiting reply.
    Pending,
    /// Reply drained on-time; chosen action applied to the world.
    Applied { applied_tick: u64 },
    /// Past deadline; re-issued (new request with `attempt += 1`,
    /// SAME `TimelineId`). Once a re-issue actually goes out, the
    /// status flips back to [`DecisionStatus::Pending`] via
    /// [`DecisionTimeline::reissue`].
    Expired { expired_tick: u64 },
    /// Re-issue cap exhausted; engine gave up on this decision.
    GaveUp { last_tick: u64 },
    /// Bridge fault or unknown-id; not retriable.
    Faulted { fault_tick: u64, reason: String },
}

impl DecisionStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            DecisionStatus::Applied { .. }
                | DecisionStatus::GaveUp { .. }
                | DecisionStatus::Faulted { .. }
        )
    }
}

/// Stable pointer from a DecisionTimeline entry into the on-disk
/// AgentLog v2 row that holds the raw model response. Self-contained
/// (no live file handle) so it survives serialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawResponseRef {
    /// Scene slug used to compose the AgentLog path on disk.
    pub scene_id: String,
    /// Byte offset of the row inside the per-scene JSONL log.
    pub byte_offset: u64,
    /// Byte length of the row (NOT including the trailing newline).
    pub byte_len: u64,
}

/// Per-decision response payload. Present only when a reply was
/// actually drained (status is `Applied` or, in pathological cases,
/// `Faulted` after a malformed parse). `None` for `Pending`,
/// `Expired`, or `GaveUp` entries.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DecisionResponse {
    pub chosen: Option<Action>,
    pub rationale: String,
    pub confidence: f32,
    pub latency_ms: Option<u32>,
    pub raw_response_ref: Option<RawResponseRef>,
}

/// One row in the DecisionTimeline.
///
/// **Stable JSON shape**: this is the unit replay/editor/bundle
/// consumers see. Adding fields is forward-compatible because consumers
/// must ignore unknown fields. Removing or renaming fields requires a
/// `DECISION_TIMELINE_SCHEMA_VERSION` bump.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionEntry {
    pub schema_version: u32,
    pub id: TimelineId,
    /// Engine tick at which this decision was FIRST emitted. Re-issues
    /// preserve this value (see [`DecisionTimeline::reissue`]).
    pub source_tick: u64,
    pub agent_id: String,
    pub status: DecisionStatus,
    /// Number of OUTGOING attempts so far. `1` after the first issue;
    /// bumps on each re-issue.
    pub attempts: u32,
    /// `None` until a reply is drained.
    pub response: Option<DecisionResponse>,
    /// Last `Behind` / `InvalidAction` warning surfaced for this
    /// decision (so the inspector can render the most recent
    /// degradation alongside a still-`Pending` entry). Cleared on
    /// successful apply via [`DecisionTimeline::record_reply`].
    pub last_warning: Option<WarningPayload>,
    /// Tick at which the most recent attempt expired, if any. Useful
    /// for the inspector to show "expired at tick N, re-issued at
    /// tick M". `None` if the entry never expired.
    pub last_expired_tick: Option<u64>,
}

/// Bounded, monotonic-id sliding window of [`DecisionEntry`] rows.
///
/// Construct with [`DecisionTimeline::new`] (default capacity) or
/// [`DecisionTimeline::with_capacity`] (capped at ≥1). All mutating
/// methods that target an existing id return
/// [`DecisionTimelineError::Unknown`] if the id has been evicted —
/// callers can disambiguate "future id" vs "evicted id" via
/// [`DecisionTimeline::was_evicted`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionTimeline {
    pub schema_version: u32,
    next_id: u64,
    cap: usize,
    entries: VecDeque<DecisionEntry>,
}

/// Errors from [`DecisionTimeline`] mutators. Mostly programmer-error;
/// the engine should treat any non-Ok return as a bug.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionTimelineError {
    /// The supplied [`TimelineId`] is unknown (never allocated OR
    /// evicted from the sliding window).
    Unknown { id: TimelineId, evicted: bool },
    /// The requested status transition violates the state machine.
    IllegalTransition {
        id: TimelineId,
        from: &'static str,
        to: &'static str,
    },
}

impl std::fmt::Display for DecisionTimelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecisionTimelineError::Unknown { id, evicted } => {
                write!(f, "DecisionTimeline: unknown id={id} (evicted={evicted})")
            }
            DecisionTimelineError::IllegalTransition { id, from, to } => {
                write!(
                    f,
                    "DecisionTimeline: illegal transition id={id}: {from} -> {to}"
                )
            }
        }
    }
}

impl std::error::Error for DecisionTimelineError {}

impl Default for DecisionTimeline {
    fn default() -> Self {
        Self::new()
    }
}

impl DecisionTimeline {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_TIMELINE_CAPACITY)
    }

    /// Create a timeline with `cap` retained entries. `cap` is clamped
    /// to a minimum of 1 so the timeline always retains at least the
    /// most-recent entry.
    pub fn with_capacity(cap: usize) -> Self {
        let cap = cap.max(1);
        Self {
            schema_version: DECISION_TIMELINE_SCHEMA_VERSION,
            next_id: 1,
            cap,
            entries: VecDeque::with_capacity(cap),
        }
    }

    /// Allocate a fresh [`TimelineId`] and insert a `Pending` entry.
    /// Evicts the oldest entry if the timeline is at capacity. `next_id`
    /// is incremented unconditionally — eviction never frees ids.
    pub fn allocate(&mut self, source_tick: u64, agent_id: &str) -> TimelineId {
        let id = TimelineId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let entry = DecisionEntry {
            schema_version: DECISION_TIMELINE_SCHEMA_VERSION,
            id,
            source_tick,
            agent_id: agent_id.to_string(),
            status: DecisionStatus::Pending,
            attempts: 1,
            response: None,
            last_warning: None,
            last_expired_tick: None,
        };
        if self.entries.len() >= self.cap {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
        id
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// Smallest currently-retained [`TimelineId`], or `None` if empty.
    pub fn oldest_id(&self) -> Option<TimelineId> {
        self.entries.front().map(|e| e.id)
    }

    /// `true` iff `id` was allocated at some point but has been
    /// evicted from the sliding window. Distinguishes "unknown future
    /// id" (returns `false`) from "known but evicted" (returns
    /// `true`).
    pub fn was_evicted(&self, id: TimelineId) -> bool {
        let allocated = id.0 > 0 && id.0 < self.next_id;
        allocated && self.find_index(id).is_none()
    }

    pub fn get(&self, id: TimelineId) -> Option<&DecisionEntry> {
        self.find_index(id).map(|i| &self.entries[i])
    }

    pub fn iter(&self) -> impl Iterator<Item = &DecisionEntry> {
        self.entries.iter()
    }

    fn find_index(&self, id: TimelineId) -> Option<usize> {
        // VecDeque entries are sorted by id ascending (allocate is
        // monotonic), so we could binary search. Linear is fine at
        // cap=4096 and avoids `make_contiguous` surprises.
        self.entries.iter().position(|e| e.id == id)
    }

    fn entry_mut(&mut self, id: TimelineId) -> Result<&mut DecisionEntry, DecisionTimelineError> {
        let Some(idx) = self.find_index(id) else {
            return Err(DecisionTimelineError::Unknown {
                id,
                evicted: self.was_evicted(id),
            });
        };
        Ok(&mut self.entries[idx])
    }

    /// Record a successful on-time reply: status → `Applied`, response
    /// populated, last_warning cleared.
    ///
    /// Allowed prior status: `Pending`. Any other prior status is an
    /// illegal transition (e.g. you cannot re-apply after `Applied`,
    /// nor apply after `GaveUp`).
    pub fn record_reply(
        &mut self,
        id: TimelineId,
        applied_tick: u64,
        response: DecisionResponse,
    ) -> Result<(), DecisionTimelineError> {
        let entry = self.entry_mut(id)?;
        if !matches!(entry.status, DecisionStatus::Pending) {
            return Err(DecisionTimelineError::IllegalTransition {
                id,
                from: status_name(&entry.status),
                to: "applied",
            });
        }
        entry.status = DecisionStatus::Applied { applied_tick };
        entry.response = Some(response);
        entry.last_warning = None;
        Ok(())
    }

    /// Record that the current attempt expired. Status → `Expired`,
    /// `last_expired_tick` updated. Allowed prior status: `Pending`.
    /// After this you MUST either call [`reissue`](Self::reissue)
    /// (to bump `attempts` and flip back to `Pending`) or
    /// [`give_up`](Self::give_up) (to mark terminal).
    pub fn record_expiry(
        &mut self,
        id: TimelineId,
        expired_tick: u64,
        warning: Option<WarningPayload>,
    ) -> Result<(), DecisionTimelineError> {
        let entry = self.entry_mut(id)?;
        if !matches!(entry.status, DecisionStatus::Pending) {
            return Err(DecisionTimelineError::IllegalTransition {
                id,
                from: status_name(&entry.status),
                to: "expired",
            });
        }
        entry.status = DecisionStatus::Expired { expired_tick };
        entry.last_expired_tick = Some(expired_tick);
        if warning.is_some() {
            entry.last_warning = warning;
        }
        Ok(())
    }

    /// Record a re-issue: status flips from `Expired` back to
    /// `Pending`, `attempts` bumps. The `TimelineId` is preserved.
    /// Allowed prior status: `Expired`.
    pub fn reissue(&mut self, id: TimelineId) -> Result<(), DecisionTimelineError> {
        let entry = self.entry_mut(id)?;
        if !matches!(entry.status, DecisionStatus::Expired { .. }) {
            return Err(DecisionTimelineError::IllegalTransition {
                id,
                from: status_name(&entry.status),
                to: "pending(reissue)",
            });
        }
        entry.attempts = entry.attempts.saturating_add(1);
        entry.status = DecisionStatus::Pending;
        Ok(())
    }

    /// Record terminal give-up after re-issue cap exhausted. Allowed
    /// prior status: `Expired`.
    pub fn give_up(
        &mut self,
        id: TimelineId,
        last_tick: u64,
        warning: Option<WarningPayload>,
    ) -> Result<(), DecisionTimelineError> {
        let entry = self.entry_mut(id)?;
        if !matches!(entry.status, DecisionStatus::Expired { .. }) {
            return Err(DecisionTimelineError::IllegalTransition {
                id,
                from: status_name(&entry.status),
                to: "gave_up",
            });
        }
        entry.status = DecisionStatus::GaveUp { last_tick };
        if warning.is_some() {
            entry.last_warning = warning;
        }
        Ok(())
    }

    /// Record terminal bridge fault. Allowed prior status: `Pending`
    /// or `Expired` (a fault can happen mid-flight or instead of a
    /// reissue attempt).
    pub fn record_fault(
        &mut self,
        id: TimelineId,
        fault_tick: u64,
        reason: impl Into<String>,
    ) -> Result<(), DecisionTimelineError> {
        let entry = self.entry_mut(id)?;
        if !matches!(
            entry.status,
            DecisionStatus::Pending | DecisionStatus::Expired { .. }
        ) {
            return Err(DecisionTimelineError::IllegalTransition {
                id,
                from: status_name(&entry.status),
                to: "faulted",
            });
        }
        entry.status = DecisionStatus::Faulted {
            fault_tick,
            reason: reason.into(),
        };
        Ok(())
    }

    /// Snapshot for bundle export. Excludes operational fields
    /// (`next_id`, `cap`) so the on-disk shape is `{schema_version,
    /// entries}` only — cleaner contract for replay consumers and
    /// avoids depending on internals that may evolve.
    pub fn snapshot(&self) -> DecisionTimelineSnapshot {
        DecisionTimelineSnapshot {
            schema_version: self.schema_version,
            entries: self.entries.iter().cloned().collect(),
        }
    }
}

fn status_name(s: &DecisionStatus) -> &'static str {
    match s {
        DecisionStatus::Pending => "pending",
        DecisionStatus::Applied { .. } => "applied",
        DecisionStatus::Expired { .. } => "expired",
        DecisionStatus::GaveUp { .. } => "gave_up",
        DecisionStatus::Faulted { .. } => "faulted",
    }
}

/// Bundle-export view of a [`DecisionTimeline`]. Stable JSON contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionTimelineSnapshot {
    pub schema_version: u32,
    pub entries: Vec<DecisionEntry>,
}

impl DecisionTimelineSnapshot {
    /// Validate the snapshot for schema compatibility. Rejects:
    /// - snapshot-level `schema_version != DECISION_TIMELINE_SCHEMA_VERSION`
    /// - any entry with `schema_version != DECISION_TIMELINE_SCHEMA_VERSION`
    pub fn validate(&self) -> Result<(), SchemaVersionMismatch> {
        if self.schema_version != DECISION_TIMELINE_SCHEMA_VERSION {
            return Err(SchemaVersionMismatch {
                where_: "snapshot",
                got: self.schema_version,
                expected: DECISION_TIMELINE_SCHEMA_VERSION,
            });
        }
        for entry in &self.entries {
            if entry.schema_version != DECISION_TIMELINE_SCHEMA_VERSION {
                return Err(SchemaVersionMismatch {
                    where_: "entry",
                    got: entry.schema_version,
                    expected: DECISION_TIMELINE_SCHEMA_VERSION,
                });
            }
        }
        Ok(())
    }

    /// Parse + validate from JSON in one step. The intended public
    /// decode path; ensures schema mismatch is always rejected.
    pub fn from_json_str(s: &str) -> Result<Self, FromJsonError> {
        let snap: Self = serde_json::from_str(s).map_err(FromJsonError::Parse)?;
        snap.validate().map_err(FromJsonError::Schema)?;
        Ok(snap)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaVersionMismatch {
    pub where_: &'static str,
    pub got: u32,
    pub expected: u32,
}

impl std::fmt::Display for SchemaVersionMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DecisionTimeline {} schema_version mismatch: got {}, expected {}",
            self.where_, self.got, self.expected
        )
    }
}

impl std::error::Error for SchemaVersionMismatch {}

#[derive(Debug)]
pub enum FromJsonError {
    Parse(serde_json::Error),
    Schema(SchemaVersionMismatch),
}

impl std::fmt::Display for FromJsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FromJsonError::Parse(e) => write!(f, "DecisionTimeline parse: {e}"),
            FromJsonError::Schema(e) => write!(f, "DecisionTimeline schema: {e}"),
        }
    }
}

impl std::error::Error for FromJsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FromJsonError::Parse(e) => Some(e),
            FromJsonError::Schema(e) => Some(e),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn applied_response() -> DecisionResponse {
        DecisionResponse {
            chosen: Some(Action::NoOp),
            rationale: "did nothing".to_string(),
            confidence: 0.5,
            latency_ms: Some(42),
            raw_response_ref: Some(RawResponseRef {
                scene_id: "metro-pulse".to_string(),
                byte_offset: 1024,
                byte_len: 256,
            }),
        }
    }

    #[test]
    fn allocate_produces_monotonic_ids_starting_at_one() {
        let mut t = DecisionTimeline::new();
        let a = t.allocate(10, "agent-a");
        let b = t.allocate(10, "agent-b");
        let c = t.allocate(11, "agent-a");
        assert_eq!(a, TimelineId(1));
        assert_eq!(b, TimelineId(2));
        assert_eq!(c, TimelineId(3));
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn allocate_after_eviction_keeps_monotonic_ids() {
        let mut t = DecisionTimeline::with_capacity(2);
        let _id1 = t.allocate(1, "a");
        let _id2 = t.allocate(2, "a");
        let id3 = t.allocate(3, "a");
        let id4 = t.allocate(4, "a");
        let id5 = t.allocate(5, "a");
        assert_eq!(id3, TimelineId(3));
        assert_eq!(id4, TimelineId(4));
        assert_eq!(id5, TimelineId(5));
        assert_eq!(t.len(), 2);
        // Retained: ids 4 and 5.
        assert_eq!(
            t.iter().map(|e| e.id).collect::<Vec<_>>(),
            vec![TimelineId(4), TimelineId(5)]
        );
    }

    #[test]
    fn with_capacity_zero_clamps_to_one() {
        let t = DecisionTimeline::with_capacity(0);
        assert_eq!(t.capacity(), 1);
    }

    #[test]
    fn get_returns_none_for_evicted_and_future_ids() {
        let mut t = DecisionTimeline::with_capacity(2);
        let id1 = t.allocate(1, "a");
        t.allocate(2, "a");
        t.allocate(3, "a");
        assert!(t.get(id1).is_none(), "id1 was evicted");
        assert!(t.was_evicted(id1));
        // Future id (never allocated) → not evicted, just unknown.
        let future = TimelineId(99);
        assert!(t.get(future).is_none());
        assert!(!t.was_evicted(future));
    }

    #[test]
    fn oldest_id_tracks_eviction() {
        let mut t = DecisionTimeline::with_capacity(2);
        assert!(t.oldest_id().is_none());
        let id1 = t.allocate(1, "a");
        assert_eq!(t.oldest_id(), Some(id1));
        t.allocate(2, "a");
        t.allocate(3, "a");
        assert_eq!(t.oldest_id(), Some(TimelineId(2)));
    }

    #[test]
    fn record_reply_transitions_pending_to_applied() {
        let mut t = DecisionTimeline::new();
        let id = t.allocate(10, "agent-a");
        t.record_reply(id, 12, applied_response()).unwrap();
        let e = t.get(id).unwrap();
        assert!(matches!(
            e.status,
            DecisionStatus::Applied { applied_tick: 12 }
        ));
        assert!(e.response.is_some());
        assert!(e.last_warning.is_none());
    }

    #[test]
    fn record_reply_on_non_pending_is_illegal() {
        let mut t = DecisionTimeline::new();
        let id = t.allocate(10, "agent-a");
        t.record_reply(id, 12, applied_response()).unwrap();
        let err = t.record_reply(id, 13, applied_response()).unwrap_err();
        assert!(matches!(
            err,
            DecisionTimelineError::IllegalTransition {
                from: "applied",
                to: "applied",
                ..
            }
        ));
    }

    #[test]
    fn full_reissue_cycle_preserves_id_and_bumps_attempts() {
        let mut t = DecisionTimeline::new();
        let id = t.allocate(10, "agent-a");
        // attempt 1 expires
        t.record_expiry(id, 20, None).unwrap();
        assert!(matches!(
            t.get(id).unwrap().status,
            DecisionStatus::Expired { expired_tick: 20 }
        ));
        assert_eq!(t.get(id).unwrap().last_expired_tick, Some(20));
        // re-issue → attempt 2 pending
        t.reissue(id).unwrap();
        assert!(matches!(t.get(id).unwrap().status, DecisionStatus::Pending));
        assert_eq!(t.get(id).unwrap().attempts, 2);
        // attempt 2 expires
        t.record_expiry(id, 30, None).unwrap();
        // give up
        t.give_up(id, 30, None).unwrap();
        assert!(matches!(
            t.get(id).unwrap().status,
            DecisionStatus::GaveUp { last_tick: 30 }
        ));
        assert!(t.get(id).unwrap().status.is_terminal());
    }

    #[test]
    fn reissue_only_from_expired() {
        let mut t = DecisionTimeline::new();
        let id = t.allocate(10, "a");
        let err = t.reissue(id).unwrap_err();
        assert!(matches!(
            err,
            DecisionTimelineError::IllegalTransition {
                from: "pending",
                ..
            }
        ));
    }

    #[test]
    fn record_fault_allowed_from_pending_or_expired() {
        let mut t = DecisionTimeline::new();
        let a = t.allocate(10, "a");
        t.record_fault(a, 11, "engine fault").unwrap();
        assert!(matches!(
            t.get(a).unwrap().status,
            DecisionStatus::Faulted { fault_tick: 11, .. }
        ));
        let b = t.allocate(10, "b");
        t.record_expiry(b, 20, None).unwrap();
        t.record_fault(b, 21, "bridge crash").unwrap();
        assert!(matches!(
            t.get(b).unwrap().status,
            DecisionStatus::Faulted { .. }
        ));
    }

    #[test]
    fn record_fault_from_terminal_is_illegal() {
        let mut t = DecisionTimeline::new();
        let id = t.allocate(10, "a");
        t.record_reply(id, 11, applied_response()).unwrap();
        let err = t.record_fault(id, 12, "x").unwrap_err();
        assert!(matches!(
            err,
            DecisionTimelineError::IllegalTransition { .. }
        ));
    }

    #[test]
    fn unknown_id_returns_evicted_flag_correctly() {
        let mut t = DecisionTimeline::with_capacity(2);
        let id1 = t.allocate(1, "a");
        t.allocate(2, "a");
        t.allocate(3, "a"); // evicts id1
        let err = t.record_reply(id1, 5, applied_response()).unwrap_err();
        assert_eq!(
            err,
            DecisionTimelineError::Unknown {
                id: id1,
                evicted: true,
            }
        );
        let future = TimelineId(99);
        let err = t.record_reply(future, 5, applied_response()).unwrap_err();
        assert_eq!(
            err,
            DecisionTimelineError::Unknown {
                id: future,
                evicted: false,
            }
        );
    }

    #[test]
    fn iter_yields_entries_in_insertion_order() {
        let mut t = DecisionTimeline::new();
        t.allocate(1, "a");
        t.allocate(2, "b");
        t.allocate(3, "c");
        let ids: Vec<TimelineId> = t.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![TimelineId(1), TimelineId(2), TimelineId(3)]);
    }

    #[test]
    fn snapshot_round_trips_through_json() {
        let mut t = DecisionTimeline::new();
        let id = t.allocate(10, "agent-a");
        t.record_reply(id, 11, applied_response()).unwrap();
        let snap = t.snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        let back = DecisionTimelineSnapshot::from_json_str(&json).unwrap();
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].id, id);
        assert_eq!(back.entries[0].agent_id, "agent-a");
        assert!(matches!(
            back.entries[0].status,
            DecisionStatus::Applied { applied_tick: 11 }
        ));
        let resp = back.entries[0].response.as_ref().unwrap();
        assert_eq!(resp.latency_ms, Some(42));
        assert_eq!(
            resp.raw_response_ref.as_ref().unwrap().scene_id,
            "metro-pulse"
        );
    }

    #[test]
    fn snapshot_validate_rejects_wrong_schema_version() {
        let mut snap = DecisionTimelineSnapshot {
            schema_version: 99,
            entries: vec![],
        };
        assert!(snap.validate().is_err());
        snap.schema_version = DECISION_TIMELINE_SCHEMA_VERSION;
        snap.entries.push(DecisionEntry {
            schema_version: 7,
            id: TimelineId(1),
            source_tick: 0,
            agent_id: "x".into(),
            status: DecisionStatus::Pending,
            attempts: 1,
            response: None,
            last_warning: None,
            last_expired_tick: None,
        });
        let err = snap.validate().unwrap_err();
        assert_eq!(err.where_, "entry");
        assert_eq!(err.got, 7);
        assert_eq!(err.expected, DECISION_TIMELINE_SCHEMA_VERSION);
    }

    #[test]
    fn snapshot_from_json_str_rejects_malformed() {
        assert!(matches!(
            DecisionTimelineSnapshot::from_json_str("not json"),
            Err(FromJsonError::Parse(_))
        ));
    }

    #[test]
    fn snapshot_excludes_operational_fields_from_json() {
        // Bundle-export contract: snapshot JSON must NOT leak next_id
        // or cap. Stability test — if you change the shape, bump
        // DECISION_TIMELINE_SCHEMA_VERSION.
        let mut t = DecisionTimeline::with_capacity(8);
        t.allocate(1, "a");
        let snap = t.snapshot();
        let json = serde_json::to_string(&snap).unwrap();
        assert!(
            !json.contains("next_id"),
            "snapshot must not expose next_id: {json}"
        );
        assert!(
            !json.contains("\"cap\""),
            "snapshot must not expose cap: {json}"
        );
        assert!(json.contains("\"schema_version\""));
        assert!(json.contains("\"entries\""));
    }

    #[test]
    fn record_expiry_carries_warning_through_to_entry() {
        let mut t = DecisionTimeline::new();
        let id = t.allocate(10, "agent-a");
        let warning = WarningPayload::Behind {
            lag_frames: 5,
            agent_id: Some("agent-a".to_string()),
        };
        t.record_expiry(id, 20, Some(warning.clone())).unwrap();
        let e = t.get(id).unwrap();
        assert_eq!(
            e.last_warning
                .as_ref()
                .map(|w| matches!(w, WarningPayload::Behind { .. })),
            Some(true)
        );
    }

    #[test]
    fn default_capacity_is_4096() {
        let t = DecisionTimeline::new();
        assert_eq!(t.capacity(), DEFAULT_TIMELINE_CAPACITY);
        assert_eq!(DEFAULT_TIMELINE_CAPACITY, 4096);
    }
}
