//! End-to-end smoke tests for the `simetro-headless` binary.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_simetro-headless")
}

fn scene_path() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("games");
    p.push("demo-paths.json");
    p
}

#[test]
fn hash_is_deterministic_across_two_invocations() {
    let scene = scene_path();
    let scene_str = scene.to_str().expect("utf8 path");
    let out1 = Command::new(bin())
        .args([
            "hash", "--scene", scene_str, "--ticks", "200", "--seed", "42",
        ])
        .output()
        .expect("run hash");
    assert!(out1.status.success(), "first hash failed: {out1:?}");
    let h1 = String::from_utf8_lossy(&out1.stdout).trim().to_string();
    let out2 = Command::new(bin())
        .args([
            "hash", "--scene", scene_str, "--ticks", "200", "--seed", "42",
        ])
        .output()
        .expect("run hash");
    assert!(out2.status.success());
    let h2 = String::from_utf8_lossy(&out2.stdout).trim().to_string();
    assert_eq!(h1, h2, "determinism violated");
    assert_eq!(h1.len(), 64);
}

#[test]
fn bench_exits_zero_and_prints_tps() {
    let scene = scene_path();
    let out = Command::new(bin())
        .args([
            "bench",
            "--scene",
            scene.to_str().expect("utf8 path"),
            "--ticks",
            "500",
            "--seed",
            "42",
        ])
        .output()
        .expect("run bench");
    assert!(out.status.success(), "bench failed: {out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("tps="), "missing tps in output: {stdout}");
}

#[test]
fn run_exits_zero() {
    let scene = scene_path();
    let out = Command::new(bin())
        .args([
            "run",
            "--scene",
            scene.to_str().expect("utf8 path"),
            "--ticks",
            "100",
            "--seed",
            "42",
        ])
        .output()
        .expect("run");
    assert!(out.status.success(), "run failed: {out:?}");
}

#[test]
fn export_session_writes_expected_layout() {
    let scene = scene_path();
    let tmp = std::env::temp_dir().join(format!("simetro-headless-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let out = Command::new(bin())
        .args([
            "export-session",
            "--scene",
            scene.to_str().expect("utf8 path"),
            "--ticks",
            "100",
            "--seed",
            "42",
            "--out",
            tmp.to_str().expect("utf8 path"),
        ])
        .output()
        .expect("run export-session");
    assert!(out.status.success(), "export failed: {out:?}");
    for name in [
        "scene.json",
        "baseline.hash",
        "manifest.json",
        "agent-log.jsonl",
        "tracing.jsonl",
    ] {
        assert!(tmp.join(name).exists(), "expected {name} in bundle");
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn unknown_scene_exits_nonzero() {
    let out = Command::new(bin())
        .args([
            "run",
            "--scene",
            "/nonexistent/does-not-exist.json",
            "--ticks",
            "10",
        ])
        .output()
        .expect("run");
    assert!(!out.status.success());
}
