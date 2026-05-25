//! Integration tests for `simetro-headless policy-search` (PR 13).
//!
//! These cover the spec-mandated end-to-end behavior:
//!
//! * **Repeatability:** same `(scene, policy, seed, ticks)` produces
//!   identical hash, identical score, identical JSONL.
//! * **keep / discard:** candidate that strictly beats baseline →
//!   `status: "kept"`. Identical-to-baseline candidate → `discarded`.
//! * **blocked:** policy with an unknown agent id → `status: "blocked"`,
//!   process exits 2.
//! * **JSONL shape:** every row has `type`, the trial row has a
//!   `hash`, and the trailing row is `type: "summary"`.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_simetro-headless")
}

fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

fn gpu_launch_week_scene() -> PathBuf {
    let mut p = repo_root();
    p.push("games");
    p.push("gpu-launch-week.json");
    p
}

fn baseline_policy() -> PathBuf {
    let mut p = repo_root();
    p.push("policies");
    p.push("gpu-launch-week-baseline.json");
    p
}

fn aggressive_policy() -> PathBuf {
    let mut p = repo_root();
    p.push("policies");
    p.push("gpu-launch-week-throttler-aggressive.json");
    p
}

fn tmp_path(stem: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    let pid = std::process::id();
    let nonce: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    p.push(format!("simetro-policy-search-{stem}-{pid}-{nonce}.jsonl"));
    p
}

fn read_lines(path: &PathBuf) -> Vec<serde_json::Value> {
    let text = fs::read_to_string(path).expect("read jsonl");
    text.lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str(l).expect("parse jsonl line"))
        .collect()
}

#[test]
fn policy_search_baseline_and_aggressive_emit_jsonl_with_summary() {
    let out = tmp_path("smoke");
    let status = Command::new(bin())
        .args([
            "policy-search",
            "--scene",
            gpu_launch_week_scene().to_str().unwrap(),
            "--baseline-policy",
            baseline_policy().to_str().unwrap(),
            "--candidate-policy",
            aggressive_policy().to_str().unwrap(),
            "--ticks",
            "200",
            "--seed",
            "42",
            "--out",
            out.to_str().unwrap(),
        ])
        .status()
        .expect("run policy-search");
    assert!(status.success(), "expected exit 0, got {status:?}");

    let rows = read_lines(&out);
    assert_eq!(
        rows.len(),
        3,
        "expected baseline + candidate + summary, got {rows:?}"
    );
    assert_eq!(rows[0]["type"], "trial");
    assert_eq!(rows[0]["status"], "baseline");
    assert!(rows[0]["hash"].is_string(), "baseline must have hash");
    assert!(rows[0]["score"].is_object(), "baseline must have score");

    assert_eq!(rows[1]["type"], "trial");
    assert!(["kept", "discarded"].contains(&rows[1]["status"].as_str().unwrap()));
    assert!(rows[1]["hash"].is_string());
    assert!(rows[1]["score"].is_object());
    assert!(rows[1]["baseline_score"].is_object());

    assert_eq!(rows[2]["type"], "summary");
    assert_eq!(rows[2]["total_trials"], 2);
    assert_eq!(rows[2]["baseline_count"], 1);

    let _ = fs::remove_file(&out);
}

#[test]
fn policy_search_is_deterministic_across_two_invocations() {
    let out1 = tmp_path("det1");
    let out2 = tmp_path("det2");
    for out in [&out1, &out2] {
        let status = Command::new(bin())
            .args([
                "policy-search",
                "--scene",
                gpu_launch_week_scene().to_str().unwrap(),
                "--baseline-policy",
                baseline_policy().to_str().unwrap(),
                "--candidate-policy",
                aggressive_policy().to_str().unwrap(),
                "--ticks",
                "200",
                "--seed",
                "42",
                "--out",
                out.to_str().unwrap(),
            ])
            .status()
            .expect("run policy-search");
        assert!(status.success());
    }
    let r1 = read_lines(&out1);
    let r2 = read_lines(&out2);
    assert_eq!(r1.len(), r2.len());
    // Hash + score must match line-by-line. (We don't compare the
    // `scene` field of summary as a path, just hashes/scores/status.)
    for (a, b) in r1.iter().zip(r2.iter()) {
        assert_eq!(a["type"], b["type"]);
        assert_eq!(a["status"], b["status"]);
        assert_eq!(a["hash"], b["hash"], "hash drift: a={a} b={b}");
        assert_eq!(a["score"], b["score"], "score drift: a={a} b={b}");
        assert_eq!(a["outcome"], b["outcome"]);
    }
    let _ = fs::remove_file(&out1);
    let _ = fs::remove_file(&out2);
}

