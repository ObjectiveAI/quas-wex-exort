//! The quas-wex-exort agent-facing MCP server.
//!
//! A streamable-HTTP `rmcp` server whose tool routers expose the plugin's
//! capabilities to ObjectiveAI agents. Currently only the `tasks` toolset is
//! wired in (stubbed); `multi_call` and `python` follow (issues #3, #4).

mod arguments;
mod run;
mod tasks;

use std::sync::Arc;

use objectiveai_sdk::cli::command::plugin::PluginExecutor;
use rmcp::{
    ErrorData, RoleServer, ServerHandler, tool_handler,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext},
    model::{
        CallToolRequestParams, CallToolResult, ListToolsResult, PaginatedRequestParams,
        ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
};

use arguments::Arguments;
use tasks::TaskRegistry;

pub use run::run;

/// The MCP server handler. Cheap to `clone` (the service factory clones one per
/// session); shared state (the task registry) lives behind `Arc`.
#[derive(Clone)]
pub struct QuasWexExortMcp {
    pub tool_router: ToolRouter<Self>,
    /// The in-process task engine, shared across all session clones.
    tasks: Arc<TaskRegistry>,
}

impl QuasWexExortMcp {
    pub fn new(executor: PluginExecutor) -> Self {
        Self {
            tool_router: Self::task_tools(),
            tasks: Arc::new(TaskRegistry::new(executor)),
        }
    }
}

// `#[tool_handler]` generates `get_tool` from the router and only fills in
// `call_tool`/`list_tools`/`get_info` when we don't define them — so our custom
// `call_tool`/`list_tools` (toolset gating) and `get_info` below take precedence.
#[tool_handler(router = self.tool_router)]
impl ServerHandler for QuasWexExortMcp {
    fn get_info(&self) -> ServerInfo {
        // Start from the default (server name/version come from the build env)
        // and advertise tool support.
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        // Gate task tools on the session's `tasks` argument. When disabled we
        // return the exact error rmcp emits for a missing tool, so a gated tool
        // is indistinguishable from one that doesn't exist.
        if tasks::is_task_tool(request.name.as_ref()) {
            let tasks_enabled = Arguments::extract(&context.extensions)
                .map(|a| a.tasks)
                .unwrap_or(false);
            if !tasks_enabled {
                return Err(ErrorData::invalid_params("tool not found", None));
            }
        }
        let tcc = ToolCallContext::new(self, request, context);
        self.tool_router.call(tcc).await
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        // The task tools are gated on the session's `x-objectiveai-arguments`
        // header: shown only when `tasks` is true, and hidden when the header
        // is absent or unparseable.
        let tasks_enabled = Arguments::extract(&context.extensions)
            .map(|a| a.tasks)
            .unwrap_or(false);
        let tools = self
            .tool_router
            .list_all()
            .into_iter()
            .filter(|t| tasks_enabled || !tasks::is_task_tool(t.name.as_ref()))
            .collect();
        Ok(ListToolsResult {
            tools,
            meta: None,
            next_cursor: None,
        })
    }
}
