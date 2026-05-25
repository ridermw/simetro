//! End-to-end test: spawn `simetro-bridge` as a subprocess and run a
//! request → reply cycle through stdin/stdout NDJSON framing.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use simetro_agent_bridge::wire::{
    hello_envelope, read_envelope, shutdown_envelope, write_envelope, BridgeMessage,
};
use simetro_engine::lifecycle::{AgentRequest, RequestId};
use simetro_protocol::Envelope;
use std::io::{BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

fn bridge_bin() -> String {
    // CARGO_BIN_EXE_<name> is set by cargo for integration tests in
    // the same crate that defines the [[bin]].
    env!("CARGO_BIN_EXE_simetro-bridge").to_string()
}

#[test]
fn request_round_trips_through_bridge_subprocess() {
    let mut child = Command::new(bridge_bin())
        .env("SIMETRO_BRIDGE_BACKEND", "mock")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn simetro-bridge");

    let mut stdin = child.stdin.take().expect("child stdin");
    let stdout = child.stdout.take().expect("child stdout");
    let mut reader = BufReader::new(stdout);

    // The bridge writes its own Hello first.
    let hello = read_envelope(&mut reader)
        .expect("read hello")
        .expect("hello envelope present");
    assert!(
        matches!(hello.payload, BridgeMessage::Hello { .. }),
        "first envelope must be Hello, got {:?}",
        hello.payload
    );
    assert!(hello.is_compatible(), "hello must use current schema");

    // Send our Hello + one Request.
    write_envelope(&mut stdin, &hello_envelope(0, "test-harness")).expect("write hello");

    let req = AgentRequest {
        id: RequestId {
            timeline_id: 1,
            agent_id: "test-agent".to_string(),
            source_tick: 10,
            attempt: 0,
        },
        deadline_ticks: 30,
        observation_json: "{\"tick\":10,\"movers\":[]}".to_string(),
    };
    let req_env = Envelope::new(1, BridgeMessage::Request(req.clone()));
    write_envelope(&mut stdin, &req_env).expect("write request");

    // Read the reply.
    let reply_env = read_envelope(&mut reader)
        .expect("read reply")
        .expect("reply present");
    match reply_env.payload {
        BridgeMessage::Reply(reply) => {
            assert_eq!(reply.id, req.id, "reply must echo the request id");
        }
        other => panic!("expected Reply, got {other:?}"),
    }

    // Shut down cleanly.
    write_envelope(&mut stdin, &shutdown_envelope(2)).expect("write shutdown");
    drop(stdin); // close stdin so the child sees EOF if Shutdown wasn't received first

    // Bridge should exit with code 0 within a reasonable timeout.
    let status = wait_with_timeout(&mut child, Duration::from_secs(5)).expect("bridge exits");
    assert!(status.success(), "bridge exit code = {status:?}");
}

#[test]
fn eof_on_stdin_exits_cleanly() {
    let mut child = Command::new(bridge_bin())
        .env("SIMETRO_BRIDGE_BACKEND", "mock")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");

    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);

    // Read Hello, then immediately close stdin.
    let _hello = read_envelope(&mut reader)
        .expect("read hello")
        .expect("hello present");
    drop(stdin);

    let status = wait_with_timeout(&mut child, Duration::from_secs(5)).expect("bridge exits");
    assert!(
        status.success(),
        "EOF on stdin must produce a clean exit; got {status:?}"
    );
}

#[test]
fn unknown_backend_exits_with_code_two() {
    let mut child = Command::new(bridge_bin())
        .env("SIMETRO_BRIDGE_BACKEND", "wat")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");

    let status = wait_with_timeout(&mut child, Duration::from_secs(5)).expect("bridge exits");
    assert_eq!(
        status.code(),
        Some(2),
        "unknown backend must exit code 2; got {status:?}"
    );
}

/// Cross-platform child-process wait with timeout. The std lib's
/// `Child::wait` blocks forever; we poll `try_wait` so a hanging bridge
/// doesn't deadlock the test.
fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> std::io::Result<std::process::ExitStatus> {
    let start = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if start.elapsed() >= timeout {
            // Forcibly kill so we don't leak. ignore kill errors.
            let _ = child.kill();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "bridge subprocess did not exit within timeout",
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

// Squash unused-import warning if a test panics during dev iteration.
#[allow(dead_code)]
fn _use_write(_w: &mut dyn Write) {}
