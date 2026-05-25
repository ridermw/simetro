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
fn export_session_bundle_flag_produces_tarball() {
    let tmp_dir = std::env::temp_dir().join(format!("simetro-bundle-tar-{}", std::process::id()));
    let tar_path = tmp_dir.with_extension("tar");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = std::fs::remove_file(&tar_path);

    let scene = scene_path();
    let scene_str = scene.to_string_lossy();
    let dir_str = tmp_dir.to_string_lossy();

    let out = Command::new(bin())
        .args([
            "export-session",
            "--scene",
            &scene_str,
            "--ticks",
            "5",
            "--seed",
            "42",
            "--out",
            &dir_str,
            "--bundle",
        ])
        .output()
        .expect("export-session --bundle");

    assert!(
        out.status.success(),
        "export-session --bundle failed: stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(
        tar_path.exists(),
        "expected tarball at {}",
        tar_path.display()
    );
    assert!(
        tmp_dir.exists(),
        "bundle directory must still exist alongside tarball for backward-compat"
    );

    // Sanity: tarball reproduces the bundle layout under <basename>/
    let f = std::fs::File::open(&tar_path).expect("open tar");
    let mut archive = tar::Archive::new(f);
    let names: Vec<String> = archive
        .entries()
        .expect("entries")
        .filter_map(Result::ok)
        .map(|e| e.path().expect("path").to_string_lossy().into_owned())
        .collect();
    let prefix = tmp_dir
        .file_name()
        .expect("basename")
        .to_string_lossy()
        .into_owned();
    for required in &["scene.json", "manifest.json", "baseline.hash"] {
        let full = format!("{prefix}/{required}");
        assert!(
            names.contains(&full),
            "tar missing required entry {full:?}; have {names:?}"
        );
    }

    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = std::fs::remove_file(&tar_path);
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
