//! The `multi_call` toolset: invoke several MCP tools concurrently in one
//! request and concatenate their results.

use objectiveai_sdk::cli::command::plugin::PluginExecutor;
use rmcp::{
    ErrorData, RoleServer, tool, tool_router,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content},
    service::RequestContext,
};
use schemars::JsonSchema;
use serde::Deserialize;

use super::QuasWexExortMcp;
use super::common::{self, RESPONSE_ID_HEADER};

/// Wire names of the multi_call tools, used to gate them in `list_tools`.
pub const TOOL_NAMES: &[&str] = &["multi_call"];

/// Whether `name` is one of the multi_call tools.
pub fn is_multi_tool(name: &str) -> bool {
    TOOL_NAMES.contains(&name)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MultiCallRequest {
    /// The tool calls to run concurrently.
    pub calls: Vec<ToolCall>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ToolCall {
    /// Name of the MCP tool (in your arsenal) to invoke.
    pub tool: String,
    /// Arguments to pass to that tool.
    pub arguments: serde_json::Value,
}

#[tool_router(router = multi_tools, vis = "pub")]
impl QuasWexExortMcp {
    #[tool(
        name = "multi_call",
        description = "Invoke multiple MCP tools concurrently in one request; their results are concatenated, each prefixed with its index and tool name."
    )]
    async fn multi_call_tool(
        &self,
        Parameters(req): Parameters<MultiCallRequest>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let response_id = common::required_header(&ctx.extensions, RESPONSE_ID_HEADER)?;
        Ok(run(&self.context.executor, &response_id, req.calls).await)
    }
}

/// Run every call concurrently (all dispatched before any is awaited),
/// preserving input order, and join the results into one response.
async fn run(
    executor: &PluginExecutor,
    response_id: &str,
    calls: Vec<ToolCall>,
) -> CallToolResult {
    if calls.is_empty() {
        return CallToolResult::error(vec![Content::text(
            "multi_call requires at least one call",
        )]);
    }

    // Tool names are needed for the per-segment prefixes after the calls run.
    let tools: Vec<String> = calls.iter().map(|c| c.tool.clone()).collect();
    let futures = calls.into_iter().map(|c| async move {
        common::call_tool(executor, response_id, &c.tool, c.arguments).await
    });
    let results = futures::future::join_all(futures).await;

    let mut content: Vec<Content> = Vec::new();
    let mut all_errored = true;
    for (i, (tool, result)) in tools.iter().zip(results).enumerate() {
        if i > 0 {
            content.push(Content::text("\n\n"));
        }
        content.push(Content::text(format!("[result {i} ({tool})]\n")));
        match result {
            Ok(native) => {
                // At least one call succeeded — the joined result is non-error.
                all_errored = false;
                // Push each result block verbatim (native -> rmcp via the SDK bridge).
                for block in native.content {
                    content.push(block.into());
                }
            }
            // Raw (non-MCP) executor failure — this segment is the error text.
            Err(e) => content.push(Content::text(e)),
        }
    }

    if all_errored {
        CallToolResult::error(content)
    } else {
        CallToolResult::success(content)
    }
}
