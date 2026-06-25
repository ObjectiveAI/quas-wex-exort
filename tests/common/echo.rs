//! An in-process rmcp echo/add MCP server the tests spin up as a RAW MCP server
//! (bound on `127.0.0.1:0`, a unique port per test binary). It's passed to the
//! mock agent as a plain `mcp_servers` URL — not a plugin — so quas-wex-exort's
//! task/multi_call tools have a real tool to invoke via `agents tools call`.

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
    input: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AddArgs {
    a: i64,
    b: i64,
}

#[derive(Clone)]
struct Echo {
    tool_router: ToolRouter<Self>,
}

impl Echo {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl Echo {
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
impl ServerHandler for Echo {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        // The aggregation prefix the agent sees (tools surface as test_echo / test_add).
        info.server_info.name = "test".into();
        info
    }
}

/// A running echo server; aborts its task on drop (so the test doesn't leak it).
pub struct EchoServer {
    url: String,
    handle: tokio::task::JoinHandle<()>,
}

impl EchoServer {
    /// The connect URL to pass to [`Agent::mcp_server`](super::Agent::mcp_server).
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl Drop for EchoServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Spawn the echo server on a background task at `127.0.0.1:0`.
pub async fn spawn() -> EchoServer {
    let service = StreamableHttpService::new(
        || Ok(Echo::new()),
        Arc::new(LocalSessionManager::default()),
        {
            let mut cfg = StreamableHttpServerConfig::default();
            cfg.stateful_mode = true;
            cfg.sse_keep_alive = None;
            cfg
        },
    );
    let router = axum::Router::new().fallback_service(service);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind echo server");
    let addr = listener.local_addr().expect("echo server local_addr");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    EchoServer {
        url: format!("http://{addr}"),
        handle,
    }
}
