mod commands;
mod config;
mod context;
mod mcp;

use clap::Parser;

use commands::Cli;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let _ = dotenv::dotenv();
    Cli::parse().run().await
}
