//! `list_tools` integration tests — paginated arsenal listing with optional
//! per-field projection.

mod common;

use common::{Agent, Host, spawn_echo};
use serde_json::{json, Value};

/// Args with every field flag off (the names-only path).
fn names_args(offset: u32, count: u32) -> Value {
    json!({
        "offset": offset,
        "count": count,
        "description": false,
        "input_schema": false,
        "output_schema": false,
        "annotations": false,
    })
}

/// `list_tools` returns the arsenal's tool names, and `offset`/`count` paginate
/// them: `page(0,2)` ++ `page(2,N)` reconstructs `page(0,N)`, independent of the
/// proxy's exact ordering.
#[tokio::test(flavor = "multi_thread")]
async fn list_tools_paginates_names() {
    let host = Host::new("list_tools_paginates_names");
    let echo = spawn_echo().await;
    let agent = Agent::new()
        .mcp_server(echo.url())
        .call("list_tools", names_args(0, 100))
        .call("list_tools", names_args(0, 2))
        .call("list_tools", names_args(2, 100));

    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let texts = host.tool_texts(&aih).await;
    assert!(texts.len() >= 3, "expected 3 list_tools results, got {texts:?}");

    let parse = |s: &str| -> Vec<String> {
        serde_json::from_str(s).unwrap_or_else(|e| panic!("not a JSON array of names: {e}\n{s}"))
    };
    let full = parse(&texts[0]);
    let a = parse(&texts[1]);
    let b = parse(&texts[2]);

    // The echo server's tools appear in the full listing (names only).
    assert!(full.contains(&"test_echo".to_string()), "missing test_echo: {full:?}");
    assert!(full.contains(&"test_add".to_string()), "missing test_add: {full:?}");

    // `count` caps the page; `offset`+`count` slice cleanly and reconstruct the whole.
    assert!(a.len() <= 2, "count=2 should cap the page: {a:?}");
    let mut reconstructed = a.clone();
    reconstructed.extend(b.clone());
    assert_eq!(
        reconstructed, full,
        "page(0,2) ++ page(2,100) should equal page(0,100)"
    );
}

/// `list_tools` is ungated: it works even with `tasks` and `multi` both off, and
/// the listing reflects gating (the disabled tools are absent, but `list_tools`
/// itself is present).
#[tokio::test(flavor = "multi_thread")]
async fn list_tools_visible_in_any_mode() {
    let host = Host::new("list_tools_visible_in_any_mode");
    let echo = spawn_echo().await;
    let agent = Agent::new()
        .tasks(false)
        .multi(false)
        .mcp_server(echo.url())
        .call("list_tools", names_args(0, 100));

    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let texts = host.tool_texts(&aih).await;
    assert!(!texts.is_empty(), "list_tools produced no result (was it gated?)");

    let names: Vec<String> = serde_json::from_str(&texts[0]).expect("JSON array of names");
    // Ungated → it ran and lists the echo tools and itself...
    assert!(names.contains(&"test_echo".to_string()), "missing test_echo: {names:?}");
    assert!(
        names.contains(&"quas-wex-exort_list_tools".to_string()),
        "list_tools should list itself: {names:?}"
    );
    // ...while the gated tools stay hidden with their flags off.
    assert!(
        !names.contains(&"quas-wex-exort_multi_call".to_string()),
        "multi_call should be hidden when multi=false: {names:?}"
    );
}

/// With `input_schema: true`, items become objects carrying `name` + the native
/// `inputSchema` (always present), and nothing else unrequested.
#[tokio::test(flavor = "multi_thread")]
async fn list_tools_includes_input_schema() {
    let host = Host::new("list_tools_includes_input_schema");
    let echo = spawn_echo().await;
    let agent = Agent::new().mcp_server(echo.url()).call(
        "list_tools",
        json!({
            "offset": 0, "count": 100,
            "description": false, "input_schema": true,
            "output_schema": false, "annotations": false,
        }),
    );

    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let texts = host.tool_texts(&aih).await;
    let items: Vec<Value> = serde_json::from_str(&texts[0]).expect("JSON array of objects");
    assert!(!items.is_empty(), "no tools listed");

    for item in &items {
        let obj = item.as_object().expect("each item is an object");
        assert!(obj.contains_key("name"), "missing name: {item}");
        let schema = obj.get("inputSchema").expect("inputSchema present");
        assert_eq!(schema.get("type"), Some(&json!("object")), "schema: {schema}");
        assert!(!obj.contains_key("description"), "unrequested description present: {item}");
        assert!(!obj.contains_key("outputSchema"), "unrequested outputSchema present: {item}");
    }
}

/// With `description: true`, items carry the native `description` for tools that
/// have one (the echo fixture's tools do).
#[tokio::test(flavor = "multi_thread")]
async fn list_tools_includes_description() {
    let host = Host::new("list_tools_includes_description");
    let echo = spawn_echo().await;
    let agent = Agent::new().mcp_server(echo.url()).call(
        "list_tools",
        json!({
            "offset": 0, "count": 100,
            "description": true, "input_schema": false,
            "output_schema": false, "annotations": false,
        }),
    );

    let aih = host.spawn_detached(&agent).await;
    host.agents_wait(&aih).await;
    let texts = host.tool_texts(&aih).await;
    let items: Vec<Value> = serde_json::from_str(&texts[0]).expect("JSON array of objects");

    let echo_item = items
        .iter()
        .find(|i| i.get("name") == Some(&json!("test_echo")))
        .expect("test_echo listed");
    assert_eq!(
        echo_item.get("description"),
        Some(&json!("Echo the input string back verbatim.")),
        "test_echo description: {echo_item}"
    );
    // inputSchema wasn't requested, so it's absent.
    assert!(
        echo_item.get("inputSchema").is_none(),
        "unrequested inputSchema present: {echo_item}"
    );
}
