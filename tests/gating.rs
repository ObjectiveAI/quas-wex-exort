//! Toolset-gating integration tests (the `x-objectiveai-arguments` flags).
//!
//! A gated-off tool is hidden from the agent's surface, so when the mock emits
//! a call to it the host can't route it and no `tool_response` is produced —
//! the inverse of the enabled case (e.g. `multi_call_two_echoes`, which does
//! produce a `tool_response`).

mod common;

use common::{Agent, Host, spawn_echo, test_tool};
use serde_json::json;

fn called(logs: &[serde_json::Value], name: &str) -> bool {
    logs.iter().any(|b| b.to_string().contains(name))
}

fn executed(logs: &[serde_json::Value]) -> bool {
    logs.iter()
        .any(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_response"))
}

/// With `multi` disabled, `multi_call` is hidden — the call never executes.
#[tokio::test(flavor = "multi_thread")]
async fn multi_disabled() {
    let host = Host::new("multi_disabled");
    let echo = spawn_echo().await;
    let agent = Agent::new().multi(false).mcp_server(echo.url()).call(
        "multi_call",
        json!({ "calls": [{ "tool": test_tool("echo"), "arguments": { "input": "x" } }] }),
    );
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let logs = host.logs(&aih).await;
    assert!(
        called(&logs, "quas-wex-exort_multi_call"),
        "assistant should have emitted the multi_call"
    );
    assert!(
        !executed(&logs),
        "multi_call should be hidden when multi=false (no tool_response)"
    );
}

/// With `tasks` disabled, the task tools are hidden — `wait` never executes.
#[tokio::test(flavor = "multi_thread")]
async fn tasks_disabled() {
    let host = Host::new("tasks_disabled");
    let agent = Agent::new()
        .tasks(false)
        .call("wait", json!({ "task_id": "whatever" }));
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let logs = host.logs(&aih).await;
    assert!(
        called(&logs, "quas-wex-exort_wait"),
        "assistant should have emitted the wait call"
    );
    assert!(
        !executed(&logs),
        "task tools should be hidden when tasks=false (no tool_response)"
    );
}

/// With `loops` disabled, the loop tools are hidden — `begin_loop` never
/// executes.
#[tokio::test(flavor = "multi_thread")]
async fn loops_disabled() {
    let host = Host::new("loops_disabled");
    let agent = Agent::new().loops(false).call(
        "begin_loop",
        json!({ "interval_seconds": 3600, "message": "x" }),
    );
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let logs = host.logs(&aih).await;
    assert!(
        called(&logs, "quas-wex-exort_begin_loop"),
        "assistant should have emitted the begin_loop call"
    );
    assert!(
        !executed(&logs),
        "loop tools should be hidden when loops=false (no tool_response)"
    );
}
