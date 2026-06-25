//! Shared helpers used by more than one toolset: invoking a tool through the
//! ObjectiveAI CLI, and extracting required request headers.

use objectiveai_sdk::cli::command::agents::tools::call as tools_call;
use objectiveai_sdk::cli::command::plugin::PluginExecutor;
use rmcp::ErrorData;
use rmcp::model::Extensions;

/// Header carrying the caller's agent instance hierarchy (keys the task map and
/// is the completion-message target).
pub const AIH_HEADER: &str = "x-objectiveai-agent-instance-hierarchy";
/// Header carrying the caller's response id (scopes the underlying tool call).
pub const RESPONSE_ID_HEADER: &str = "x-objectiveai-response-id";

/// Read a required header off the request extensions, erroring if it is absent
/// or empty.
pub fn required_header(extensions: &Extensions, name: &str) -> Result<String, ErrorData> {
    let parts = extensions
        .get::<http::request::Parts>()
        .ok_or_else(|| ErrorData::invalid_params("missing request parts", None))?;
    parts
        .headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ErrorData::invalid_params(format!("missing required header: {name}"), None))
}

/// Run `agents tools call` for `tool`+`arguments` scoped to `response_id`,
/// returning the native MCP tool result, or a raw executor error as a string.
pub async fn call_tool(
    executor: &PluginExecutor,
    response_id: &str,
    tool: &str,
    arguments: serde_json::Value,
) -> Result<objectiveai_sdk::mcp::tool::CallToolResult, String> {
    let params: objectiveai_sdk::mcp::tool::CallToolRequestParams =
        serde_json::from_value(serde_json::json!({ "name": tool, "arguments": arguments }))
            .map_err(|e| format!("invalid tool arguments: {e}"))?;
    let request = tools_call::Request {
        path_type: tools_call::Path::AgentsToolsCall,
        response_id: response_id.to_string(),
        params,
        base: Default::default(),
    };
    tools_call::execute(executor, request, None)
        .await
        .map_err(|e| e.to_string())
}
