//! Task-tool integration tests.
//!
//! Note: the mock `calls` script is fixed at spawn, so a task's runtime id
//! (returned by `create`) can't be threaded into a later `wait`/`cancel`. These
//! tests therefore cover the deterministic edges; the create→complete→nudge
//! flow is exercised in `nudge.rs`.

mod common;

use common::{Agent, Host, spawn_echo, test_tool};
use serde_json::json;

/// `create` returns a fresh task id (11-char base62).
#[tokio::test(flavor = "multi_thread")]
async fn task_create_returns_id() {
    let host = Host::new("task_create_returns_id");
    let echo = spawn_echo().await;
    let agent = Agent::new().mcp_server(echo.url()).call(
        "create",
        json!({ "tool": test_tool("echo"), "arguments": { "input": "hi" } }),
    );
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let texts = host.tool_texts(&aih).await;
    let id = texts.first().expect("a create result");
    assert_eq!(id.len(), 11, "task id should be 11 chars, got {id:?}");
    assert!(
        id.chars().all(|c| c.is_ascii_alphanumeric()),
        "task id should be base62, got {id:?}"
    );
}

/// After `create`, `list` shows the created task (same id).
#[tokio::test(flavor = "multi_thread")]
async fn task_create_then_list() {
    let host = Host::new("task_create_then_list");
    let echo = spawn_echo().await;
    let agent = Agent::new()
        .mcp_server(echo.url())
        .call(
            "create",
            json!({ "tool": test_tool("echo"), "arguments": { "input": "hi" } }),
        )
        .call("list", json!({}));
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let texts = host.tool_texts(&aih).await;
    let id = texts.first().expect("a create result").clone();
    assert!(
        texts.iter().skip(1).any(|t| t.contains(&id)),
        "list should show the created task {id}: {texts:?}"
    );
}

/// `wait` on an unknown id returns "task not found" (immediately, no blocking).
#[tokio::test(flavor = "multi_thread")]
async fn task_wait_unknown_id() {
    let host = Host::new("task_wait_unknown_id");
    let agent = Agent::new().call("wait", json!({ "task_id": "doesnotexist" }));
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let joined = host.tool_texts(&aih).await.join("");
    assert!(
        joined.contains("task not found"),
        "expected 'task not found':\n{joined}"
    );
}

/// `cancel` on an unknown id returns "task not found".
#[tokio::test(flavor = "multi_thread")]
async fn task_cancel_unknown_id() {
    let host = Host::new("task_cancel_unknown_id");
    let agent = Agent::new().call("cancel", json!({ "task_id": "doesnotexist" }));
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let joined = host.tool_texts(&aih).await.join("");
    assert!(
        joined.contains("task not found"),
        "expected 'task not found':\n{joined}"
    );
}
