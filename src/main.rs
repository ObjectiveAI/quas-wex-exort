mod config;
mod context;
mod mcp;

use context::Context;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let _ = dotenv::dotenv();
    let ctx = Context::new();
    mcp::run(&ctx).await
}
