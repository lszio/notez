mod common;

use common::{initialize_request, run_session};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_sync_push_pull_and_conflicts() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_a = temp_dir.path().join("space_a");
    let space_b = temp_dir.path().join("space_b");
    let shared = temp_dir.path().join("shared");

    fs::create_dir_all(&space_a).unwrap();
    fs::create_dir_all(&space_b).unwrap();
    fs::create_dir_all(&shared).unwrap();

    let doc = space_a.join("note.org");
    fs::write(
        &doc,
        "#+title: MCP Sync Note\n#+ID: 01J00000000000000000000045\n",
    )
    .unwrap();

    // Engine A is bound to space_a (composition-root wiring via the
    // shared helper) and pushes into the shared folder.
    let mut service_a = common::make_mcp_engine(&space_a);
    service_a.scan_native().unwrap();

    let push_responses = run_session(
        service_a,
        vec![
            initialize_request(1).to_string(),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "sync_push", "arguments": {"actor": "actor_a", "folder": shared.to_string_lossy()}}}).to_string(),
        ],
    )
    .await;
    assert_eq!(push_responses.len(), 2);
    let push = &push_responses[1];
    assert_eq!(
        push["result"]["isError"], false,
        "sync_push should succeed: {push:?}"
    );

    // Engine B is bound to a second space and pulls from the same
    // shared folder, then lists conflicts (none expected).
    let service_b = common::make_mcp_engine(&space_b);
    let pull_responses = run_session(
        service_b,
        vec![
            initialize_request(1).to_string(),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "sync_pull", "arguments": {"actor": "actor_b", "folder": shared.to_string_lossy()}}}).to_string(),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "sync_conflicts", "arguments": {}}}).to_string(),
        ],
    )
    .await;
    assert_eq!(pull_responses.len(), 3);

    let by_id: HashMap<u64, &Value> = pull_responses
        .iter()
        .map(|v| (v["id"].as_u64().expect("id is u64"), v))
        .collect();

    let init = by_id.get(&1).expect("initialize");
    assert!(init["result"].is_object());

    let pull = by_id.get(&2).expect("sync_pull");
    assert_eq!(
        pull["result"]["isError"], false,
        "sync_pull should succeed: {pull:?}"
    );
    assert!(
        space_b.join("note.org").exists(),
        "pulled file must land in space_b"
    );

    let conflicts = by_id.get(&3).expect("sync_conflicts");
    assert_eq!(
        conflicts["result"]["isError"], false,
        "sync_conflicts should succeed: {conflicts:?}"
    );
}
