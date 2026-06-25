//! The `mcp` command group.

mod quas_wex_exort;

use clap::Subcommand;

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
    pub async fn run(self) -> std::io::Result<()> {
        match self {
            Commands::QuasWexExort { command } => command.run().await,
        }
    }
}
