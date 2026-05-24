//! AgentLog — append-only JSONL log of agent decisions (PLAN §15).
//!
//! Each agent action produces one line. The log lives on disk under
//! `~/.local/share/simetro/logs/` or a path the embedder picks. When
//! the sink fails (disk full, permission denied) the log falls back
//! to an in-memory ring buffer so the simulation never blocks; the
//! engine surfaces this via `Warning::AgentLogSlow` (PLAN §13 chaos
//! test 3, §17.3).
//!
//! ```text
//!   tick → observation → agent.act() → AgentReport
//!                                          │
//!                                          ▼
//!                                   ┌──────────────┐
//!                                   │  AgentLog    │
//!                                   │              │
//!                       try write ──▶ Sink (file)  │
//!                                   │     │ on err │
//!                                   │     ▼        │
//!                                   │   ring        │
//!                                   │ (bounded VecDeque)
//!                                   └──────────────┘
//! ```
//!
//! Replay (P2) reads the log back: re-emit the parsed action without
//! re-invoking the LLM. Captured `observation_hash` validates that the
//! engine reached the same point on the same seed (PLAN §16).
//!
//! ## Schema version v1 → v2 (P2.A0.5)
//!
//! Schema v2 is additive: every new field is `Option<T>` with serde
//! defaults, so v1 jsonl rows still load through the v2 deserializer
//! and produce the same in-memory `AgentLogEntry` with the new fields
//! as `None`. The `schema_version` field on the entry is what
//! distinguishes them on the wire; it defaults to 2 on serialize.
//!
//! New v2 fields cover live-LLM provenance: `backend`, `model`,
//! `latency_ms`, `prompt_tokens`, `completion_tokens`. `raw_response`
//! existed in v1 but is now subject to a hard 64 KiB cap with a
//! `truncated_bytes` marker on the row when capping fires (per the
//! P2.A0.4 security threat model §5.6). The secret-pattern redactor
//! lands in a follow-up PR; today the cap and the migration shim
//! are in place so a future live backend cannot accidentally land
//! arbitrarily large unredacted rows.
//!
//! Path derivation for file-backed logs uses `dirs::data_dir() /
//! "simetro" / <validated-scene-id> / decisions-<timestamp>.jsonl`
//! with file mode `0o600` on Unix (P2.A0.4 §5.1 + §5.2).

use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use simetro_protocol::{Action, WarningPayload};

use crate::agent::Observation;
use crate::components::MoverState;

const DEFAULT_RING_CAP: usize = 4096;

/// Current AgentLog schema version. v2 was introduced by P2.A0.5.
pub const SCHEMA_VERSION: u32 = 2;

/// Hard cap on the size of the `raw_response` field, in bytes. Larger
/// LLM replies are truncated and the entry records the original size
/// in `truncated_bytes` so downstream tooling can detect the truncation.
/// Per P2.A0.4 §5.6 (64 KiB ceiling).
pub const RAW_RESPONSE_MAX_BYTES: usize = 64 * 1024;

/// Maximum length of an `agent_id` accepted in an entry. Defends
/// against degenerate or attacker-controlled IDs in scene JSON.
pub const AGENT_ID_MAX_LEN: usize = 256;

/// Maximum length of a `rationale` accepted in an entry. Matches the
/// in-engine cap from `crate::agent::MAX_RATIONALE_CHARS`.
pub const RATIONALE_MAX_LEN: usize = 4096;

/// Maximum length of `backend` / `model` strings.
pub const PROVENANCE_STR_MAX_LEN: usize = 128;

fn default_schema_version() -> u32 {
    // When a v1 row (no `schema_version` field) is loaded through the
    // v2 deserializer, serde inserts `1` here. This is the migration
    // shim's anchor: any code that needs to distinguish v1-from-disk
    // vs v2-from-disk reads this field.
    1
}

/// One line in the AgentLog. Serializes as a single JSON object.
///
/// **Schema versioning:** the `schema_version` field is `2` on entries
/// produced by this build; it defaults to `1` when deserializing a
/// v1 jsonl line that lacks the field, which is how v1 → v2 migration
/// works without rewriting the historical log on load. The field is
/// **always emitted** on serialize (no `skip_serializing_if`) so that
/// v1 and v2 rows are unambiguous on disk; replay tooling can
/// distinguish them and apply the correct decoder.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentLogEntry {
    /// Wire-protocol schema version of this row. Defaults to 1 on
    /// deserialize so legacy rows load cleanly; always emitted on
    /// serialize.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub tick: u64,
    pub agent_id: String,
    /// Deterministic FNV-1a hash of the observation. Replay uses this
    /// to verify the engine reached the same state on a re-run.
    pub observation_hash: u64,
    /// Raw text from an LLM backend, if any. Native built-in agents
    /// leave this `None`. Capped at `RAW_RESPONSE_MAX_BYTES` by
    /// `AgentLogEntry::new`; when capping fires, `truncated_bytes`
    /// records the original size in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_response: Option<String>,
    /// Original byte length of `raw_response` BEFORE truncation, set
    /// only when the writer truncated the response to fit
    /// `RAW_RESPONSE_MAX_BYTES`. Absent on rows that were never
    /// truncated. (v2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncated_bytes: Option<usize>,
    pub parsed_action: Option<Action>,
    pub considered_count: usize,
    pub rationale: String,
    /// Backend identifier (e.g. `"copilot"`, `"mock"`, `"speed_tuner"`).
    /// Optional + back-compat with v1. (v2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// Model identifier (e.g. `"gpt-5-mini"`). Optional. (v2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// End-to-end latency from outbox-enqueue to inbox-drain, in
    /// milliseconds. Optional. (v2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u32>,
    /// Tokens consumed by the prompt, as reported by the backend.
    /// Optional. (v2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u32>,
    /// Tokens consumed by the completion, as reported by the backend.
    /// Optional. (v2)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,
}

