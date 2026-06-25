//! The `mcp quas-wex-exort` command group.

mod begin;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// Launch the MCP server (via the daemon) and announce its URL.
    Begin(begin::Args),
}

impl Commands {
    pub async fn run(self) -> std::io::Result<()> {
        match self {
            Commands::Begin(args) => args.run().await,
        }
    }
}
