//! The `tasks` toolset: create / list / wait / cancel background task
//! invocations of other MCP tools in the agent's arsenal.
//!
//! The tool bodies are thin: they extract the required headers and delegate to
//! the [`TaskRegistry`] engine in [`registry`].

mod registry;

use rmcp::{
    ErrorData, RoleServer, tool, tool_router,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content},
    service::RequestContext,
};
use schemars::JsonSchema;
use serde::Deserialize;

use super::QuasWexExortMcp;
use super::common::{AIH_HEADER, RESPONSE_ID_HEADER, required_header};
pub use registry::TaskRegistry;

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
        description = "Create a task: a background invocation of another MCP tool in your arsenal. Returns the task id immediately."
    )]
    async fn task_create(
        &self,
        Parameters(req): Parameters<TaskCreateRequest>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let aih = required_header(&ctx.extensions, AIH_HEADER)?;
        let response_id = required_header(&ctx.extensions, RESPONSE_ID_HEADER)?;
        let id = self.tasks.create(aih, response_id, req.tool, req.arguments);
        Ok(CallToolResult::success(vec![Content::text(id)]))
    }

    #[tool(name = "list", description = "List your tasks and their status.")]
    async fn task_list(
        &self,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let aih = required_header(&ctx.extensions, AIH_HEADER)?;
        Ok(self.tasks.list(&aih))
    }

    #[tool(
        name = "wait",
        description = "Wait for a task to complete and return its result. Suppresses the task's completion message."
    )]
    async fn task_wait(
        &self,
        Parameters(req): Parameters<TaskWaitRequest>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let aih = required_header(&ctx.extensions, AIH_HEADER)?;
        Ok(self.tasks.wait(&aih, &req.task_id).await)
    }

    #[tool(name = "cancel", description = "Cancel a running task.")]
    async fn task_cancel(
        &self,
        Parameters(req): Parameters<TaskCancelRequest>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let aih = required_header(&ctx.extensions, AIH_HEADER)?;
        Ok(self.tasks.cancel(&aih, &req.task_id))
    }
}
