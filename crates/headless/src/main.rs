//! `simetro-headless` binary.
//!
//! Step 1 stub. Subcommands wired up in Step 13.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "simetro-headless", version)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a scene for N ticks.
    Run,
    /// Benchmark tick throughput.
    Bench,
    /// Emit deterministic state hash.
    Hash,
    /// Replay an AgentLog (P2).
    Replay,
    /// Export a session bundle (scene + AgentLog + tracing + hash).
    ExportSession,
}

fn main() {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Run => println!("run: stub (Step 1)"),
        Cmd::Bench => println!("bench: stub (Step 1)"),
        Cmd::Hash => println!("hash: stub (Step 1)"),
        Cmd::Replay => println!("replay: stub (Step 1)"),
        Cmd::ExportSession => println!("export-session: stub (Step 1)"),
    }
}
