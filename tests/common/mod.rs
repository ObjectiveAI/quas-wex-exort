//! Integration-test harness for quas-wex-exort.
//!
//! Every test drives the prebuilt `objectiveai` host in the repo's
//! `.objectiveai/` (staged by `build.sh`) through the SDK [`BinaryExecutor`].
//! A test gets an isolated state by setting `OBJECTIVEAI_STATE` to its own name;
//! the host bootstraps a fresh per-state postgres on first command. **No prep or
//! spawning is needed in advance — the command executor launches the plugin
//! daemon / test MCP server on demand.**
//!
//! quas-wex-exort's tools invoke OTHER MCP tools (the agent's arsenal) via
//! `agents tools call`, so the mock agent's arsenal contains BOTH quas-wex-exort
//! (its task/multi_call tools, enabled via the `tasks`/`multi` arguments which
//! the host bridges to the `x-objectiveai-arguments` header) and the
//! `test-mcp-server` fixture (the `test_echo` / `test_add` tools quas-wex-exort
//! actually calls).
//!
//! Note: the mock `calls` script is fixed at spawn, so a task's runtime id
//! (returned by `create`) can't be threaded into a later `wait`/`cancel`. The
//! deterministic result path is therefore `multi_call` (inline tool calls).
#![allow(dead_code, unused_imports)]

mod echo;
pub use echo::spawn as spawn_echo;

use std::path::PathBuf;

use futures::StreamExt;
use objectiveai_sdk::agent::InlineAgentBaseWithFallbacksOrRemoteCommitOptional;
use objectiveai_sdk::cli::command::CommandExecutor;
use objectiveai_sdk::cli::command::agents::logs::list as logs_list;
use objectiveai_sdk::cli::command::agents::message::RequestMessage;
use objectiveai_sdk::cli::command::db::query as db_query;
use objectiveai_sdk::cli::command::agents::selector::{AgentRef, AgentSelector};
use objectiveai_sdk::cli::command::agents::spawn as agents_spawn;
use objectiveai_sdk::cli::command::agents::wait as agents_wait;
use objectiveai_sdk::cli::command::binary::{BinaryExecutor, Error as ExecError};
use serde_json::{Value, json};

// Plugin coordinate (matches the repo-root objectiveai.json, staged by build.sh).
const OWNER: &str = "ObjectiveAI";
const NAME: &str = "quas-wex-exort";
const VERSION: &str = "0.1.0";
const MCP_SERVER: &str = "quas-wex-exort";
/// The aggregated tool prefix = the MCP server's `serverInfo.name`
/// (CARGO_PKG_NAME). Tools surface as `quas-wex-exort_<tool>`.
const PREFIX: &str = "quas-wex-exort";

/// The aggregated tool-name prefix for the in-process echo server (its
/// `serverInfo.name`). Tools surface as `test_echo` / `test_add`.
const TEST_PREFIX: &str = "test";

/// An aggregated tool name on the echo server (e.g. `test_echo`).
pub fn test_tool(tool: &str) -> String {
    format!("{TEST_PREFIX}_{tool}")
}

fn objectiveai_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".objectiveai")
}

// ──────────────────────────── mock-agent builder ───────────────────────────

/// Builds an inline mock agent whose arsenal is quas-wex-exort + the test
/// fixture, running a deterministic `calls` script.
pub struct Agent {
    calls: Vec<Value>,
    tasks: bool,
    multi: bool,
    mcp_servers: Vec<String>,
    plugin: bool,
}

impl Agent {
    pub fn new() -> Self {
        Self {
            calls: Vec::new(),
            tasks: true,
            multi: true,
            mcp_servers: Vec::new(),
            plugin: true,
        }
    }

    /// Exclude the quas-wex-exort plugin from the arsenal (diagnostic).
    pub fn no_plugin(mut self) -> Self {
        self.plugin = false;
        self
    }

    /// Add a raw MCP server (by URL) to the agent's arsenal — e.g. the
    /// in-process echo server from [`spawn_echo`].
    pub fn mcp_server(mut self, url: &str) -> Self {
        self.mcp_servers.push(url.to_string());
        self
    }

