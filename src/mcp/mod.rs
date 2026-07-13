//! The quas-wex-exort agent-facing MCP server.
//!
//! A streamable-HTTP `rmcp` server whose tool routers expose the plugin's
//! capabilities to ObjectiveAI agents. The `tasks`, `multi_call`, `loops`, and
//! `list_tools` toolsets are wired in; `python` follows (issue #4).

mod arguments;
mod common;
mod list_tools;
mod loops;
mod multi_call;
mod run;
mod tasks;

use std::sync::Arc;

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
use loops::LoopRegistry;
use tasks::TaskRegistry;
use crate::context::Context;

pub use run::run;

/// The MCP server handler. Cheap to `clone` (the service factory clones one per
/// session); shared state lives behind `Arc`.
#[derive(Clone)]
pub struct QuasWexExortMcp {
    pub tool_router: ToolRouter<Self>,
    /// The runtime context (config + plugin executor), shared across all session
    /// clones. `multi_call` reads `context.executor` directly.
    context: Arc<Context>,
    /// The in-process task engine, shared across all session clones.
    tasks: Arc<TaskRegistry>,
    /// The in-process loop engine, shared across all session clones.
    loops: Arc<LoopRegistry>,
}

impl QuasWexExortMcp {
    pub fn new(context: Arc<Context>) -> Self {
        Self {
            tool_router: Self::task_tools()
                + Self::multi_tools()
                + Self::loop_tools()
                + Self::listing_tools(),
            tasks: Arc::new(TaskRegistry::new(context.executor.clone())),
            loops: Arc::new(LoopRegistry::new(context.executor.clone())),
            context,
        }
    }
}

/// Whether a tool is allowed for the session, per the toolset flags in the
/// `x-objectiveai-arguments` header. Tools outside a gated toolset are always
/// allowed; a gated tool with its flag off is treated as nonexistent.
fn tool_allowed(name: &str, args: Option<Arguments>) -> bool {
    if tasks::is_task_tool(name) {
        args.map(|a| a.tasks).unwrap_or(false)
    } else if multi_call::is_multi_tool(name) {
        args.map(|a| a.multi).unwrap_or(false)
    } else if loops::is_loop_tool(name) {
        args.map(|a| a.loops).unwrap_or(false)
    } else {
        true
    }
}

// `#[tool_handler]` generates `get_tool` from the router and only fills in
// `call_tool`/`list_tools`/`get_info` when we don't define them — so our custom
// `call_tool`/`list_tools` (toolset gating) and `get_info` below take precedence.
#[tool_handler(router = self.tool_router)]
impl ServerHandler for QuasWexExortMcp {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        // `ServerInfo::default()` reports rmcp's own crate name ("rmcp"); set
        // ours explicitly — the host's MCP proxy prefixes the agent-visible tool
        // names with this `serverInfo.name` (so tools surface as
        // `quas-wex-exort_create`, `quas-wex-exort_multi_call`, …).
        info.server_info.name = "quas-wex-exort".into();
        info
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        // Gate each toolset on its session argument. When disabled we return the
        // exact error rmcp emits for a missing tool, so a gated tool is
        // indistinguishable from one that doesn't exist.
        let args = Arguments::extract(&context.extensions);
        if !tool_allowed(request.name.as_ref(), args) {
            return Err(ErrorData::invalid_params("tool not found", None));
        }
        let tcc = ToolCallContext::new(self, request, context);
        self.tool_router.call(tcc).await
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        // Each toolset is gated on the session's `x-objectiveai-arguments`
        // header: a gated tool is shown only when its flag is true, and hidden
        // when the header is absent or unparseable.
        let args = Arguments::extract(&context.extensions);
        let tools = self
            .tool_router
            .list_all()
            .into_iter()
            .filter(|t| tool_allowed(t.name.as_ref(), args))
            .collect();
        Ok(ListToolsResult {
            tools,
            meta: None,
            next_cursor: None,
        })
    }
}
