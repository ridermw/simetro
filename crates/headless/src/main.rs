//! `simetro-headless` binary.
//!
//! Subcommands per PLAN §19 step 13:
//!
//! ```text
//!   simetro-headless run            --scene PATH --ticks N --seed S
//!   simetro-headless bench          --scene PATH --ticks N --seed S
//!   simetro-headless hash           --scene PATH --ticks N --seed S
//!   simetro-headless replay         --log PATH [--format summary|json|protocol-jsonl]
//!   simetro-headless export-session --scene PATH --ticks N --seed S --out DIR
//! ```
//!
//! Exits non-zero on load failures (LoadError surfaces as a printed
//! diagnostic + exit code 2). Other failure modes (IO on session
//! export) exit code 3. Process never panics — every error path
//! flows through `Result` and `std::process::exit`.

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use simetro_engine::{hash_run, load_scene_str, TickRunner};
use simetro_protocol::{
    Action, ActionTag, AgentReport, ConsideredAction, Envelope, SimEvent, SimMessage,
};

const DEFAULT_TICKS: u64 = 10_000;
const DEFAULT_SEED: u64 = 42;

#[derive(Parser)]
#[command(name = "simetro-headless", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a scene for N ticks (prints summary).
    Run {
        #[arg(long)]
        scene: PathBuf,
        #[arg(long, default_value_t = DEFAULT_TICKS)]
        ticks: u64,
        #[arg(long, default_value_t = DEFAULT_SEED)]
        seed: u64,
    },
    /// Benchmark tick throughput; reports tps.
    Bench {
        #[arg(long)]
        scene: PathBuf,
        #[arg(long, default_value_t = DEFAULT_TICKS)]
        ticks: u64,
        #[arg(long, default_value_t = DEFAULT_SEED)]
        seed: u64,
    },
    /// Emit a deterministic SHA-256 of (world_state, event_stream).
    /// Use this to author or refresh `tests/baselines/<scene>.hash`.
    Hash {
        #[arg(long)]
        scene: PathBuf,
        #[arg(long, default_value_t = DEFAULT_TICKS)]
        ticks: u64,
        #[arg(long, default_value_t = DEFAULT_SEED)]
        seed: u64,
    },
    /// Replay an AgentLog decision stream without invoking live agents.
    Replay {
        #[arg(long)]
        log: PathBuf,
        /// Start replay at this tick (inclusive).
        #[arg(long)]
        from_tick: Option<u64>,
        /// Stop replay at this tick (inclusive).
        #[arg(long)]
        to_tick: Option<u64>,
        /// Restrict replay to one agent id.
        #[arg(long)]
        agent_id: Option<String>,
        /// Output mode: human summary, JSON report, or protocol JSONL for UI consumers.
        #[arg(long, value_enum, default_value_t = ReplayFormat::Summary)]
        format: ReplayFormat,
    },
    /// Export a session bundle (scene + AgentLog + tracing + hash).
    ExportSession {
        #[arg(long)]
        scene: PathBuf,
        #[arg(long, default_value_t = DEFAULT_TICKS)]
        ticks: u64,
        #[arg(long, default_value_t = DEFAULT_SEED)]
        seed: u64,
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    let code = match cli.cmd {
        Cmd::Run { scene, ticks, seed } => cmd_run(&scene, ticks, seed),
        Cmd::Bench { scene, ticks, seed } => cmd_bench(&scene, ticks, seed),
        Cmd::Hash { scene, ticks, seed } => cmd_hash(&scene, ticks, seed),
        Cmd::Replay {
            log,
            from_tick,
            to_tick,
            agent_id,
            format,
        } => cmd_replay(
            &log,
            ReplayOptions {
                from_tick,
                to_tick,
                agent_id,
                format,
            },
        ),
        Cmd::ExportSession {
            scene,
            ticks,
            seed,
            out,
        } => cmd_export_session(&scene, ticks, seed, &out),
    };
    std::process::exit(code);
}

fn read_scene(path: &std::path::Path) -> Result<String, i32> {
    std::fs::read_to_string(path).map_err(|e| {
        eprintln!("error: failed to read {}: {e}", path.display());
        3
    })
}

