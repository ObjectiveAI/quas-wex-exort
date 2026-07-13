//! Shared helpers used by more than one toolset: invoking a tool through the
//! ObjectiveAI CLI, messaging the agent, generating ids, and extracting
//! required request headers.

use objectiveai_sdk::cli::command::agents::mcp::tools::call as tools_call;
use objectiveai_sdk::cli::command::agents::mcp::tools::list as tools_list;
use objectiveai_sdk::cli::command::agents::message as agents_message;
use objectiveai_sdk::cli::command::agents::selector::AgentSelector;
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

/// Run `agents mcp tools call` for `tool`+`arguments` scoped to `response_id`,
/// returning the native MCP tool result, or a raw executor error as a string.
///
/// When the call fails because the tool name isn't in the agent's arsenal, the
/// error string is enriched with a `did you mean <name>?` hint pointing at the
/// closest available tool (see [`enrich_tool_not_found`]).
pub async fn call_tool(
    executor: &PluginExecutor,
    response_id: &str,
    tool: &str,
    arguments: serde_json::Value,
) -> Result<objectiveai_sdk::mcp::tool::CallToolResult, String> {
    let params: objectiveai_sdk::mcp::tool::CallToolRequestParams =
        serde_json::from_value(serde_json::json!({ "name": tool, "arguments": normalize_arguments(arguments) }))
            .map_err(|e| format!("invalid tool arguments: {e}"))?;
    let request = tools_call::Request {
        path_type: tools_call::Path::AgentsMcpToolsCall,
        response_id: response_id.to_string(),
        params,
        base: Default::default(),
    };
    match tools_call::execute(executor, request, None).await {
        Ok(result) => Ok(result),
        Err(e) => Err(enrich_tool_not_found(executor, response_id, tool, e.to_string()).await),
    }
}

/// Tool arguments must be a JSON object (map), but agents commonly double-encode
/// them as a JSON *string* (a tool-calling quirk; the ObjectiveAI host likewise
/// passes args as strings in places). If `arguments` is a string that parses as
/// JSON, unwrap it to the parsed value; otherwise return it unchanged (a genuine
/// type error then surfaces downstream).
fn normalize_arguments(arguments: serde_json::Value) -> serde_json::Value {
    match arguments {
        serde_json::Value::String(s) => serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s)),
        other => other,
    }
}

/// If `error` is a "tool not found" failure, append a `did you mean <name>?`
/// hint for the closest tool in the arsenal. Best-effort: returns `error`
/// unchanged when it isn't a not-found error, the arsenal can't be listed, or
/// it is empty.
///
/// Both not-found shapes are covered: the proxy's own `method not found: tool:
/// <name>` (the server prefix matched no upstream) and an upstream MCP server's
/// `tool not found` (the prefix matched but the bare tool name didn't).
async fn enrich_tool_not_found(
    executor: &PluginExecutor,
    response_id: &str,
    tool: &str,
    error: String,
) -> String {
    if !(error.contains("tool not found") || error.contains("method not found")) {
        return error;
    }
    let Ok(names) = list_tool_names(executor, response_id).await else {
        return error;
    };
    match closest_tool(tool, &names) {
        Some(best) => format!("{error} — did you mean `{best}`?"),
        None => error,
    }
}

/// List the full native tools in the agent's arsenal (`agents mcp tools list`),
/// scoped to `response_id`. Tool names are the aggregated `<server>_<tool>` form.
pub(crate) async fn list_tools_full(
    executor: &PluginExecutor,
    response_id: &str,
) -> Result<Vec<objectiveai_sdk::mcp::tool::Tool>, String> {
    let request = tools_list::Request {
        path_type: tools_list::Path::AgentsMcpToolsList,
        response_id: response_id.to_string(),
        params: objectiveai_sdk::mcp::tool::ListToolsRequest { cursor: None },
        name: None,
        base: Default::default(),
    };
    let result = tools_list::execute(executor, request, None)
        .await
        .map_err(|e| e.to_string())?;
    Ok(result.tools)
}

/// Just the tool names from [`list_tools_full`].
pub(crate) async fn list_tool_names(
    executor: &PluginExecutor,
    response_id: &str,
) -> Result<Vec<String>, String> {
    list_tools_full(executor, response_id)
        .await
        .map(|tools| tools.into_iter().map(|t| t.name).collect())
}

/// Generate an id: a random `u64` in base62, zero-padded to a constant width
/// (11 chars covers all of `u64`).
pub(crate) fn gen_id() -> String {
    format!("{:0>11}", base62::encode(rand::random::<u64>()))
}

/// Send `text` to `aih` via `agents message`. The AIH is split on its last `/`
/// into a lineage prefix + leaf instance, per the SDK's `AgentSelector::Instance`.
pub(crate) async fn send_message(
    executor: &PluginExecutor,
    aih: &str,
    text: &str,
) -> Result<(), String> {
    let (parent, instance) = aih
        .rsplit_once('/')
        .map(|(p, i)| (Some(p.to_string()), i.to_string()))
        .unwrap_or((None, aih.to_string()));
    let request = agents_message::Request {
        path_type: agents_message::Path::AgentsMessage,
        agent: AgentSelector::Instance {
            parent_agent_instance_hierarchy: parent,
            agent_instance: instance,
        },
        message: agents_message::RequestMessage::Simple(text.to_string()),
        dangerous_advanced: None,
        base: Default::default(),
    };
    agents_message::execute(executor, request, None)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The arsenal name closest to `target` by case-insensitive Levenshtein
/// distance, or `None` if `names` is empty. Ties resolve to the first in order.
fn closest_tool<'a>(target: &str, names: &'a [String]) -> Option<&'a str> {
    let target = target.to_ascii_lowercase();
    names
        .iter()
        .min_by_key(|name| levenshtein(&target, &name.to_ascii_lowercase()))
        .map(String::as_str)
}

/// Levenshtein edit distance between two strings (single-row DP).
fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levenshtein_basics() {
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!(levenshtein("abc", "abc"), 0);
        assert_eq!(levenshtein("abc", "abd"), 1);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("test_ecko", "test_echo"), 1); // single k→h swap
        assert_eq!(levenshtein("test_ec", "test_echo"), 2); // two trailing inserts
    }

    #[test]
    fn normalize_arguments_unwraps_double_encoded() {
        use serde_json::json;
        // A double-encoded object string is parsed back into an object.
        assert_eq!(
            normalize_arguments(json!("{\"input\":\"hi\"}")),
            json!({ "input": "hi" })
        );
        // A real object passes through untouched.
        assert_eq!(normalize_arguments(json!({ "a": 1 })), json!({ "a": 1 }));
        // A non-JSON string is left as-is (a genuine error surfaces downstream).
        assert_eq!(normalize_arguments(json!("not json")), json!("not json"));
        // `null` (no arguments) passes through.
        assert_eq!(normalize_arguments(json!(null)), json!(null));
    }

    #[test]
    fn closest_picks_nearest_and_handles_empty() {
        let names = vec![
            "test_echo".to_string(),
            "test_add".to_string(),
            "quas-wex-exort_multi_call".to_string(),
        ];
        assert_eq!(closest_tool("test_ecko", &names), Some("test_echo"));
        assert_eq!(closest_tool("test_ad", &names), Some("test_add"));
        assert_eq!(closest_tool("anything", &[]), None);
    }
}
