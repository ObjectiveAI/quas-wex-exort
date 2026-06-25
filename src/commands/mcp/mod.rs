//! The `mcp` command group.

mod quas_wex_exort;

use std::sync::Arc;

use clap::Subcommand;

use crate::context::Context;

#[derive(Subcommand)]
pub enum Commands {
    /// quas-wex-exort MCP server commands.
    #[command(name = "quas-wex-exort")]
    QuasWexExort {
        #[command(subcommand)]
        command: quas_wex_exort::Commands,
    },
}

impl Commands {
    pub async fn run(self, ctx: Arc<Context>) -> std::io::Result<()> {
        match self {
            Commands::QuasWexExort { command } => command.run(ctx).await,
        }
    }
}