fn load(path: &std::path::Path, seed: u64) -> Result<simetro_engine::LoadedScene, i32> {
    let src = read_scene(path)?;
    load_scene_str(&src, seed).map_err(|e| {
        eprintln!("LoadError: {e}");
        2
    })
}

fn cmd_run(scene: &std::path::Path, ticks: u64, seed: u64) -> i32 {
    let mut loaded = match load(scene, seed) {
        Ok(v) => v,
        Err(c) => return c,
    };
    let mut runner = TickRunner::new();
    runner.reserve_for(loaded.world.movers.len());
    let mut events_total: u64 = 0;
    let started = Instant::now();
    for _ in 0..ticks {
        runner.tick_once(&mut loaded.world);
        events_total += runner.events().len() as u64;
    }
    let elapsed = started.elapsed();
    let tps = ticks as f64 / elapsed.as_secs_f64().max(1e-9);
    println!(
        "run: ticks={ticks} seed={seed} elapsed={:.3}s tps={tps:.0} events={events_total}",
        elapsed.as_secs_f64()
    );
    0
}

fn cmd_bench(scene: &std::path::Path, ticks: u64, seed: u64) -> i32 {
    // bench differs from run by minimizing per-tick overhead: no event
    // counter, no log, only tps.
    let mut loaded = match load(scene, seed) {
        Ok(v) => v,
        Err(c) => return c,
    };
    let mut runner = TickRunner::new();
    runner.reserve_for(loaded.world.movers.len());
    // Warmup (200 ticks) so we measure steady-state.
    let warmup = ticks.min(200);
    for _ in 0..warmup {
        runner.tick_once(&mut loaded.world);
    }
    let started = Instant::now();
    for _ in 0..ticks {
        runner.tick_once(&mut loaded.world);
    }
    let elapsed = started.elapsed();
    let tps = ticks as f64 / elapsed.as_secs_f64().max(1e-9);
    println!(
        "bench: ticks={ticks} seed={seed} warmup={warmup} elapsed={:.3}s tps={tps:.0}",
        elapsed.as_secs_f64()
    );
    0
}

fn cmd_hash(scene: &std::path::Path, ticks: u64, seed: u64) -> i32 {
    let mut loaded = match load(scene, seed) {
        Ok(v) => v,
        Err(c) => return c,
    };
    let mut runner = TickRunner::new();
    runner.reserve_for(loaded.world.movers.len());
    let hex = hash_run(&mut loaded.world, &mut runner, ticks);
    println!("{hex}");
    0
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
enum ReplayFormat {
    Summary,
    Json,
    ProtocolJsonl,
}

#[derive(Debug)]
struct ReplayOptions {
    from_tick: Option<u64>,
    to_tick: Option<u64>,
    agent_id: Option<String>,
    format: ReplayFormat,
}

#[derive(Debug)]
struct NumberedEntry {
    line: usize,
    entry: simetro_engine::AgentLogEntry,
}

#[derive(Debug, Serialize)]
struct ReplayReport {
    summary: ReplaySummary,
    issues: Vec<ReplayIssue>,
    entries: Vec<ReplayRecord>,
}

#[derive(Debug, Serialize)]
struct ReplaySummary {
    total_lines: u64,
    entries: u64,
    selected: u64,
    malformed: u64,
    first_tick: Option<u64>,
    last_tick: Option<u64>,
    selected_first_tick: Option<u64>,
    selected_last_tick: Option<u64>,
    agents: Vec<String>,
    actions: std::collections::BTreeMap<String, u64>,
    duplicate_decisions: u64,
    out_of_order: u64,
    filters: ReplayFilters,
}

#[derive(Debug, Serialize)]
struct ReplayFilters {
    from_tick: Option<u64>,
    to_tick: Option<u64>,
    agent_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct ReplayIssue {
    kind: &'static str,
    line: usize,
    tick: Option<u64>,
    agent_id: Option<String>,
    message: String,
}

#[derive(Debug, Serialize)]
struct ReplayRecord {
    replay_index: u64,
    line: usize,
    tick: u64,
    agent_id: String,
    observation_hash: u64,
    action: &'static str,
    parsed_action: Option<Action>,
    considered_count: usize,
    rationale: String,
    has_raw_response: bool,
}

fn cmd_replay(log: &std::path::Path, options: ReplayOptions) -> i32 {
    if let (Some(from), Some(to)) = (options.from_tick, options.to_tick) {
        if from > to {
            eprintln!("error: --from-tick ({from}) must be <= --to-tick ({to})");
            return 2;
        }
    }

    let src = match std::fs::read_to_string(log) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to read {}: {e}", log.display());
            return 3;
        }
    };

    let parsed = parse_agent_log(&src);
    let selected = select_entries(&parsed.entries, &options);
    let report = build_replay_report(&src, &parsed, &selected, &options);

    if options.format == ReplayFormat::ProtocolJsonl {
        if report.summary.malformed > 0 {
            eprintln!(
                "error: AgentLog contains {} malformed line(s); refusing protocol replay",
                report.summary.malformed
            );
            print_issues(&report.issues);
            return 2;
        }
        return match emit_protocol_jsonl(&selected) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("error: serializing protocol replay: {e}");
                3
            }
        };
    }

    match options.format {
        ReplayFormat::Summary => print_replay_summary(&report),
        ReplayFormat::Json => match serde_json::to_string_pretty(&report) {
            Ok(json) => println!("{json}"),
            Err(e) => {
                eprintln!("error: serializing replay report: {e}");
                return 3;
            }
        },
        ReplayFormat::ProtocolJsonl => {}
    }

    if report.summary.malformed > 0 {
        print_issues(&report.issues);
        2
    } else {
        0
    }
}

