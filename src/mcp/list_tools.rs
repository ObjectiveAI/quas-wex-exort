//! The `list_tools` toolset: page through the tools in the agent's arsenal.
//! Ungated — it shows up in every mode (it is not a `tasks`/`multi_call` tool,
//! so `tool_allowed`'s `else` branch always permits it).
//!
//! `offset`/`count` paginate; the four boolean flags select which native fields
//! each item carries. With every flag false the result is a JSON array of names
//! (strings); with any flag set it is a JSON array of objects keyed by the
//! native field names (`name`, `description`, `inputSchema`, `outputSchema`,
//! `annotations`), each selected field omitted when absent on the tool.

use rmcp::{
    ErrorData, RoleServer, tool, tool_router,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content},
    service::RequestContext,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value};

use super::QuasWexExortMcp;
use super::common::{self, RESPONSE_ID_HEADER};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListToolsArgs {
    /// Number of tools to skip from the start of the arsenal.
    pub offset: u32,
    /// Maximum number of tools to return.
    pub count: u32,
    /// Include each tool's `description`.
    pub description: bool,
    /// Include each tool's `inputSchema` (its parameters' JSON Schema).
    pub input_schema: bool,
    /// Include each tool's `outputSchema`, when it has one.
    pub output_schema: bool,
    /// Include each tool's `annotations` (behavior hints), when present.
    pub annotations: bool,
}

#[tool_router(router = listing_tools, vis = "pub")]
impl QuasWexExortMcp {
    #[tool(
        name = "list_tools",
        description = "List the tools in your arsenal, paginated by `offset` and `count`. With `description`/`input_schema`/`output_schema`/`annotations` all false, returns a JSON array of tool names; set any to true to instead return a JSON array of objects carrying `name` plus the selected native fields."
    )]
    async fn list_tools_tool(
        &self,
        Parameters(args): Parameters<ListToolsArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let response_id = common::required_header(&ctx.extensions, RESPONSE_ID_HEADER)?;
        let result = match common::list_tools_full(&self.context.executor, &response_id).await {
            Ok(tools) => {
                let page = tools
                    .into_iter()
                    .skip(args.offset as usize)
                    .take(args.count as usize);
                let body = if args.description
                    || args.input_schema
                    || args.output_schema
                    || args.annotations
                {
                    let items: Vec<Value> = page.map(|t| project(&t, &args)).collect();
                    serde_json::to_string(&items)
                } else {
                    let names: Vec<String> = page.map(|t| t.name).collect();
                    serde_json::to_string(&names)
                };
                CallToolResult::success(vec![Content::text(
                    body.unwrap_or_else(|_| "[]".to_string()),
                )])
            }
            Err(e) => CallToolResult::error(vec![Content::text(e)]),
        };
        Ok(result)
    }
}

/// Project a native `Tool` to `name` plus the flag-selected native fields,
/// omitting any selected field that is absent on the tool.
fn project(tool: &objectiveai_sdk::mcp::tool::Tool, args: &ListToolsArgs) -> Value {
    let mut obj = Map::new();
    obj.insert("name".into(), Value::String(tool.name.clone()));
    if args.description {
        if let Some(d) = &tool.description {
            obj.insert("description".into(), Value::String(d.clone()));
        }
    }
    if args.input_schema {
        if let Ok(v) = serde_json::to_value(&tool.input_schema) {
            obj.insert("inputSchema".into(), v);
        }
    }
    if args.output_schema {
        if let Some(s) = &tool.output_schema {
            if let Ok(v) = serde_json::to_value(s) {
                obj.insert("outputSchema".into(), v);
            }
        }
    }
    if args.annotations {
        if let Some(a) = &tool.annotations {
            if let Ok(v) = serde_json::to_value(a) {
                obj.insert("annotations".into(), v);
            }
        }
    }
    Value::Object(obj)
}
