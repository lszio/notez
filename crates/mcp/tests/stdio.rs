mod common;

use application::ApplicationService;
use common::{initialize_request, run_session};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use storage::SqliteProjection;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_stdio_jsonrpc_transcript() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_root = temp_dir.path();

    let file1 = space_root.join("file1.org");
    fs::write(
        &file1,
        "#+title: MCP Test Doc\n#+ID: 01J00000000000000000000200\n* NEXT Sync mcp heading\n:PROPERTIES:\n:ID: 01J00000000000000000000201\n:END:\n",
    ).unwrap();

    let store = SqliteProjection::in_memory().unwrap();
    let mut service = ApplicationService::new(store);
    service.scan_native(space_root).unwrap();

    let requests = vec![
        initialize_request(1).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "query", "arguments": {"kind": "heading", "title_contains": "sync"}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "resolve", "arguments": {"query": "nonexistent"}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "read", "arguments": {"ref": "heading:01J00000000000000000000201"}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "inspect", "arguments": {}}}).to_string(),
    ];

    let responses = run_session(service, requests).await;
    assert_eq!(
        responses.len(),
        6,
        "expected 6 responses, got {responses:?}"
    );

    let by_id: HashMap<u64, &Value> = responses
        .iter()
        .map(|v| (v["id"].as_u64().expect("id is u64"), v))
        .collect();

    let init = by_id.get(&1).expect("initialize response");
    assert_eq!(init["result"]["serverInfo"]["name"], "notez-mcp");

    let list = by_id.get(&2).expect("tools/list response");
    let tools = list["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|t| t["name"] == "resolve"));
    assert!(tools.iter().any(|t| t["name"] == "query"));
    assert!(tools.iter().any(|t| t["name"] == "read"));
    assert!(tools.iter().any(|t| t["name"] == "inspect"));

    let query_resp = by_id.get(&3).expect("query response");
    let text = query_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        text.contains("Sync mcp heading"),
        "query response missing heading: {text}"
    );

    let resolve_resp = by_id.get(&4).expect("resolve response");
    assert_eq!(resolve_resp["result"]["isError"], true);

    let read_resp = by_id.get(&5).expect("read response");
    let read_text = read_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        read_text.contains("Sync mcp heading"),
        "read response missing heading: {read_text}"
    );

    let inspect_resp = by_id.get(&6).expect("inspect response");
    let inspect_text = inspect_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        inspect_text.contains("items"),
        "inspect response missing 'items': {inspect_text}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_stdio_handshake_state_dropped() {
    // rmcp 1.8+ drops the `notifications/initialized` gate. Tools are
    // callable immediately after `initialize`. This test sends initialize
    // -> tools/call and verifies the tool call returns data, proving the
    // handshake state machine is gone.
    let temp_dir = tempfile::tempdir().unwrap();
    let space_root = temp_dir.path();

    let file = space_root.join("note.org");
    fs::write(
        &file,
        "#+title: Handshake Drop Test\n#+ID: 01J00000000000000000000900\n",
    )
    .unwrap();

    let store = SqliteProjection::in_memory().unwrap();
    let mut service = ApplicationService::new(store);
    service.scan_native(space_root).unwrap();

    let requests = vec![
        initialize_request(1).to_string(),
        // No notifications/initialized here — tools/call must still work.
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "query", "arguments": {}}}).to_string(),
    ];

    let responses = run_session(service, requests).await;
    assert_eq!(responses.len(), 2);

    assert_eq!(responses[0]["id"], 1);
    assert!(responses[0]["result"].is_object());

    assert_eq!(responses[1]["id"], 2);
    let text = responses[1]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        text.contains("Handshake Drop Test"),
        "tool call failed without notifications/initialized; text={text}"
    );
}