struct ParsedLog {
    entries: Vec<NumberedEntry>,
    issues: Vec<ReplayIssue>,
}

fn parse_agent_log(src: &str) -> ParsedLog {
    let mut entries = Vec::new();
    let mut issues = Vec::new();
    for (idx, line) in src.lines().enumerate() {
        let line_no = idx.saturating_add(1);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<simetro_engine::AgentLogEntry>(trimmed) {
            Ok(entry) => entries.push(NumberedEntry {
                line: line_no,
                entry,
            }),
            Err(e) => issues.push(ReplayIssue {
                kind: "malformed_json",
                line: line_no,
                tick: None,
                agent_id: None,
                message: e.to_string(),
            }),
        }
    }
    ParsedLog { entries, issues }
}

fn select_entries<'a>(
    entries: &'a [NumberedEntry],
    options: &ReplayOptions,
) -> Vec<&'a NumberedEntry> {
    entries
        .iter()
        .filter(|line| {
            options
                .from_tick
                .map_or(true, |from| line.entry.tick >= from)
                && options.to_tick.map_or(true, |to| line.entry.tick <= to)
                && options
                    .agent_id
                    .as_ref()
                    .map_or(true, |agent| line.entry.agent_id == *agent)
        })
        .collect()
}

fn build_replay_report(
    src: &str,
    parsed: &ParsedLog,
    selected: &[&NumberedEntry],
    options: &ReplayOptions,
) -> ReplayReport {
    let mut issues = parsed.issues.clone();
    issues.extend(correlate_entries(&parsed.entries));

    let mut agents = std::collections::BTreeSet::new();
    let mut actions = std::collections::BTreeMap::new();
    let mut first_tick = None;
    let mut last_tick = None;
    for line in &parsed.entries {
        agents.insert(line.entry.agent_id.clone());
        let action = action_tag_name(entry_action_tag(&line.entry)).to_string();
        *actions.entry(action).or_insert(0) += 1;
        first_tick = Some(first_tick.map_or(line.entry.tick, |cur: u64| cur.min(line.entry.tick)));
        last_tick = Some(last_tick.map_or(line.entry.tick, |cur: u64| cur.max(line.entry.tick)));
    }

    let mut selected_first_tick = None;
    let mut selected_last_tick = None;
    let mut records = Vec::with_capacity(selected.len());
    for (idx, line) in selected.iter().enumerate() {
        selected_first_tick =
            Some(selected_first_tick.map_or(line.entry.tick, |cur: u64| cur.min(line.entry.tick)));
        selected_last_tick =
            Some(selected_last_tick.map_or(line.entry.tick, |cur: u64| cur.max(line.entry.tick)));
        records.push(ReplayRecord {
            replay_index: idx as u64,
            line: line.line,
            tick: line.entry.tick,
            agent_id: line.entry.agent_id.clone(),
            observation_hash: line.entry.observation_hash,
            action: action_tag_name(entry_action_tag(&line.entry)),
            parsed_action: line.entry.parsed_action.clone(),
            considered_count: line.entry.considered_count,
            rationale: line.entry.rationale.clone(),
            has_raw_response: line.entry.raw_response.is_some(),
        });
    }

    let duplicate_decisions = issues
        .iter()
        .filter(|issue| issue.kind == "duplicate_decision")
        .count() as u64;
    let out_of_order = issues
        .iter()
        .filter(|issue| issue.kind == "out_of_order")
        .count() as u64;

    ReplayReport {
        summary: ReplaySummary {
            total_lines: src.lines().count() as u64,
            entries: parsed.entries.len() as u64,
            selected: selected.len() as u64,
            malformed: parsed.issues.len() as u64,
            first_tick,
            last_tick,
            selected_first_tick,
            selected_last_tick,
            agents: agents.into_iter().collect(),
            actions,
            duplicate_decisions,
            out_of_order,
            filters: ReplayFilters {
                from_tick: options.from_tick,
                to_tick: options.to_tick,
                agent_id: options.agent_id.clone(),
            },
        },
        issues,
        entries: records,
    }
}

