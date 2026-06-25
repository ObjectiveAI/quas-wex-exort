//! The `quas-wex-exort` command-line interface.

mod daemon;

use clap::{Parser, Subcommand};

/// Programmatic invocation of MCP tools and the ObjectiveAI CLI for ObjectiveAI
/// agents.
#[derive(Parser)]
#[command(name = "quas-wex-exort", version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Daemon commands.
    #[command(subcommand)]
    Daemon(daemon::Commands),
}

impl Cli {
    pub async fn run(self) -> std::io::Result<()> {
        match self.command {
            Commands::Daemon(command) => command.run().await,
        }
    }
}
