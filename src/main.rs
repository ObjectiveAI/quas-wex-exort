mod config;
mod context;
mod mcp;

use std::sync::Arc;

use context::Context;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let _ = dotenv::dotenv();
    let ctx = Arc::new(Context::new());
    mcp::run(ctx).await
}