fn correlate_entries(entries: &[NumberedEntry]) -> Vec<ReplayIssue> {
    let mut issues = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut prev_tick = None;
    for line in entries {
        if let Some(prev) = prev_tick {
            if line.entry.tick < prev {
                issues.push(ReplayIssue {
                    kind: "out_of_order",
                    line: line.line,
                    tick: Some(line.entry.tick),
                    agent_id: Some(line.entry.agent_id.clone()),
                    message: format!("tick {} appears after tick {prev}", line.entry.tick),
                });
            }
        }
        prev_tick = Some(line.entry.tick);

        let key = (line.entry.tick, line.entry.agent_id.clone());
        if !seen.insert(key) {
            issues.push(ReplayIssue {
                kind: "duplicate_decision",
                line: line.line,
                tick: Some(line.entry.tick),
                agent_id: Some(line.entry.agent_id.clone()),
                message: "duplicate decision for tick+agent_id".to_string(),
            });
        }
    }
    issues
}

fn print_replay_summary(report: &ReplayReport) {
    let agents = if report.summary.agents.is_empty() {
        "-".to_string()
    } else {
        report.summary.agents.join(",")
    };
    let actions = if report.summary.actions.is_empty() {
        "-".to_string()
    } else {
        report
            .summary
            .actions
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    println!(
        "replay: entries={} selected={} malformed={} ticks={:?}..{:?} selected_ticks={:?}..{:?} agents={} actions={} duplicate_decisions={} out_of_order={}",
        report.summary.entries,
        report.summary.selected,
        report.summary.malformed,
        report.summary.first_tick,
        report.summary.last_tick,
        report.summary.selected_first_tick,
        report.summary.selected_last_tick,
        agents,
        actions,
        report.summary.duplicate_decisions,
        report.summary.out_of_order
    );
}

fn print_issues(issues: &[ReplayIssue]) {
    for issue in issues.iter().filter(|issue| issue.kind == "malformed_json") {
        eprintln!("line {}: {}", issue.line, issue.message);
    }
}

fn emit_protocol_jsonl(entries: &[&NumberedEntry]) -> Result<(), serde_json::Error> {
    for env in protocol_envelopes(entries) {
        println!("{}", serde_json::to_string(&env)?);
    }
    Ok(())
}

fn protocol_envelopes(entries: &[&NumberedEntry]) -> Vec<Envelope<SimMessage>> {
    let mut seq = 0u64;
    let mut out = Vec::with_capacity(entries.len().saturating_mul(2));
    for line in entries {
        let action = entry_action_tag(&line.entry);
        out.push(Envelope::new(
            seq,
            SimMessage::Events(vec![SimEvent::AgentDecided {
                agent_id: line.entry.agent_id.clone(),
                action,
            }]),
        ));
        seq = seq.saturating_add(1);
        out.push(Envelope::new(
            seq,
            SimMessage::AgentReport(report_from_entry(&line.entry)),
        ));
        seq = seq.saturating_add(1);
    }
    out
}

fn report_from_entry(entry: &simetro_engine::AgentLogEntry) -> AgentReport {
    let considered = entry
        .parsed_action
        .clone()
        .map(|action| {
            vec![ConsideredAction {
                action,
                confidence: 1.0,
            }]
        })
        .unwrap_or_default();
    AgentReport {
        tick: entry.tick,
        agent_id: entry.agent_id.clone(),
        considered,
        chosen: entry.parsed_action.clone(),
        rationale: entry.rationale.clone(),
        confidence: if entry.parsed_action.is_some() {
            1.0
        } else {
            0.0
        },
    }
}

fn entry_action_tag(entry: &simetro_engine::AgentLogEntry) -> ActionTag {
    entry
        .parsed_action
        .as_ref()
        .map_or(ActionTag::NoOp, Action::tag)
}

fn action_tag_name(tag: ActionTag) -> &'static str {
    match tag {
        ActionTag::NoOp => "no_op",
        ActionTag::SetSpeed => "set_speed",
        ActionTag::PlacePiece => "place_piece",
        ActionTag::ConnectPieces => "connect_pieces",
        ActionTag::RemovePiece => "remove_piece",
        ActionTag::DefineResource => "define_resource",
        ActionTag::AddProducer => "add_producer",
        ActionTag::AddConsumer => "add_consumer",
        ActionTag::SetGoal => "set_goal",
    }
}