/// Provenance attached to an `AgentLogEntry` by the live-LLM bridge.
/// Built-in deterministic agents pass `LlmProvenance::default()` (all
/// fields `None`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LlmProvenance {
    pub backend: Option<String>,
    pub model: Option<String>,
    pub latency_ms: Option<u32>,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
}

impl AgentLogEntry {
    /// Build a v1-compatible entry from an agent's observation + report.
    /// Computes the observation hash for replay verification. No live-
    /// LLM provenance (backend / model / token counts) is recorded;
    /// for that, use `AgentLogEntry::with_provenance`.
    #[must_use]
    pub fn new(
        obs: &Observation,
        agent_id: &str,
        chosen: Option<Action>,
        considered_count: usize,
        rationale: String,
        raw_response: Option<String>,
    ) -> Self {
        Self::with_provenance(
            obs,
            agent_id,
            chosen,
            considered_count,
            rationale,
            raw_response,
            LlmProvenance::default(),
        )
    }

    /// Build a v2 entry including LLM provenance (backend / model /
    /// latency / token counts). Used by the live-LLM bridge. The
    /// `raw_response` is capped at `RAW_RESPONSE_MAX_BYTES`; if the
    /// caller supplied a larger string, the resulting entry stores
    /// the truncated head and records the original byte length in
    /// `truncated_bytes`. UTF-8 boundary safety is preserved.
    #[must_use]
    pub fn with_provenance(
        obs: &Observation,
        agent_id: &str,
        chosen: Option<Action>,
        considered_count: usize,
        rationale: String,
        raw_response: Option<String>,
        provenance: LlmProvenance,
    ) -> Self {
        let (raw_response, truncated_bytes) = cap_raw_response(raw_response);
        Self {
            schema_version: SCHEMA_VERSION,
            tick: obs.tick,
            agent_id: agent_id.to_string(),
            observation_hash: observation_hash(obs),
            raw_response,
            truncated_bytes,
            parsed_action: chosen,
            considered_count,
            rationale,
            backend: provenance.backend,
            model: provenance.model,
            latency_ms: provenance.latency_ms,
            prompt_tokens: provenance.prompt_tokens,
            completion_tokens: provenance.completion_tokens,
        }
    }
}

/// Cap the input string at `RAW_RESPONSE_MAX_BYTES`, taking care to
/// truncate at a UTF-8 boundary so the resulting string is always
/// valid. Returns `(Some(capped), Some(original_len))` when truncation
/// fired, or `(Some(s), None)` / `(None, None)` otherwise.
fn cap_raw_response(s: Option<String>) -> (Option<String>, Option<usize>) {
    let s = match s {
        None => return (None, None),
        Some(s) => s,
    };
    if s.len() <= RAW_RESPONSE_MAX_BYTES {
        return (Some(s), None);
    }
    let original_len = s.len();
    let mut cap = RAW_RESPONSE_MAX_BYTES;
    while cap > 0 && !s.is_char_boundary(cap) {
        cap -= 1;
    }
    let mut capped = s;
    capped.truncate(cap);
    (Some(capped), Some(original_len))
}

/// Schema validation: reject rows whose fields are obviously
/// malformed before they reach the disk. This is the
/// PR #4 sec Finding 4 control 3 ("validate before persist") and
/// the P2.A0.4 §5.4 fence. Returns `Ok(())` for valid entries; a
/// typed `Err(SchemaError)` for the rest.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SchemaError {
    #[error("schema_version {0} not supported (max {SCHEMA_VERSION})")]
    UnsupportedVersion(u32),
    #[error("agent_id is empty")]
    EmptyAgentId,
    #[error("agent_id is {0} bytes (max {AGENT_ID_MAX_LEN})")]
    AgentIdTooLong(usize),
    #[error("rationale is {0} bytes (max {RATIONALE_MAX_LEN})")]
    RationaleTooLong(usize),
    #[error("raw_response exceeded {RAW_RESPONSE_MAX_BYTES} bytes without truncated_bytes marker")]
    UncappedRawResponse,
    /// truncated_bytes set but raw_response is None — the marker
    /// claims a truncation happened, but there is no data to mark.
    #[error("truncated_bytes set without raw_response")]
    TruncatedBytesWithoutResponse,
    /// truncated_bytes set but the (capped) raw_response is ≤ MAX
    /// bytes — the marker claims a truncation that didn't actually
    /// fire. Catches struct-literal construction that forgot to call
    /// `cap_raw_response`.
    #[error("truncated_bytes={0} set but raw_response is only {1} bytes (≤ cap)")]
    TruncatedBytesOnSmallResponse(usize, usize),
    /// truncated_bytes ≤ raw_response.len() — logically impossible
    /// because the truncated form must be SHORTER than the original.
    /// Catches a struct-literal construction that swapped the
    /// original / capped lengths.
    #[error("truncated_bytes={0} not greater than current raw_response length {1}")]
    TruncatedBytesNotGreaterThanCurrent(usize, usize),
    #[error("backend string is {0} bytes (max {PROVENANCE_STR_MAX_LEN})")]
    BackendTooLong(usize),
    #[error("model string is {0} bytes (max {PROVENANCE_STR_MAX_LEN})")]
    ModelTooLong(usize),
}

