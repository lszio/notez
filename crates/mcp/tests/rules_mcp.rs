mod common;

use application::ApplicationService;
use common::{initialize_request, run_session};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use storage::SqliteProjection;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_rules_agenda_and_task_transition() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_root = temp_dir.path();

    let file = space_root.join("tasks.org");
    fs::write(
        &file,
        "#+title: MCP Rules\n#+ID: 01J00000000000000000000800\n* TODO MCP Task\n:PROPERTIES:\n:ID: 01J00000000000000000000801\n:SCHEDULED: <2026-07-22 Wed>\n:TYPE: project\n:END:\n",
    ).unwrap();

    let store = SqliteProjection::in_memory().unwrap();
    let mut service = ApplicationService::new(store);
    service.scan_native(space_root).unwrap();

    let requests = vec![
        initialize_request(1).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "agenda", "arguments": {}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "inspect_rules", "arguments": {"ref": "heading:01J00000000000000000000801"}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "task_transition", "arguments": {"ref": "heading:01J00000000000000000000801", "to": "DONE"}}}).to_string(),
    ];

    let responses = run_session(service, requests).await;
    assert_eq!(responses.len(), 4);

    let by_id: HashMap<u64, &Value> = responses
        .iter()
        .map(|v| (v["id"].as_u64().expect("id is u64"), v))
        .collect();

    // 1. initialize
    let init = by_id.get(&1).expect("initialize response");
    assert!(init["result"].is_object());

    // 2. agenda
    let agenda = by_id.get(&2).expect("agenda response");
    let text = agenda["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("MCP Task"),
        "agenda response missing 'MCP Task': {text}"
    );

    // 3. inspect_rules
    let inspect = by_id.get(&3).expect("inspect_rules response");
    let text = inspect["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("classified_type"),
        "inspect_rules response missing 'classified_type': {text}"
    );

    // 4. task_transition
    let transition = by_id.get(&4).expect("task_transition response");
    let text = transition["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        text.contains("to_state"),
        "task_transition response missing 'to_state': {text}"
    );
}