//! The `list_tools` toolset: page through the names of the tools in the agent's
//! arsenal. Ungated — it shows up in every mode (it is not a `tasks`/`multi_call`
//! tool, so `tool_allowed`'s `else` branch always permits it).

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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListToolsArgs {
    /// Number of tool names to skip from the start of the arsenal.
    pub offset: u32,
    /// Maximum number of tool names to return.
    pub count: u32,
}

#[tool_router(router = listing_tools, vis = "pub")]
impl QuasWexExortMcp {
    #[tool(
        name = "list_tools",
        description = "List the names of the tools in your arsenal, paginated by `offset` and `count`. Returns only tool names, as a JSON array of strings."
    )]
    async fn list_tools_tool(
        &self,
        Parameters(args): Parameters<ListToolsArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let response_id = common::required_header(&ctx.extensions, RESPONSE_ID_HEADER)?;
        let result = match common::list_tool_names(&self.context.executor, &response_id).await {
            Ok(names) => {
                let page: Vec<String> = names
                    .into_iter()
                    .skip(args.offset as usize)
                    .take(args.count as usize)
                    .collect();
                let body = serde_json::to_string(&page).unwrap_or_else(|_| "[]".to_string());
                CallToolResult::success(vec![Content::text(body)])
            }
            Err(e) => CallToolResult::error(vec![Content::text(e)]),
        };
        Ok(result)
    }
}