pub fn validate_entry(entry: &AgentLogEntry) -> Result<(), SchemaError> {
    if entry.schema_version == 0 || entry.schema_version > SCHEMA_VERSION {
        return Err(SchemaError::UnsupportedVersion(entry.schema_version));
    }
    if entry.agent_id.is_empty() {
        return Err(SchemaError::EmptyAgentId);
    }
    if entry.agent_id.len() > AGENT_ID_MAX_LEN {
        return Err(SchemaError::AgentIdTooLong(entry.agent_id.len()));
    }
    if entry.rationale.len() > RATIONALE_MAX_LEN {
        return Err(SchemaError::RationaleTooLong(entry.rationale.len()));
    }
    // raw_response + truncated_bytes consistency.
    match (&entry.raw_response, entry.truncated_bytes) {
        // Most common: no raw_response, no marker.
        (None, None) => {}
        // Marker without data is impossible.
        (None, Some(_)) => return Err(SchemaError::TruncatedBytesWithoutResponse),
        // Data without marker: must be at-or-under the cap.
        (Some(raw), None) => {
            if raw.len() > RAW_RESPONSE_MAX_BYTES {
                return Err(SchemaError::UncappedRawResponse);
            }
        }
        // Data with marker: the marker (= original byte length before
        // truncation) must be GREATER than the kept raw.len() (so
        // truncation actually dropped bytes) AND must be > MAX (so
        // the cap was the reason truncation happened — the only
        // truncation reason today). Note raw.len() ≤ MAX is the
        // post-cap invariant but can legitimately be < MAX due to
        // UTF-8 boundary backoff.
        (Some(raw), Some(orig)) => {
            if orig <= RAW_RESPONSE_MAX_BYTES {
                return Err(SchemaError::TruncatedBytesOnSmallResponse(orig, raw.len()));
            }
            if orig <= raw.len() {
                return Err(SchemaError::TruncatedBytesNotGreaterThanCurrent(
                    orig,
                    raw.len(),
                ));
            }
        }
    }
    if let Some(b) = &entry.backend {
        if b.len() > PROVENANCE_STR_MAX_LEN {
            return Err(SchemaError::BackendTooLong(b.len()));
        }
    }
    if let Some(m) = &entry.model {
        if m.len() > PROVENANCE_STR_MAX_LEN {
            return Err(SchemaError::ModelTooLong(m.len()));
        }
    }
    Ok(())
}

/// Validate that a `scene_id` is safe to use as a path component.
/// Mirrors the registry's contract (`^[A-Za-z0-9_-]{1,64}$`) so the
/// log writer can construct paths without re-validating the scene
/// registry's invariants — but the writer enforces this anyway for
/// defense in depth (P2.A0.4 §5.1).
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SceneIdError {
    #[error("scene_id is empty")]
    Empty,
    #[error("scene_id is {0} chars (max 64)")]
    TooLong(usize),
    #[error("scene_id contains disallowed character at byte offset {0}")]
    InvalidChar(usize),
}

pub fn validate_scene_id(scene_id: &str) -> Result<(), SceneIdError> {
    if scene_id.is_empty() {
        return Err(SceneIdError::Empty);
    }
    if scene_id.len() > 64 {
        return Err(SceneIdError::TooLong(scene_id.len()));
    }
    for (i, b) in scene_id.bytes().enumerate() {
        let ok = b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
        if !ok {
            return Err(SceneIdError::InvalidChar(i));
        }
    }
    Ok(())
}

/// Build the per-scene AgentLog directory under the platform data
/// dir. Used by `AgentLog::open_for_scene`. Public so callers can
/// inspect the chosen path before creating the file (useful for
/// runbook diagnostics).
pub fn agent_log_dir(scene_id: &str) -> Result<PathBuf, SceneIdError> {
    validate_scene_id(scene_id)?;
    let mut base = data_dir_for_simetro();
    base.push(scene_id);
    Ok(base)
}

fn data_dir_for_simetro() -> PathBuf {
    // Avoid pulling in the `dirs` crate to keep engine deps minimal.
    // Fall through the same priority dirs::data_dir uses on each OS.
    if let Ok(custom) = std::env::var("SIMETRO_DATA_DIR") {
        return PathBuf::from(custom).join("simetro");
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            if !xdg.is_empty() {
                return PathBuf::from(xdg).join("simetro");
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".local/share/simetro");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join("Library/Application Support/simetro");
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("simetro");
        }
    }
    // Fallback: current directory subdir. Better than panicking; the
    // caller can always supply a path via `AgentLog::open_file`.
    PathBuf::from("./.simetro")
}

fn timestamp_suffix() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

/// File mode for newly-created log files. Only meaningful on Unix; on
/// other platforms the value is ignored.
#[derive(Debug, Clone, Copy)]
struct FileMode {
    #[cfg_attr(not(unix), allow(dead_code))]
    unix_mode: u32,
}

impl FileMode {
    /// Owner read/write only. Used by `open_for_scene` to satisfy
    /// P2.A0.4 §5.2 (file mode 0o600). On Windows the value is
    /// ignored; ACL hardening is deferred to a follow-up PR per
    /// spec §13.
    const fn owner_only() -> Self {
        Self { unix_mode: 0o600 }
    }

