//! Completion-nudge integration test.
//!
//! When a task finishes without having been waited on, it sends the agent
//! instance hierarchy a `<quas-wex-exort>` completion message via `agents
//! message`, which re-runs the agent. The deterministic barrier (no polling) is
//! a second `agents wait`: the first wait covers the create turn; the message
//! re-runs the agent, and the second wait covers that re-run — by which point
//! the nudge has been enqueued.

mod common;

use common::{Agent, Host, spawn_echo, test_tool};
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn create_unwaited_sends_completion_nudge() {
    let host = Host::new("create_unwaited_sends_completion_nudge");
    let echo = spawn_echo().await;
    let agent = Agent::new().mcp_server(echo.url()).call(
        "create",
        json!({ "tool": test_tool("echo"), "arguments": { "input": "hi" } }),
    );

    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await; // the create turn
    host.agents_wait(&aih).await; // the nudge-triggered re-run

    // The completion nudge was delivered to the agent, in the documented shape.
    let msgs = host.message_texts().await;
    assert!(
        msgs.iter().any(|m| m.starts_with("<quas-wex-exort>")
            && m.contains("has completed")
            && m.trim_end().ends_with("</quas-wex-exort>")),
        "expected a `<quas-wex-exort> ... has completed ...` nudge, got: {msgs:?}",
    );
}