fn cmd_export_session(
    scene: &std::path::Path,
    ticks: u64,
    seed: u64,
    out: &std::path::Path,
) -> i32 {
    // Layout per PLAN §15:
    //  out/
    //    scene.json
    //    baseline.hash
    //    manifest.json
    //    agent-log.jsonl  (empty in P1; populated when an agent is registered)
    //    tracing.jsonl    (empty in P1; populated when tracing subscriber is wired)
    if let Err(e) = std::fs::create_dir_all(out) {
        eprintln!("error: failed to create {}: {e}", out.display());
        return 3;
    }
    let src = match read_scene(scene) {
        Ok(s) => s,
        Err(c) => return c,
    };
    if let Err(e) = std::fs::write(out.join("scene.json"), &src) {
        eprintln!("error: copying scene: {e}");
        return 3;
    }
    let mut loaded = match load_scene_str(&src, seed) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("LoadError: {e}");
            return 2;
        }
    };
    let mut runner = TickRunner::new();
    runner.reserve_for(loaded.world.movers.len());
    let hex = hash_run(&mut loaded.world, &mut runner, ticks);
    if let Err(e) = std::fs::write(out.join("baseline.hash"), format!("{hex}\n")) {
        eprintln!("error: writing baseline: {e}");
        return 3;
    }
    // Placeholder empty log + tracing files so the bundle layout is
    // stable across builds.
    if let Err(e) = std::fs::File::create(out.join("agent-log.jsonl")) {
        eprintln!("error: creating agent-log.jsonl: {e}");
        return 3;
    }
    if let Err(e) = std::fs::File::create(out.join("tracing.jsonl")) {
        eprintln!("error: creating tracing.jsonl: {e}");
        return 3;
    }
    let manifest = serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "build": option_env!("VERGEN_GIT_SHA").unwrap_or("unknown"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "seed": seed,
        "ticks": ticks,
        "scene": scene.file_name().and_then(|s| s.to_str()).unwrap_or(""),
    });
    let manifest_str = match serde_json::to_string_pretty(&manifest) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: serializing manifest: {e}");
            return 3;
        }
    };
    let mut f = match std::fs::File::create(out.join("manifest.json")) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: creating manifest.json: {e}");
            return 3;
        }
    };
    if let Err(e) = f.write_all(manifest_str.as_bytes()) {
        eprintln!("error: writing manifest.json: {e}");
        return 3;
    }
    println!("export-session: {} (hash={hex})", out.display());
    0
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn entry_json(tick: u64, agent_id: &str, action: &str) -> String {
        format!(
            r#"{{"tick":{tick},"agent_id":"{agent_id}","observation_hash":{},"raw_response":null,"parsed_action":{action},"considered_count":1,"rationale":"r{tick}"}}"#,
            tick + 100
        )
    }

    fn default_options() -> ReplayOptions {
        ReplayOptions {
            from_tick: None,
            to_tick: None,
            agent_id: None,
            format: ReplayFormat::Summary,
        }
    }

    #[test]
    fn replay_parser_counts_valid_and_malformed_lines() {
        let src = format!(
            "{}\nnot-json\n\n{}\n",
            entry_json(
                1,
                "speed_tuner_0",
                r#"{"kind":"set_speed","mover":7,"speed":1.5}"#
            ),
            entry_json(2, "speed_tuner_0", r#"{"kind":"no_op"}"#)
        );

        let parsed = parse_agent_log(&src);
        let selected = select_entries(&parsed.entries, &default_options());
        let report = build_replay_report(&src, &parsed, &selected, &default_options());

        assert_eq!(report.summary.total_lines, 4);
        assert_eq!(report.summary.entries, 2);
        assert_eq!(report.summary.selected, 2);
        assert_eq!(report.summary.malformed, 1);
        assert_eq!(report.summary.first_tick, Some(1));
        assert_eq!(report.summary.last_tick, Some(2));
        assert_eq!(report.summary.actions.get("set_speed"), Some(&1));
        assert_eq!(report.summary.actions.get("no_op"), Some(&1));
    }

    #[test]
    fn replay_filters_by_tick_range_and_agent() {
        let src = format!(
            "{}\n{}\n{}\n",
            entry_json(10, "a", r#"{"kind":"no_op"}"#),
            entry_json(11, "b", r#"{"kind":"no_op"}"#),
            entry_json(12, "a", r#"{"kind":"no_op"}"#)
        );
        let options = ReplayOptions {
            from_tick: Some(11),
            to_tick: Some(12),
            agent_id: Some("a".to_string()),
            format: ReplayFormat::Summary,
        };
        let parsed = parse_agent_log(&src);
        let selected = select_entries(&parsed.entries, &options);
        let report = build_replay_report(&src, &parsed, &selected, &options);

        assert_eq!(report.summary.selected, 1);
        assert_eq!(report.entries[0].tick, 12);
        assert_eq!(report.entries[0].agent_id, "a");
    }

    #[test]
    fn replay_correlation_flags_duplicate_and_out_of_order_decisions() {
        let src = format!(
            "{}\n{}\n{}\n",
            entry_json(10, "a", r#"{"kind":"no_op"}"#),
            entry_json(10, "a", r#"{"kind":"no_op"}"#),
            entry_json(9, "b", r#"{"kind":"no_op"}"#)
        );
        let parsed = parse_agent_log(&src);
        let selected = select_entries(&parsed.entries, &default_options());
        let report = build_replay_report(&src, &parsed, &selected, &default_options());

        assert_eq!(report.summary.duplicate_decisions, 1);
        assert_eq!(report.summary.out_of_order, 1);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.kind == "duplicate_decision" && issue.line == 2));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.kind == "out_of_order" && issue.line == 3));
    }

    #[test]
    fn protocol_replay_emits_event_then_agent_report_per_entry() {
        let src = entry_json(
            5,
            "speed_tuner_0",
            r#"{"kind":"set_speed","mover":2,"speed":1.25}"#,
        );
        let parsed = parse_agent_log(&src);
        let selected = select_entries(&parsed.entries, &default_options());
        let envs = protocol_envelopes(&selected);

        assert_eq!(envs.len(), 2);
        assert_eq!(envs[0].seq, 0);
        match &envs[0].payload {
            SimMessage::Events(events) => {
                assert_eq!(
                    events,
                    &vec![SimEvent::AgentDecided {
                        agent_id: "speed_tuner_0".to_string(),
                        action: ActionTag::SetSpeed,
                    }]
                );
            }
            other => panic!("expected events, got {other:?}"),
        }
        assert_eq!(envs[1].seq, 1);
        match &envs[1].payload {
            SimMessage::AgentReport(report) => {
                assert_eq!(report.tick, 5);
                assert_eq!(report.agent_id, "speed_tuner_0");
                assert_eq!(report.confidence, 1.0);
                assert_eq!(report.considered.len(), 1);
            }
            other => panic!("expected agent report, got {other:?}"),
        }
    }
}
