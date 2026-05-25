//! `cargo xtask copilot-smoke` — spawns the real `copilot --acp`
//! subprocess once and verifies it can be launched cleanly.
//!
//! Per spec §3 task 12, this is **human-run only**:
//! - Requires `copilot` CLI on PATH (`which copilot`)
//! - Requires authenticated `gh auth` with the `copilot` scope
//! - NOT invoked by CI (runners don't have copilot installed)
//!
//! What it actually checks:
//! - `copilot` binary is on PATH.
//! - `copilot --acp` spawns successfully.
//! - The subprocess does NOT exit within a readiness window
//!   (`READINESS_WINDOW`, default 5s). ACP servers idle waiting for
//!   stdin; an immediate exit means auth or environment is broken.
//!
//! We intentionally do NOT try to read stdout — ACP servers produce
//! nothing until they receive a `tools/initialize` envelope, and a
//! cross-platform non-blocking pipe read is out of scope for a
//! "effort: S" smoke. Once the captured ACP fixture lands (spec
//! §2.5), this smoke can be extended to issue a real `initialize`
//! and verify the reply shape.
//!
//! Exit codes:
//!   0  — copilot spawned and stayed alive past the readiness window
//!   1  — `copilot` binary not found on PATH, or spawn failed
//!   2  — copilot exited within the readiness window (auth / env
//!        broken)
//!   64 — usage error

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;

/// How long copilot must stay alive without exiting to be considered
/// "successfully launched". ACP servers idle waiting for stdin, so
/// any process that's still running at this point is healthy.
const READINESS_WINDOW: Duration = Duration::from_secs(5);

pub fn run(args: &[String]) -> std::process::ExitCode {
    let mut verbose = false;
    for arg in args {
        match arg.as_str() {
            "--verbose" | "-v" => verbose = true,
            "--help" | "-h" => {
                print_help();
                return std::process::ExitCode::SUCCESS;
            }
            other => {
                eprintln!("copilot-smoke: unknown flag `{other}`");
                print_help();
                return std::process::ExitCode::from(64);
            }
        }
    }

    println!("[xtask copilot-smoke] looking for `copilot` on PATH…");
    if !binary_exists("copilot") {
        eprintln!(
            "[xtask copilot-smoke] FAIL: `copilot` not found on PATH.\n\
             Install GitHub Copilot CLI first:\n\
             https://docs.github.com/en/copilot/github-copilot-cli"
        );
        return std::process::ExitCode::from(1);
    }
    println!("[xtask copilot-smoke] found `copilot`");

    println!("[xtask copilot-smoke] spawning `copilot --acp`…");
    let mut child = match Command::new("copilot")
        .arg("--acp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(err) => {
            eprintln!("[xtask copilot-smoke] FAIL: spawn failed: {err}");
            return std::process::ExitCode::from(1);
        }
    };
    let child_pid = child.id();
    println!(
        "[xtask copilot-smoke] spawned PID {child_pid}; waiting {:?} for \
         readiness…",
        READINESS_WINDOW
    );

    // Poll try_wait every 100ms for READINESS_WINDOW. If the child
    // exits early, that's a failure. If it's still running at the
    // end, that's success — ACP servers idle on stdin.
    let start = std::time::Instant::now();
    while start.elapsed() < READINESS_WINDOW {
        match child.try_wait() {
            Ok(Some(status)) => {
                eprintln!(
                    "[xtask copilot-smoke] FAIL: copilot exited within the \
                     readiness window (status={status:?}). Likely cause: \
                     not authenticated. Run `gh auth status` and verify the \
                     `copilot` scope is granted."
                );
                drain_stderr_for_diagnostics(&mut child, verbose);
                return std::process::ExitCode::from(2);
            }
            Ok(None) => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(err) => {
                eprintln!("[xtask copilot-smoke] FAIL: try_wait error: {err}");
                let _ = child.kill();
                return std::process::ExitCode::from(2);
            }
        }
    }

    println!(
        "[xtask copilot-smoke] OK — copilot stayed alive past {:?}; \
         shutting it down",
        READINESS_WINDOW
    );
    let _ = child.kill();
    let _ = child.wait();
    std::process::ExitCode::SUCCESS
}

fn print_help() {
    println!(
        r"cargo xtask copilot-smoke — human-run copilot --acp smoke test

USAGE:
    cargo xtask copilot-smoke [--verbose|-v]

OPTIONS:
    --verbose, -v    Stream copilot stderr to this process's stderr on failure.

EXIT CODES:
    0  copilot launched and stayed alive past the readiness window
    1  copilot binary not found on PATH, or spawn failed
    2  copilot exited within the readiness window (auth/env broken)
    64 usage error"
    );
}

fn binary_exists(name: &str) -> bool {
    #[cfg(unix)]
    let probe = Command::new("which").arg(name).output();
    #[cfg(windows)]
    let probe = Command::new("where").arg(name).output();
    matches!(probe, Ok(out) if out.status.success() && !out.stdout.is_empty())
}

fn drain_stderr_for_diagnostics(child: &mut std::process::Child, verbose: bool) {
    if let Some(mut stderr) = child.stderr.take() {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        if !buf.is_empty() && verbose {
            eprintln!(
                "[xtask copilot-smoke] copilot stderr:\n{}",
                String::from_utf8_lossy(&buf)
            );
        } else if !buf.is_empty() {
            eprintln!(
                "[xtask copilot-smoke] (run with --verbose to see copilot's \
                 stderr; {} bytes captured)",
                buf.len()
            );
        }
    }
}
