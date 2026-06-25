//! The quas-wex-exort agent-facing MCP server.
//!
//! A streamable-HTTP `rmcp` server whose tool routers expose the plugin's
//! capabilities to ObjectiveAI agents. Currently only the `tasks` toolset is
//! wired in (stubbed); `multi_call` and `python` follow (issues #3, #4).

mod run;
mod tasks;

use rmcp::{
    ServerHandler, tool_handler,
    handler::server::router::tool::ToolRouter,
    model::{ServerCapabilities, ServerInfo},
};

pub use run::run;

/// The MCP server handler. Cheap to `clone` (the service factory clones one
/// per session); all real state will hang off here as the toolsets gain
/// backends.
#[derive(Clone)]
pub struct QuasWexExortMcp {
    pub tool_router: ToolRouter<Self>,
}

impl QuasWexExortMcp {
    pub fn new() -> Self {
        Self {
            tool_router: Self::task_tools(),
        }
    }
}

impl Default for QuasWexExortMcp {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for QuasWexExortMcp {
    fn get_info(&self) -> ServerInfo {
        // Start from the default (server name/version come from the build env)
        // and advertise tool support.
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }
}
