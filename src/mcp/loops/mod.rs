//! The `loops` toolset: begin / end recurring message loops.
//!
//! A **loop** delivers a fixed message back to the agent every interval, until
//! ended. The tool bodies are thin: they extract the required headers and
//! delegate to the [`LoopRegistry`] engine in [`registry`].

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
use super::common::{AIH_HEADER, required_header};
pub use registry::LoopRegistry;

/// Wire names of the loop tools, used to gate them in `list_tools`.
pub const TOOL_NAMES: &[&str] = &["begin_loop", "end_loop"];

/// Whether `name` is one of the loop tools.
pub fn is_loop_tool(name: &str) -> bool {
    TOOL_NAMES.contains(&name)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BeginLoopRequest {
    /// Seconds between messages (minimum 1). The first message arrives after
    /// one full interval.
    pub interval_seconds: u64,
    /// The message delivered to you each interval, verbatim.
    pub message: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EndLoopRequest {
    /// Id of the loop to end.
    pub loop_id: String,
}

#[tool_router(router = loop_tools, vis = "pub")]
impl QuasWexExortMcp {
    #[tool(
        name = "begin_loop",
        description = "Begin a loop: you will be messaged with the given message every interval, until you end the loop. Returns the loop id immediately."
    )]
    async fn begin_loop(
        &self,
        Parameters(req): Parameters<BeginLoopRequest>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let aih = required_header(&ctx.extensions, AIH_HEADER)?;
        if req.interval_seconds == 0 {
            return Ok(CallToolResult::error(vec![Content::text(
                "interval_seconds must be at least 1",
            )]));
        }
        let id = self.loops.begin(aih, req.interval_seconds, req.message);
        Ok(CallToolResult::success(vec![Content::text(id)]))
    }

    #[tool(name = "end_loop", description = "End a loop, stopping its messages.")]
    async fn end_loop(
        &self,
        Parameters(req): Parameters<EndLoopRequest>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let aih = required_header(&ctx.extensions, AIH_HEADER)?;
        Ok(self.loops.end(&aih, &req.loop_id))
    }
}