    /// Toggle the `tasks` toolset flag (the per-server argument the host bridges
    /// into the `x-objectiveai-arguments` header).
    pub fn tasks(mut self, on: bool) -> Self {
        self.tasks = on;
        self
    }

    /// Toggle the `multi` toolset flag.
    pub fn multi(mut self, on: bool) -> Self {
        self.multi = on;
        self
    }

    /// Append a turn that calls a quas-wex-exort tool (`create`/`list`/`wait`/
    /// `cancel`/`multi_call`) with `args`.
    pub fn call(mut self, tool: &str, args: Value) -> Self {
        self.calls.push(json!({
            "tool_calls": [{ "name": format!("{PREFIX}_{tool}"), "arguments": args.to_string() }],
            "content": "",
        }));
        self
    }

    /// Append a turn that calls an arbitrary aggregated tool name verbatim
    /// (e.g. `test_echo`).
    pub fn call_raw(mut self, name: &str, args: Value) -> Self {
        self.calls.push(json!({
            "tool_calls": [{ "name": name, "arguments": args.to_string() }],
            "content": "",
        }));
        self
    }

    fn definition(&self) -> Value {
        let mut calls = self.calls.clone();
        // A trailing content-only turn so the completion terminates cleanly.
        calls.push(json!({ "tool_calls": [], "content": "done" }));
        let raw_servers: Vec<Value> = self
            .mcp_servers
            .iter()
            .map(|url| json!({ "url": url, "authorization": false }))
            .collect();
        let plugins = if self.plugin {
            json!([{
                "owner": OWNER, "name": NAME, "version": VERSION, "executable": false,
                "mcp_servers": [{
                    "name": MCP_SERVER,
                    "arguments": {
                        "tasks": self.tasks.to_string(),
                        "multi": self.multi.to_string(),
                        "python": "false",
                        "objectiveai": "false",
                    },
                }],
            }])
        } else {
            json!([])
        };
        json!({
            "upstream": "mock",
            "output_mode": "instruction",
            "client_objectiveai_mcp": { "plugins": plugins },
            "mcp_servers": raw_servers,
            "calls": calls,
        })
    }

    fn selector(&self) -> AgentSelector {
        let spec: InlineAgentBaseWithFallbacksOrRemoteCommitOptional =
            serde_json::from_value(self.definition()).expect("agent definition deserializes");
        AgentSelector::Ref {
            agent: AgentRef::Resolved(spec),
        }
    }
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────── the host ─────────────────────────────────

/// Drives quas-wex-exort against one isolated objectiveai state.
pub struct Host {
    executor: BinaryExecutor,
    state: String,
}

impl Host {
    /// Build a handle for a test; pass the test's own name as `state`.
    pub fn new(state: &str) -> Self {
        let dir = objectiveai_dir();
        let executor = BinaryExecutor::new(Some(dir.clone()))
            .env("OBJECTIVEAI_DIR", dir.to_string_lossy().into_owned())
            .env("OBJECTIVEAI_STATE", state)
            .kill_on_drop(true);
        Self {
            executor,
            state: state.to_string(),
        }
    }

    /// Spawn the agent (streaming, seeded) and collect the completion. Streaming
    /// returns only once the completion has finalized.
    pub async fn spawn(&self, agent: &Agent) -> SpawnResult {
        self.collect_spawn(agent.selector(), "go").await
    }

    /// Spawn non-streaming: the host re-execs the completion detached and returns
    /// the minted AIH immediately (pair with [`Host::agents_wait`]).
    pub async fn spawn_detached(&self, agent: &Agent) -> String {
        let req = agents_spawn::Request {
            path_type: agents_spawn::Path::AgentsSpawn,
            message: RequestMessage::Simple("go".to_string()),
            agent: agent.selector(),
            dangerous_advanced: Some(agents_spawn::RequestDangerousAdvanced {
                stream: Some(false),
                seed: Some(42),
            }),
            base: Default::default(),
        };
        let mut stream = self
            .executor
            .execute::<_, agents_spawn::ResponseItem>(req, None)
            .await
            .unwrap_or_else(|e| panic!("[{}] spawn_detached execute: {e}", self.state));
        while let Some(item) = stream.next().await {
            match item {
                Ok(agents_spawn::ResponseItem::Id(aih)) => return aih,
                Ok(_) => {}
                Err(ExecError::Cli(e)) => panic!("[{}] spawn_detached error: {e:?}", self.state),
                Err(other) => panic!("[{}] spawn_detached harness error: {other}", self.state),
            }
        }
        panic!("[{}] spawn_detached: no Id in response", self.state)
    }