    fn unix_mode(self) -> u32 {
        self.unix_mode
    }
}

impl Default for FileMode {
    /// Default mode preserves the historical behavior of `open_file`:
    /// the OS umask applies. Embedders that need 0o600 should use
    /// `open_for_scene` (which always pins owner-only).
    fn default() -> Self {
        // 0o644 = OS-typical-default-after-umask; we set explicitly to
        // make the intent visible in the diff. Embedders who want
        // 0o600 should use `open_for_scene`.
        Self { unix_mode: 0o644 }
    }
}

/// Deterministic 64-bit hash of an observation. FNV-1a (no random
/// seed) so two runs of the same scene + seed produce identical
/// hashes (PLAN §16).
#[must_use]
pub fn observation_hash(obs: &Observation) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mix = |h: &mut u64, x: u64| {
        *h ^= x;
        *h = h.wrapping_mul(0x0000_0100_0000_01B3);
    };
    mix(&mut h, obs.tick);
    for m in &obs.movers {
        mix(&mut h, u64::from(m.id.0));
        mix(&mut h, u64::from(m.speed.to_bits()));
        mix(&mut h, u64::from(m.home_path.0));
        match m.state {
            MoverState::Empty => mix(&mut h, 0xE0),
            MoverState::Waiting { at } => {
                mix(&mut h, 0xE1);
                mix(&mut h, u64::from(at.0));
            }
            MoverState::Traveling { path, progress } => {
                mix(&mut h, 0xE2);
                mix(&mut h, u64::from(path.0));
                mix(&mut h, u64::from(progress.to_bits()));
            }
        }
    }
    h
}

/// Append-only log writer with a ring-buffer fallback.
pub struct AgentLog {
    sink: Box<dyn Write + Send>,
    ring: VecDeque<String>,
    ring_cap: usize,
    /// True once a sink failure has caused us to fall back to the ring.
    degraded: bool,
    /// Number of entries dropped because the ring was full while
    /// degraded.
    dropped: u64,
}

impl AgentLog {
    /// Wrap a writer (typically a `BufWriter<File>`). The ring
    /// fallback holds at most `DEFAULT_RING_CAP` lines.
    pub fn new(sink: Box<dyn Write + Send>) -> Self {
        Self::with_capacity(sink, DEFAULT_RING_CAP)
    }

    pub fn with_capacity(sink: Box<dyn Write + Send>, ring_cap: usize) -> Self {
        Self {
            sink,
            ring: VecDeque::with_capacity(ring_cap.min(64)),
            ring_cap,
            degraded: false,
            dropped: 0,
        }
    }

    /// Open (or create + append to) a file-backed log at `path`. Parent
    /// directories are created if missing.
    ///
    /// # Errors
    /// Propagates any IO error from creating the directory or opening
    /// the file.
    pub fn open_file(path: &Path) -> std::io::Result<Self> {
        Self::open_file_with_mode(path, FileMode::default())
    }

    /// Open (or create + append to) a file-backed log under the
    /// platform's data dir, scoped to a registry-validated `scene_id`.
    ///
    /// Path: `data_dir() / "simetro" / <scene_id> / decisions-<ts>.jsonl`.
    /// On Unix the file is created with mode `0o600`.
    /// The `scene_id` MUST satisfy `validate_scene_id`; this is
    /// defense in depth against path traversal even if the upstream
    /// registry was bypassed (P2.A0.4 §5.1).
    ///
    /// # Errors
    /// Returns `io::ErrorKind::InvalidInput` wrapping `SceneIdError`
    /// for an unsafe scene_id; propagates any other IO error from
    /// directory creation or file open.
    pub fn open_for_scene(scene_id: &str) -> std::io::Result<Self> {
        let dir = agent_log_dir(scene_id)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))?;
        let filename = format!("decisions-{}.jsonl", timestamp_suffix());
        let path = dir.join(filename);
        Self::open_file_with_mode(&path, FileMode::owner_only())
    }

    fn open_file_with_mode(path: &Path, mode: FileMode) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(mode.unix_mode());
        }
        let _ = mode; // silence unused on non-unix
        let file = opts.open(path)?;
        let writer = std::io::BufWriter::new(file);
        Ok(Self::new(Box::new(writer)))
    }

    /// Force the log into degraded (ring) mode. Used by chaos tests
    /// (PLAN §17.3 slow_agent_log_disk).
    pub fn force_degrade(&mut self) {
        self.degraded = true;
    }

    /// True iff a sink failure has switched us to the ring buffer.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    /// Count of entries dropped because the ring was full while in
    /// degraded mode.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Snapshot of the ring contents (for tests and replay tooling).
    #[must_use]
    pub fn ring_snapshot(&self) -> Vec<String> {
        self.ring.iter().cloned().collect()
    }

    /// Append one entry.
    ///
    /// Returns `Some(WarningPayload::AgentLogSlow)` the first time we
    /// fall back to the ring so the caller can surface it via
    /// `SimMessage::Warning`. Subsequent failures while already
    /// degraded return `None` (caller already knows).
    ///
    /// **Schema validation:** the entry is validated against
    /// `validate_entry` BEFORE serialization. A validation failure is
    /// treated as a degradation event: the row is dropped (NEVER
    /// written to disk), a counter increments, and the first failure
    /// emits `WarningPayload::AgentLogSlow` (same single-warning
    /// semantics as sink failures). This is the P2.A0.4 §5.4 control.
    pub fn append(&mut self, entry: &AgentLogEntry) -> Option<WarningPayload> {
        if let Err(e) = validate_entry(entry) {
            tracing::warn!(
                error = %e,
                agent_id = %entry.agent_id,
                "AgentLog: row failed schema validation; dropped"
            );
            self.dropped = self.dropped.saturating_add(1);
            return self.first_degrade();
        }

        let line = match serde_json::to_string(entry) {
            Ok(s) => s,
            Err(_) => {
                // Serialization failure is engine-internal and should
                // never happen with our types; record and degrade.
                self.push_ring(format!(
                    "{{\"error\":\"serialize\",\"agent_id\":\"{}\"}}",
                    entry.agent_id
                ));
                return self.first_degrade();
            }
        };

        if self.degraded {
            self.push_ring(line);
            return None;
        }

        // Try sink. On error, switch to ring and signal once.
        let res = (|| -> std::io::Result<()> {
            self.sink.write_all(line.as_bytes())?;
            self.sink.write_all(b"\n")?;
            Ok(())
        })();

        if res.is_err() {
            self.push_ring(line);
            return self.first_degrade();
        }

        None
    }

    /// Flush the underlying sink. Returns the underlying io error
    /// without degrading (caller decides).
    ///
    /// # Errors
    /// Whatever the sink's flush returned.
    pub fn flush(&mut self) -> std::io::Result<()> {
        self.sink.flush()
    }

    fn push_ring(&mut self, line: String) {
        if self.ring.len() >= self.ring_cap {
            self.ring.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.ring.push_back(line);
    }

    fn first_degrade(&mut self) -> Option<WarningPayload> {
        if !self.degraded {
            self.degraded = true;
            Some(WarningPayload::AgentLogSlow)
        } else {
            None
        }
    }
}

