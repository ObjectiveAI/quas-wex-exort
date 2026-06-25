//! Task-tool integration tests.
//!
//! Note: the mock `calls` script is fixed at spawn, so a task's runtime id
//! (returned by `create`) can't be threaded into a later `wait`/`cancel`. These
//! tests therefore cover the deterministic edges; the create→complete→nudge
//! flow is exercised in `nudge.rs`.

mod common;

use common::{Agent, Host};
use serde_json::json;

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
