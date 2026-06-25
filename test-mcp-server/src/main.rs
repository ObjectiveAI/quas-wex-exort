//! A tiny rmcp MCP server fixture for quas-wex-exort's integration tests.
//!
//! Launched by the host as `test-mcp-server mcp <server> begin`; it binds a
//! loopback port, announces its URL on stdout (`{"type":"mcp","url":...}`), and
//! serves two deterministic tools (`echo`, `add`) over streamable HTTP. A mock
//! agent puts this in its arsenal so quas-wex-exort's task/multi_call tools have
//! a real tool to invoke via `agents tools call`.

use std::io::Write;
use std::sync::Arc;

use rmcp::{
    ServerHandler, tool, tool_handler, tool_router,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct EchoArgs {
    /// The string to echo back.
    input: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AddArgs {
    a: i64,
    b: i64,
}

#[derive(Clone)]
struct TestServer {
    tool_router: ToolRouter<Self>,
}

impl TestServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl TestServer {
    #[tool(name = "echo", description = "Echo the input string back verbatim.")]
    async fn echo(&self, Parameters(req): Parameters<EchoArgs>) -> String {
        req.input
    }

    #[tool(name = "add", description = "Add two integers and return the sum.")]
    async fn add(&self, Parameters(req): Parameters<AddArgs>) -> String {
        (req.a + req.b).to_string()
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TestServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        // The aggregation prefix the agent sees: tools surface as `test_echo` /
        // `test_add` (matches the manifest's mcp_server name "test").
        info.server_info.name = "test".into();
        info
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // The host launches us as `test-mcp-server mcp <server> begin`.
    let args: Vec<String> = std::env::args().collect();
    if !(args.len() >= 4 && args[1] == "mcp" && args[3] == "begin") {
        eprintln!("usage: test-mcp-server mcp <server> begin");
        std::process::exit(2);
    }

    let service = StreamableHttpService::new(
        || Ok(TestServer::new()),
        Arc::new(LocalSessionManager::default()),
        {
            let mut cfg = StreamableHttpServerConfig::default();
            cfg.stateful_mode = true;
            cfg.sse_keep_alive = None;
            cfg
        },
    );
    let router = axum::Router::new().fallback_service(service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    // Announce the URL (flushed) before serving — the host reads this line and
    // dials the endpoint; without a flush it would sit buffered behind the
    // never-returning `serve`.
    let line = format!("{{\"type\":\"mcp\",\"url\":\"http://{addr}\"}}");
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    handle.write_all(line.as_bytes())?;
    handle.write_all(b"\n")?;
    handle.flush()?;
    drop(handle);

    axum::serve(listener, router).await
}