    /// Block until agent instance `aih` has fully finalized and released its lock
    /// (`agents wait`) — the deterministic barrier, no polling.
    pub async fn agents_wait(&self, aih: &str) {
        let (parent, instance) = split_aih(aih);
        let req = agents_wait::Request {
            path_type: agents_wait::Path::AgentsWait,
            agent: AgentSelector::Instance {
                parent_agent_instance_hierarchy: Some(parent),
                agent_instance: instance,
            },
            base: Default::default(),
        };
        let mut stream = self
            .executor
            .execute::<_, agents_wait::Response>(req, None)
            .await
            .unwrap_or_else(|e| panic!("[{}] agents wait `{aih}`: {e}", self.state));
        while let Some(item) = stream.next().await {
            match item {
                Ok(_) => {}
                Err(ExecError::Cli(e)) => panic!("[{}] agents wait error: {e:?}", self.state),
                Err(other) => panic!("[{}] agents wait harness error: {other}", self.state),
            }
        }
    }

    /// Read the agent instance's logs (`agents logs list --all`) as raw JSON
    /// blocks (AssistantResponse / ToolResponse / ClientNotification / …).
    pub async fn logs(&self, aih: &str) -> Vec<Value> {
        let (parent, instance) = split_aih(aih);
        let req = logs_list::Request {
            path_type: logs_list::Path::AgentsLogsList,
            pending: false,
            targets: vec![logs_list::Target::Direct {
                parent_agent_instance_hierarchy: Some(parent),
                agent_instance: instance,
            }],
            after_id: None,
            limit: None,
            base: Default::default(),
        };
        let mut stream = self
            .executor
            .execute::<_, logs_list::ResponseItem>(req, None)
            .await
            .unwrap_or_else(|e| panic!("[{}] logs execute: {e}", self.state));
        let mut out = Vec::new();
        while let Some(item) = stream.next().await {
            match item {
                Ok(it) => out.push(serde_json::to_value(it).unwrap_or(Value::Null)),
                Err(ExecError::Cli(e)) => panic!("[{}] logs error: {e:?}", self.state),
                Err(other) => panic!("[{}] logs harness error: {other}", self.state),
            }
        }
        out
    }

    /// Run a SQL query against the per-state postgres and return its rows.
    pub async fn db_query(&self, sql: &str) -> Vec<Vec<Value>> {
        let req = db_query::Request {
            path_type: db_query::Path::DbQuery,
            query: sql.to_string(),
            base: Default::default(),
        };
        let resp: db_query::Response = self
            .executor
            .execute_one(req, None)
            .await
            .unwrap_or_else(|e| panic!("[{}] db_query: {e:?}", self.state));
        resp.rows
    }

    /// Every tool-result text the agent received, in order — read from
    /// `objectiveai.tool_response_content_text` for every response id in the
    /// agent's logs (covers the original turn + any nudge-triggered re-run).
    pub async fn tool_texts(&self, aih: &str) -> Vec<String> {
        let logs = self.logs(aih).await;
        let mut rids: Vec<String> = Vec::new();
        for b in &logs {
            if let Some(r) = b.get("response_id").and_then(Value::as_str) {
                if !rids.iter().any(|x| x == r) {
                    rids.push(r.to_string());
                }
            }
        }
        let mut out = Vec::new();
        for rid in rids {
            let sql = format!(
                "SELECT text FROM objectiveai.tool_response_content_text \
                 WHERE response_id = '{}' ORDER BY \"index\", part_index",
                rid.replace('\'', "''"),
            );
            for row in self.db_query(&sql).await {
                if let Some(s) = row.into_iter().next().and_then(|v| v.as_str().map(String::from)) {
                    out.push(s);
                }
            }
        }
        out
    }

    /// Every queued message text delivered to the agent (e.g. quas-wex-exort's
    /// completion nudges), read from `objectiveai.message_queue_texts`.
    pub async fn message_texts(&self) -> Vec<String> {
        self.db_query("SELECT text FROM objectiveai.message_queue_texts")
            .await
            .into_iter()
            .filter_map(|r| r.into_iter().next().and_then(|v| v.as_str().map(String::from)))
            .collect()
    }

