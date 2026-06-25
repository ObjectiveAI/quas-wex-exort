//! The `daemon` command group.

mod begin;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    /// Launch the MCP server.
    Begin(begin::Args),
}

impl Commands {
    pub async fn run(self) -> std::io::Result<()> {
        match self {
            Commands::Begin(args) => args.run().await,
        }
    }
}
