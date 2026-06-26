//! `multi_call` integration tests — the deterministic "invoke other MCP tools
//! concurrently and get results back" path.

mod common;

use common::{Agent, Host, spawn_echo, test_tool};
use serde_json::json;

/// Two echo calls run via multi_call are joined into one response, each segment
/// prefixed with `[result N (tool)]` and carrying that call's echoed input.
#[tokio::test(flavor = "multi_thread")]
async fn multi_call_two_echoes() {
    let host = Host::new("multi_call_two_echoes");
    let echo = spawn_echo().await;
    let agent = Agent::new().mcp_server(echo.url()).call(
        "multi_call",
        json!({
            "calls": [
                { "tool": test_tool("echo"), "arguments": { "input": "alpha" } },
                { "tool": test_tool("echo"), "arguments": { "input": "beta" } },
            ],
        }),
    );

    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let joined = host.tool_texts(&aih).await.join("");

    // Both segment prefixes + both echoes, in input order.
    let r0 = joined.find("[result 0 (test_echo)]").expect("missing result 0");
    let a = joined.find("alpha").expect("missing alpha");
    let r1 = joined.find("[result 1 (test_echo)]").expect("missing result 1");
    let b = joined.find("beta").expect("missing beta");
    assert!(
        r0 < a && a < r1 && r1 < b,
        "segments out of order:\n{joined}"
    );
}

/// A single call is accepted.
#[tokio::test(flavor = "multi_thread")]
async fn multi_call_single() {
    let host = Host::new("multi_call_single");
    let echo = spawn_echo().await;
    let agent = Agent::new().mcp_server(echo.url()).call(
        "multi_call",
        json!({ "calls": [{ "tool": test_tool("echo"), "arguments": { "input": "solo" } }] }),
    );
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let joined = host.tool_texts(&aih).await.join("");
    assert!(
        joined.contains("[result 0 (test_echo)]") && joined.contains("solo"),
        "missing the single echo:\n{joined}"
    );
}

/// When every call fails, both error segments are still present.
#[tokio::test(flavor = "multi_thread")]
async fn multi_call_all_error() {
    let host = Host::new("multi_call_all_error");
    let echo = spawn_echo().await;
    let agent = Agent::new().mcp_server(echo.url()).call(
        "multi_call",
        json!({
            "calls": [
                { "tool": test_tool("nope1"), "arguments": {} },
                { "tool": test_tool("nope2"), "arguments": {} },
            ],
        }),
    );
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let joined = host.tool_texts(&aih).await.join("");
    assert!(
        joined.contains("[result 0 (test_nope1)]") && joined.contains("[result 1 (test_nope2)]"),
        "both error segments should be present:\n{joined}"
    );
}

/// A misspelled tool name yields a `did you mean <closest>?` suggestion. This
/// exercises the shared `call_tool` not-found enrichment, so it covers the task
/// toolset's identical path too.
#[tokio::test(flavor = "multi_thread")]
async fn multi_call_did_you_mean() {
    let host = Host::new("multi_call_did_you_mean");
    let echo = spawn_echo().await;
    // `test_ecko` is one edit from the real `test_echo` (and `test_add` is
    // farther), so the suggestion is unambiguous.
    let agent = Agent::new().mcp_server(echo.url()).call(
        "multi_call",
        json!({ "calls": [{ "tool": test_tool("ecko"), "arguments": { "input": "x" } }] }),
    );
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let joined = host.tool_texts(&aih).await.join("");
    assert!(
        joined.contains("did you mean `test_echo`?"),
        "expected a did-you-mean suggestion:\n{joined}"
    );
}

/// Arguments double-encoded as a JSON string (a common agent quirk) are parsed
/// rather than rejected. Shared `call_tool` path, so it covers the task toolset.
#[tokio::test(flavor = "multi_thread")]
async fn multi_call_stringified_arguments() {
    let host = Host::new("multi_call_stringified_arguments");
    let echo = spawn_echo().await;
    // `arguments` is a JSON *string*, not an object — the double-encoded shape.
    let agent = Agent::new().mcp_server(echo.url()).call(
        "multi_call",
        json!({ "calls": [{ "tool": test_tool("echo"), "arguments": "{\"input\":\"boxed\"}" }] }),
    );
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let joined = host.tool_texts(&aih).await.join("");
    assert!(
        joined.contains("boxed"),
        "double-encoded args should be parsed and echoed:\n{joined}"
    );
}

/// An empty `calls` list is rejected.
#[tokio::test(flavor = "multi_thread")]
async fn multi_call_empty_is_error() {
    let host = Host::new("multi_call_empty_is_error");
    let echo = spawn_echo().await;
    let agent = Agent::new()
        .mcp_server(echo.url())
        .call("multi_call", json!({ "calls": [] }));
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let joined = host.tool_texts(&aih).await.join("");
    assert!(
        joined.contains("at least one call"),
        "expected empty-calls error:\n{joined}"
    );
}

/// One good + one bad call: both segments present (the bad one carries an
/// error), and the result is still non-error overall (one succeeded).
#[tokio::test(flavor = "multi_thread")]
async fn multi_call_partial_failure() {
    let host = Host::new("multi_call_partial_failure");
    let echo = spawn_echo().await;
    let agent = Agent::new().mcp_server(echo.url()).call(
        "multi_call",
        json!({
            "calls": [
                { "tool": test_tool("echo"), "arguments": { "input": "good" } },
                { "tool": test_tool("nonexistent"), "arguments": {} },
            ],
        }),
    );
    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let joined = host.tool_texts(&aih).await.join("");
    assert!(joined.contains("good"), "missing the good echo:\n{joined}");
    assert!(
        joined.contains("[result 0 (test_echo)]"),
        "missing result 0:\n{joined}"
    );
    assert!(
        joined.contains("[result 1 (test_nonexistent)]"),
        "missing result 1:\n{joined}"
    );
}