#[test]
fn baseline_against_itself_is_discarded_not_kept() {
    // Running the same artifact as both baseline and candidate must
    // produce identical scores, so the candidate is `discarded` (not
    // strictly better than baseline). This is the "policy-search
    // repeatability" claim from the SL1 spec.
    let out = tmp_path("self");
    let status = Command::new(bin())
        .args([
            "policy-search",
            "--scene",
            gpu_launch_week_scene().to_str().unwrap(),
            "--baseline-policy",
            baseline_policy().to_str().unwrap(),
            "--candidate-policy",
            baseline_policy().to_str().unwrap(),
            "--ticks",
            "200",
            "--seed",
            "42",
            "--out",
            out.to_str().unwrap(),
        ])
        .status()
        .expect("run policy-search");
    assert!(status.success());
    let rows = read_lines(&out);
    assert_eq!(
        rows[0]["hash"], rows[1]["hash"],
        "self-vs-self must hash equal"
    );
    assert_eq!(
        rows[0]["score"], rows[1]["score"],
        "self-vs-self must score equal"
    );
    assert_eq!(rows[1]["status"], "discarded");
    let _ = fs::remove_file(&out);
}

#[test]
fn policy_with_unknown_agent_is_blocked_and_exit_is_2() {
    // Write a temp policy that references an agent id that does not
    // exist in gpu-launch-week.json. Expect `status: "blocked"` and
    // process exit code 2.
    let bad_policy_path = std::env::temp_dir().join(format!(
        "simetro-policy-bad-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    ));
    fs::write(
        &bad_policy_path,
        r#"{"name":"bad","overrides":{"agents":{"ghost-agent":{"interval_ticks":10}}}}"#,
    )
    .expect("write bad policy");
    let out = tmp_path("blocked");
    let status = Command::new(bin())
        .args([
            "policy-search",
            "--scene",
            gpu_launch_week_scene().to_str().unwrap(),
            "--baseline-policy",
            baseline_policy().to_str().unwrap(),
            "--candidate-policy",
            bad_policy_path.to_str().unwrap(),
            "--ticks",
            "200",
            "--seed",
            "42",
            "--out",
            out.to_str().unwrap(),
        ])
        .status()
        .expect("run policy-search");
    assert_eq!(status.code(), Some(2), "expected exit 2 for blocked trial");
    let rows = read_lines(&out);
    assert_eq!(rows[1]["status"], "blocked");
    assert!(rows[1]["error"]
        .as_str()
        .unwrap_or("")
        .contains("ghost-agent"));
    let _ = fs::remove_file(&bad_policy_path);
    let _ = fs::remove_file(&out);
}

#[test]
fn policy_with_invalid_override_key_blocks_at_load_time() {
    // Top-level policy parse failure (artifact JSON has typo) → process
    // exits 2 BEFORE any trial JSONL is written. This covers the
    // load-time gate distinct from the at-apply-time gate above.
    let bad_policy_path = std::env::temp_dir().join(format!(
        "simetro-policy-typo-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    ));
    fs::write(
        &bad_policy_path,
        r#"{"name":"typo","overrides":{"agents":{}},"unexpected":1}"#,
    )
    .expect("write bad policy");
    let out = tmp_path("typo");
    let status = Command::new(bin())
        .args([
            "policy-search",
            "--scene",
            gpu_launch_week_scene().to_str().unwrap(),
            "--candidate-policy",
            bad_policy_path.to_str().unwrap(),
            "--ticks",
            "200",
            "--seed",
            "42",
            "--out",
            out.to_str().unwrap(),
        ])
        .status()
        .expect("run policy-search");
    assert_eq!(status.code(), Some(2), "expected exit 2 for parse error");
    let _ = fs::remove_file(&bad_policy_path);
    let _ = fs::remove_file(&out);
}
