//! Loop-tool integration tests.
//!
//! Note: the mock `calls` script is fixed at spawn, so a loop's runtime id
//! (returned by `begin_loop`) can't be threaded into a later `end_loop` — the
//! same limitation as `tasks.rs`. These tests cover the deterministic edges
//! plus the interval delivery flow (asserted via the message queue).

mod common;

use common::{Agent, Host};
use serde_json::json;

/// `begin_loop` returns a fresh loop id (11-char base62).
#[tokio::test(flavor = "multi_thread")]
async fn begin_loop_returns_id() {
    let host = Host::new("begin_loop_returns_id");
    // Interval far beyond the test's lifetime: no tick ever fires.
    let agent = Agent::new().call(
        "begin_loop",
        json!({ "interval_seconds": 3600, "message": "hi" }),
    );
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let texts = host.tool_texts(&aih).await;
    let id = texts.first().expect("a begin_loop result");
    assert_eq!(id.len(), 11, "loop id should be 11 chars, got {id:?}");
    assert!(
        id.chars().all(|c| c.is_ascii_alphanumeric()),
        "loop id should be base62, got {id:?}"
    );
}

/// `begin_loop` with a zero interval is a tool error.
#[tokio::test(flavor = "multi_thread")]
async fn begin_loop_rejects_zero_interval() {
    let host = Host::new("begin_loop_rejects_zero_interval");
    let agent = Agent::new().call(
        "begin_loop",
        json!({ "interval_seconds": 0, "message": "hi" }),
    );
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let joined = host.tool_texts(&aih).await.join("");
    assert!(
        joined.contains("interval_seconds must be at least 1"),
        "expected the zero-interval error:\n{joined}"
    );
}

/// `end_loop` on an unknown id returns "loop not found".
#[tokio::test(flavor = "multi_thread")]
async fn end_loop_unknown_id() {
    let host = Host::new("end_loop_unknown_id");
    let agent = Agent::new().call("end_loop", json!({ "loop_id": "doesnotexist" }));
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let joined = host.tool_texts(&aih).await.join("");
    assert!(
        joined.contains("loop not found"),
        "expected 'loop not found':\n{joined}"
    );
}

/// A running loop delivers its message — verbatim, wrapped in the plugin
/// envelope with the loop id in the opener — after one interval.
///
/// The tick is wall-clock driven (not tied to run finalization), so a second
/// `agents_wait` would race the timer; instead poll the message queue with a
/// bounded deadline. The exact-match assertion covers the id attribute, the
/// verbatim message, and the envelope at once.
#[tokio::test(flavor = "multi_thread")]
async fn loop_delivers_message() {
    let host = Host::new("loop_delivers_message");
    let agent = Agent::new().call(
        "begin_loop",
        json!({ "interval_seconds": 1, "message": "tick tock" }),
    );
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let id = host
        .tool_texts(&aih)
        .await
        .first()
        .cloned()
        .expect("a begin_loop result");
    let expected = format!("<quas-wex-exort loop-id=\"{id}\">\ntick tock\n</quas-wex-exort>");

    // Bounded poll: 60 × 500ms = 30s ceiling, exits on first observation.
    let mut found = false;
    for _ in 0..60 {
        if host.message_texts().await.iter().any(|m| m == &expected) {
            found = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    assert!(found, "expected loop envelope {expected:?} in message_texts");
}
