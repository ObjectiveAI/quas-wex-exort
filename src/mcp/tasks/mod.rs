//! The `tasks` toolset: create / list / wait / cancel background task
//! invocations of other MCP tools in the agent's arsenal.
//!
//! Scaffolding only — every tool returns a "hello world" stub. The real
//! backend (the objectiveai-sdk plugin executor), task state, and the
//! completion wakeup are deferred (issues #1, #2).

use rmcp::{
    ErrorData, tool, tool_router,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content},
};
use schemars::JsonSchema;
use serde::Deserialize;

use super::QuasWexExortMcp;

/// Wire names of the task tools, used to gate them in `list_tools`.
pub const TOOL_NAMES: &[&str] = &["create", "list", "wait", "cancel"];

/// Whether `name` is one of the task tools.
pub fn is_task_tool(name: &str) -> bool {
    TOOL_NAMES.contains(&name)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskCreateRequest {
    /// Name of the MCP tool (in your arsenal) to invoke as a background task.
    pub tool: String,
    /// Arguments to pass to that tool.
    pub arguments: serde_json::Value,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskWaitRequest {
    /// Id of the task to wait on.
    pub task_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskCancelRequest {
    /// Id of the task to cancel.
    pub task_id: String,
}

#[tool_router(router = task_tools, vis = "pub")]
impl QuasWexExortMcp {
    #[tool(
        name = "create",
        description = "Create a task: a background invocation of another MCP tool in your arsenal."
    )]
    async fn task_create(
        &self,
        Parameters(_req): Parameters<TaskCreateRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text("hello world")]))
    }

    #[tool(name = "list", description = "List your tasks and their status.")]
    async fn task_list(&self) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text("hello world")]))
    }

    #[tool(name = "wait", description = "Wait for a task to complete.")]
    async fn task_wait(
        &self,
        Parameters(_req): Parameters<TaskWaitRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text("hello world")]))
    }

    #[tool(name = "cancel", description = "Cancel a running task.")]
    async fn task_cancel(
        &self,
        Parameters(_req): Parameters<TaskCancelRequest>,
    ) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![Content::text("hello world")]))
    }
}