    async fn collect_spawn(&self, selector: AgentSelector, message: &str) -> SpawnResult {
        let req = agents_spawn::Request {
            path_type: agents_spawn::Path::AgentsSpawn,
            message: RequestMessage::Simple(message.to_string()),
            agent: selector,
            dangerous_advanced: Some(agents_spawn::RequestDangerousAdvanced {
                stream: Some(true),
                seed: Some(42),
            }),
            base: Default::default(),
        };
        let mut stream = self
            .executor
            .execute::<_, agents_spawn::ResponseItem>(req, None)
            .await
            .unwrap_or_else(|e| panic!("[{}] spawn execute: {e}", self.state));

        let mut result = SpawnResult::default();
        while let Some(item) = stream.next().await {
            let item = match item {
                Ok(item) => item,
                Err(ExecError::Cli(e)) => {
                    result.errors.push(e);
                    continue;
                }
                Err(other) => panic!("[{}] spawn harness error: {other}", self.state),
            };
            if let agents_spawn::ResponseItem::Chunk(chunk) = &item {
                if result.agent_instance_hierarchy.is_none()
                    && !chunk.agent_instance_hierarchy.is_empty()
                {
                    result.agent_instance_hierarchy = Some(chunk.agent_instance_hierarchy.clone());
                }
                if let Some(err) = &chunk.error {
                    result.chunk_errors.push(format!("{err:?}"));
                }
            }
            result.items.push(item);
        }
        result
    }
}

/// Split an AIH `<parent>/<instance>` on its last `/`.
fn split_aih(aih: &str) -> (String, String) {
    let (parent, instance) = aih
        .rsplit_once('/')
        .unwrap_or_else(|| panic!("AIH `{aih}` must contain a '/'"));
    (parent.to_string(), instance.to_string())
}

// ──────────────────────────────── results ──────────────────────────────────

/// The collected output of one mock-agent completion.
#[derive(Default)]
pub struct SpawnResult {
    pub items: Vec<agents_spawn::ResponseItem>,
    pub errors: Vec<objectiveai_sdk::cli::Error>,
    pub chunk_errors: Vec<String>,
    pub agent_instance_hierarchy: Option<String>,
}

impl SpawnResult {
    pub fn assert_no_errors(&self) -> &Self {
        assert!(
            self.errors.is_empty(),
            "expected no host errors, got: {:?}",
            self.errors,
        );
        assert!(
            self.chunk_errors.is_empty(),
            "expected no completion errors, got: {:?}",
            self.chunk_errors,
        );
        self
    }

    /// Every streamed item serialized + joined — the surface assertions match on.
    pub fn stream_json(&self) -> String {
        self.items
            .iter()
            .map(|i| {
                serde_json::to_value(i)
                    .expect("spawn item serializes")
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn assert_contains(&self, needle: &str) -> &Self {
        let stream = self.stream_json();
        assert!(
            stream.contains(needle),
            "expected `{needle}` in completion stream:\n{stream}",
        );
        self
    }

    /// Assert the completion called the aggregated quas-wex-exort tool.
    pub fn assert_called(&self, tool: &str) -> &Self {
        self.assert_contains(&format!("{PREFIX}_{tool}"))
    }

    /// Every tool-result message body (`role:"tool"` content strings), in order.
    pub fn tool_results(&self) -> Vec<String> {
        let mut out = Vec::new();
        for item in &self.items {
            let agents_spawn::ResponseItem::Chunk(chunk) = item else {
                continue;
            };
            for msg in &chunk.messages {
                let v = serde_json::to_value(msg).unwrap_or(Value::Null);
                if v.get("role").and_then(Value::as_str) == Some("tool") {
                    if let Some(content) = v.get("content").and_then(Value::as_str) {
                        out.push(content.to_string());
                    }
                }
            }
        }
        out
    }

    pub fn aih(&self) -> &str {
        self.agent_instance_hierarchy
            .as_deref()
            .unwrap_or_else(|| panic!("no agent_instance_hierarchy in spawn stream"))
    }
}