impl std::fmt::Debug for AgentLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentLog")
            .field("degraded", &self.degraded)
            .field("ring_len", &self.ring.len())
            .field("ring_cap", &self.ring_cap)
            .field("dropped", &self.dropped)
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::agent::MoverObservation;
    use crate::components::{MoverId, NodeId, PathId};
    use std::io;

    fn obs() -> Observation {
        Observation {
            tick: 7,
            movers: vec![MoverObservation {
                id: MoverId(1),
                state: MoverState::Waiting { at: NodeId(2) },
                speed: 1.5,
                home_path: PathId(0),
            }],
        }
    }

    #[test]
    fn observation_hash_is_deterministic() {
        let a = observation_hash(&obs());
        let b = observation_hash(&obs());
        assert_eq!(a, b);
    }

    #[test]
    fn observation_hash_changes_with_state() {
        let mut o1 = obs();
        let mut o2 = obs();
        o2.tick = 8;
        assert_ne!(observation_hash(&o1), observation_hash(&o2));
        o2 = obs();
        o2.movers[0].speed = 1.6;
        assert_ne!(observation_hash(&o1), observation_hash(&o2));
        o1.movers.push(MoverObservation {
            id: MoverId(2),
            state: MoverState::Empty,
            speed: 1.0,
            home_path: PathId(1),
        });
        assert_ne!(observation_hash(&o1), observation_hash(&obs()));
    }

    #[test]
    fn append_writes_one_line_per_entry() {
        struct Counter {
            lines: usize,
            bytes: usize,
        }
        impl io::Write for Counter {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                self.bytes += b.len();
                if b == b"\n" {
                    self.lines += 1;
                }
                Ok(b.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let counter = Counter { lines: 0, bytes: 0 };
        let mut log = AgentLog::new(Box::new(counter));
        let entry = AgentLogEntry::new(&obs(), "a", Some(Action::NoOp), 1, "r".into(), None);
        assert!(log.append(&entry).is_none());
        assert!(log.append(&entry).is_none());
        assert!(!log.is_degraded());
    }

    struct AlwaysErr;
    impl io::Write for AlwaysErr {
        fn write(&mut self, _b: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("boom"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn write_failure_falls_back_to_ring_and_warns_once() {
        let mut log = AgentLog::with_capacity(Box::new(AlwaysErr), 8);
        let entry = AgentLogEntry::new(&obs(), "a", Some(Action::NoOp), 1, "r".into(), None);
        let first = log.append(&entry);
        assert!(matches!(first, Some(WarningPayload::AgentLogSlow)));
        assert!(log.is_degraded());
        // Subsequent failures don't re-warn.
        let second = log.append(&entry);
        assert!(second.is_none());
        assert_eq!(log.ring_snapshot().len(), 2);
    }

    #[test]
    fn ring_is_bounded_and_counts_drops() {
        let mut log = AgentLog::with_capacity(Box::new(AlwaysErr), 3);
        let entry = AgentLogEntry::new(&obs(), "a", Some(Action::NoOp), 1, "r".into(), None);
        for _ in 0..10 {
            let _ = log.append(&entry);
        }
        assert_eq!(log.ring_snapshot().len(), 3);
        assert_eq!(log.dropped(), 7);
    }

    #[test]
    fn force_degrade_skips_sink_immediately() {
        struct NeverCalled;
        impl io::Write for NeverCalled {
            fn write(&mut self, _b: &[u8]) -> io::Result<usize> {
                panic!("sink should not be called after force_degrade")
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut log = AgentLog::new(Box::new(NeverCalled));
        log.force_degrade();
        let entry = AgentLogEntry::new(&obs(), "a", None, 0, "".into(), None);
        assert!(log.append(&entry).is_none());
        assert_eq!(log.ring_snapshot().len(), 1);
    }

    #[test]
    fn entry_roundtrips_through_json() {
        let entry = AgentLogEntry::new(
            &obs(),
            "speed_tuner_0",
            Some(Action::SetSpeed {
                mover: 1,
                speed: 1.5,
            }),
            3,
            "nudge".into(),
            Some("raw llm text".into()),
        );
        let s = serde_json::to_string(&entry).unwrap();
        let back: AgentLogEntry = serde_json::from_str(&s).unwrap();
        assert_eq!(back, entry);
    }

    // ============================================================
    //  v2 schema tests (P2.A0.5)
    // ============================================================

    #[test]
    fn new_entry_has_current_schema_version() {
        let entry = AgentLogEntry::new(&obs(), "a", None, 0, String::new(), None);
        assert_eq!(entry.schema_version, SCHEMA_VERSION);
        assert_eq!(
            SCHEMA_VERSION, 2,
            "this test must update if SCHEMA_VERSION bumps"
        );
    }

    #[test]
    fn with_provenance_records_all_v2_fields() {
        let prov = LlmProvenance {
            backend: Some("copilot".into()),
            model: Some("gpt-5-mini".into()),
            latency_ms: Some(742),
            prompt_tokens: Some(1024),
            completion_tokens: Some(64),
        };
        let entry = AgentLogEntry::with_provenance(
            &obs(),
            "metro-pulse-llm",
            Some(Action::NoOp),
            2,
            "deliver next batch".into(),
            Some("{}".into()),
            prov.clone(),
        );
        assert_eq!(entry.schema_version, 2);
        assert_eq!(entry.backend, prov.backend);
        assert_eq!(entry.model, prov.model);
        assert_eq!(entry.latency_ms, prov.latency_ms);
        assert_eq!(entry.prompt_tokens, prov.prompt_tokens);
        assert_eq!(entry.completion_tokens, prov.completion_tokens);
    }

    #[test]
    fn v2_entry_roundtrips_through_json_preserving_all_fields() {
        let prov = LlmProvenance {
            backend: Some("copilot".into()),
            model: Some("gpt-5-mini".into()),
            latency_ms: Some(742),
            prompt_tokens: Some(1024),
            completion_tokens: Some(64),
        };
        let entry = AgentLogEntry::with_provenance(
            &obs(),
            "metro-pulse-llm",
            Some(Action::NoOp),
            2,
            "deliver next batch".into(),
            Some("RAW".into()),
            prov,
        );
        let s = serde_json::to_string(&entry).unwrap();
        // Schema version must be on the wire so v1/v2 are unambiguous.
        assert!(s.contains("\"schema_version\":2"));
        let back: AgentLogEntry = serde_json::from_str(&s).unwrap();
        assert_eq!(back, entry);
    }

    /// Critical migration shim test. A legacy v1 jsonl row (no
    /// schema_version field, no v2-only fields) MUST deserialize
    /// cleanly through the v2 struct and produce an entry whose
    /// schema_version is 1 (so replay tooling can tell). This is the
    /// P2.A0.5 acceptance criterion: replay works against both v1 and
    /// v2 fixtures bit-for-bit deterministic.
    #[test]
    fn v1_jsonl_row_deserializes_as_schema_version_1() {
        let v1_line = r#"{"tick":7,"agent_id":"speed_tuner_0","observation_hash":17277158419002680658,"raw_response":null,"parsed_action":{"kind":"no_op"},"considered_count":1,"rationale":"hold"}"#;
        let entry: AgentLogEntry = serde_json::from_str(v1_line).expect("v1 row must deserialize");
        assert_eq!(entry.schema_version, 1, "missing field must default to v1");
        assert_eq!(entry.tick, 7);
        assert_eq!(entry.agent_id, "speed_tuner_0");
        assert_eq!(entry.rationale, "hold");
        // v2-only fields default to None.
        assert_eq!(entry.backend, None);
        assert_eq!(entry.model, None);
        assert_eq!(entry.latency_ms, None);
        assert_eq!(entry.prompt_tokens, None);
        assert_eq!(entry.completion_tokens, None);
        assert_eq!(entry.truncated_bytes, None);
    }

    #[test]
    fn v1_minimal_row_without_raw_response_field_decodes() {
        // Even older v1 rows that omitted raw_response entirely.
        let v1_line = r#"{"tick":1,"agent_id":"a","observation_hash":0,"parsed_action":null,"considered_count":0,"rationale":""}"#;
        let entry: AgentLogEntry = serde_json::from_str(v1_line).expect("decode");
        assert_eq!(entry.schema_version, 1);
        assert_eq!(entry.raw_response, None);
    }

    // ---- raw_response cap (P2.A0.4 §5.6) ----------------------

    #[test]
    fn raw_response_capped_at_64kib_with_truncation_marker() {
        let huge = "a".repeat(RAW_RESPONSE_MAX_BYTES + 5_000);
        let entry = AgentLogEntry::new(&obs(), "a", None, 0, String::new(), Some(huge.clone()));
        let cap_len = entry.raw_response.as_ref().unwrap().len();
        assert!(
            cap_len <= RAW_RESPONSE_MAX_BYTES,
            "raw_response was {cap_len} bytes; cap is {RAW_RESPONSE_MAX_BYTES}"
        );
        assert_eq!(entry.truncated_bytes, Some(huge.len()));
    }

    #[test]
    fn raw_response_under_cap_unchanged_no_marker() {
        let small = "small response".to_string();
        let entry = AgentLogEntry::new(&obs(), "a", None, 0, String::new(), Some(small.clone()));
        assert_eq!(entry.raw_response, Some(small));
        assert_eq!(entry.truncated_bytes, None);
    }

    #[test]
    fn raw_response_truncation_preserves_utf8_boundary() {
        // Construct a string whose len > cap but where byte index
        // RAW_RESPONSE_MAX_BYTES falls inside a multi-byte character.
        let mut s = "a".repeat(RAW_RESPONSE_MAX_BYTES - 1);
        s.push('é'); // 2 bytes; index of last byte = MAX_BYTES
        s.push_str(&"b".repeat(100));
        let entry = AgentLogEntry::new(&obs(), "a", None, 0, String::new(), Some(s.clone()));
        let capped = entry.raw_response.as_ref().unwrap();
        // Must still be valid UTF-8 (which it is by Rust's type system).
        // Cap must be at-or-below MAX_BYTES.
        assert!(capped.len() <= RAW_RESPONSE_MAX_BYTES);
        // Truncated marker present.
        assert_eq!(entry.truncated_bytes, Some(s.len()));
    }

    // ---- schema validation (P2.A0.4 §5.4) ---------------------

    #[test]
    fn validate_entry_accepts_well_formed_row() {
        let entry = AgentLogEntry::new(&obs(), "a", None, 0, "ok".into(), None);
        assert_eq!(validate_entry(&entry), Ok(()));
    }

    #[test]
    fn validate_entry_rejects_empty_agent_id() {
        let mut entry = AgentLogEntry::new(&obs(), "a", None, 0, "".into(), None);
        entry.agent_id = String::new();
        assert_eq!(validate_entry(&entry), Err(SchemaError::EmptyAgentId));
    }

    #[test]
    fn validate_entry_rejects_oversize_agent_id() {
        let mut entry = AgentLogEntry::new(&obs(), "a", None, 0, "".into(), None);
        entry.agent_id = "x".repeat(AGENT_ID_MAX_LEN + 1);
        assert_eq!(
            validate_entry(&entry),
            Err(SchemaError::AgentIdTooLong(AGENT_ID_MAX_LEN + 1))
        );
    }

    #[test]
    fn validate_entry_rejects_oversize_rationale() {
        let mut entry = AgentLogEntry::new(&obs(), "a", None, 0, "".into(), None);
        entry.rationale = "x".repeat(RATIONALE_MAX_LEN + 1);
        assert_eq!(
            validate_entry(&entry),
            Err(SchemaError::RationaleTooLong(RATIONALE_MAX_LEN + 1))
        );
    }

    #[test]
    fn validate_entry_rejects_unsupported_version() {
        let mut entry = AgentLogEntry::new(&obs(), "a", None, 0, "".into(), None);
        entry.schema_version = 99;
        assert_eq!(
            validate_entry(&entry),
            Err(SchemaError::UnsupportedVersion(99))
        );
        entry.schema_version = 0;
        assert_eq!(
            validate_entry(&entry),
            Err(SchemaError::UnsupportedVersion(0))
        );
    }

    #[test]
    fn validate_entry_rejects_uncapped_raw_response_without_marker() {
        let mut entry = AgentLogEntry::new(&obs(), "a", None, 0, "".into(), None);
        entry.raw_response = Some("x".repeat(RAW_RESPONSE_MAX_BYTES + 1));
        entry.truncated_bytes = None;
        assert_eq!(
            validate_entry(&entry),
            Err(SchemaError::UncappedRawResponse)
        );
    }

    #[test]
    fn validate_entry_accepts_capped_raw_response_with_marker() {
        let huge = "x".repeat(RAW_RESPONSE_MAX_BYTES + 1);
        let entry = AgentLogEntry::new(&obs(), "a", None, 0, "".into(), Some(huge));
        // new() capped + marker was set.
        assert_eq!(validate_entry(&entry), Ok(()));
    }

    #[test]
    fn validate_entry_rejects_truncated_bytes_without_raw_response() {
        let mut entry = AgentLogEntry::new(&obs(), "a", None, 0, "".into(), None);
        entry.raw_response = None;
        entry.truncated_bytes = Some(100);
        assert_eq!(
            validate_entry(&entry),
            Err(SchemaError::TruncatedBytesWithoutResponse)
        );
    }

    #[test]
    fn validate_entry_rejects_truncated_bytes_on_small_response() {
        // raw_response is small (under cap) AND truncated_bytes is
        // set to a value that ALSO claims original ≤ MAX —
        // inconsistent because truncation should only fire when
        // original > MAX.
        let mut entry = AgentLogEntry::new(&obs(), "a", None, 0, "".into(), Some("short".into()));
        entry.truncated_bytes = Some(100); // claim "original was 100"; both are ≤ MAX
        assert_eq!(
            validate_entry(&entry),
            Err(SchemaError::TruncatedBytesOnSmallResponse(100, 5))
        );
    }

    #[test]
    fn validate_entry_rejects_truncated_bytes_not_greater_than_current() {
        // raw_response is OVER cap (which is itself a separate
        // invariant fault, but bear with us). truncated_bytes claims
        // original = MAX, but current length > MAX → the marker
        // claims the truncation produced MORE bytes than the
        // original, which is logically impossible.
        let mut entry = AgentLogEntry::new(&obs(), "a", None, 0, "".into(), Some("tiny".into()));
        // Bypass cap_raw_response by setting raw_response directly to
        // an over-cap value.
        entry.raw_response = Some("x".repeat(RAW_RESPONSE_MAX_BYTES + 100));
        // Marker claims original was just MAX + 1, but current is
        // MAX + 100 — impossible.
        entry.truncated_bytes = Some(RAW_RESPONSE_MAX_BYTES + 1);
        let actual_len = entry.raw_response.as_ref().unwrap().len();
        assert_eq!(
            validate_entry(&entry),
            Err(SchemaError::TruncatedBytesNotGreaterThanCurrent(
                RAW_RESPONSE_MAX_BYTES + 1,
                actual_len
            ))
        );
    }

    // ---- AgentLog.append() drops invalid rows (P2.A0.4 §5.4) ---

    #[test]
    fn append_drops_invalid_row_and_warns_once() {
        struct Counter {
            calls: usize,
        }
        impl io::Write for Counter {
            fn write(&mut self, b: &[u8]) -> io::Result<usize> {
                self.calls += 1;
                Ok(b.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut log = AgentLog::new(Box::new(Counter { calls: 0 }));
        let mut entry = AgentLogEntry::new(&obs(), "a", None, 0, "".into(), None);
        entry.schema_version = 99; // invalid
        let warn = log.append(&entry);
        assert!(matches!(warn, Some(WarningPayload::AgentLogSlow)));
        assert!(log.is_degraded());
        // Subsequent invalid rows don't re-warn.
        let warn2 = log.append(&entry);
        assert!(warn2.is_none());
        assert_eq!(log.dropped(), 2);
    }

    // ---- scene_id validation (P2.A0.4 §5.1) -------------------

    #[test]
    fn validate_scene_id_accepts_alphanumeric_underscore_dash() {
        assert_eq!(validate_scene_id("metro-pulse"), Ok(()));
        assert_eq!(validate_scene_id("demo_123"), Ok(()));
        assert_eq!(validate_scene_id("a"), Ok(()));
    }

    #[test]
    fn validate_scene_id_rejects_traversal() {
        assert!(matches!(
            validate_scene_id("../etc/passwd"),
            Err(SceneIdError::InvalidChar(_))
        ));
        assert!(matches!(
            validate_scene_id("metro/pulse"),
            Err(SceneIdError::InvalidChar(_))
        ));
        assert!(matches!(
            validate_scene_id("metro\\pulse"),
            Err(SceneIdError::InvalidChar(_))
        ));
        assert!(matches!(
            validate_scene_id(".."),
            Err(SceneIdError::InvalidChar(_))
        ));
        assert!(matches!(
            validate_scene_id("a.b"),
            Err(SceneIdError::InvalidChar(_))
        ));
    }

    #[test]
    fn validate_scene_id_rejects_empty_and_too_long() {
        assert_eq!(validate_scene_id(""), Err(SceneIdError::Empty));
        let long = "a".repeat(65);
        assert_eq!(validate_scene_id(&long), Err(SceneIdError::TooLong(65)));
    }

    #[test]
    fn agent_log_dir_uses_scene_id_after_validation() {
        let dir = agent_log_dir("metro-pulse").expect("valid");
        assert!(dir.ends_with("metro-pulse"));
        assert!(dir
            .parent()
            .map(|p| p.ends_with("simetro"))
            .unwrap_or(false));
    }

    #[test]
    fn agent_log_dir_rejects_traversal_scene_id() {
        assert!(agent_log_dir("../etc").is_err());
    }

    #[test]
    fn open_for_scene_rejects_traversal_with_invalid_input_io_error() {
        let err = AgentLog::open_for_scene("../etc").expect_err("traversal must fail");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    // ---- File mode 0o600 (P2.A0.4 §5.2) — Unix only -----------

    #[cfg(unix)]
    #[test]
    fn open_for_scene_creates_file_with_mode_0600() {
        // Use a tempdir via SIMETRO_DATA_DIR override.
        let tmp =
            std::env::temp_dir().join(format!("simetro-agentlog-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::env::set_var("SIMETRO_DATA_DIR", &tmp);

        let mut log = AgentLog::open_for_scene("metro-pulse").expect("open");
        let entry = AgentLogEntry::new(&obs(), "a", Some(Action::NoOp), 0, "ok".into(), None);
        let _ = log.append(&entry);
        log.flush().unwrap();
        drop(log);

        // Find the created file.
        let dir = tmp.join("simetro").join("metro-pulse");
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .collect();
        assert!(!entries.is_empty(), "expected at least one log file");
        for entry in entries {
            use std::os::unix::fs::PermissionsExt;
            let mode = entry.metadata().expect("stat").permissions().mode() & 0o777;
            assert_eq!(
                mode,
                0o600,
                "AgentLog file {:?} has mode {:o}, expected 0o600",
                entry.path(),
                mode
            );
        }

        let _ = std::fs::remove_dir_all(&tmp);
        std::env::remove_var("SIMETRO_DATA_DIR");
    }
}
