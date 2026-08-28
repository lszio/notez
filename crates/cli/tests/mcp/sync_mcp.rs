mod common;

use notez_core::application::Engine;
use common::{initialize_request, run_session};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use notez_core::storage::SqliteProjection;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_sync_push_pull_and_conflicts() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_a = temp_dir.path().join("space_a");
    let space_b = temp_dir.path().join("space_b");
    let shared = temp_dir.path().join("shared");

    fs::create_dir_all(&space_a).unwrap();
    fs::create_dir_all(&space_b).unwrap();
    fs::create_dir_all(&shared).unwrap();

    fs::create_dir_all(space_a.join(".notez")).unwrap();
    fs::create_dir_all(space_b.join(".notez")).unwrap();

    let doc = space_a.join("note.org");
    fs::write(
        &doc,
        "#+title: MCP Sync Note\n#+ID: 01J00000000000000000000045\n",
    )
    .unwrap();

    let store_a = SqliteProjection::open(&space_a.join(".notez/index.sqlite")).unwrap();
    let mut service_a = Engine::new(store_a);

    let requests = vec![
        initialize_request(1).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "sync_push", "arguments": {"space": space_a.to_string_lossy(), "actor": "actor_a", "folder": shared.to_string_lossy()}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "sync_pull", "arguments": {"space": space_b.to_string_lossy(), "actor": "actor_b", "folder": shared.to_string_lossy()}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "sync_conflicts", "arguments": {"space": space_b.to_string_lossy()}}}).to_string(),
    ];

    let responses = run_session(service_a, requests).await;
    assert_eq!(responses.len(), 4);

    let by_id: HashMap<u64, &Value> = responses
        .iter()
        .map(|v| (v["id"].as_u64().expect("id is u64"), v))
        .collect();

    let init = by_id.get(&1).expect("initialize");
    assert!(init["result"].is_object());

    let push = by_id.get(&2).expect("sync_push");
    assert_eq!(
        push["result"]["isError"], false,
        "sync_push should succeed: {push:?}"
    );

    let pull = by_id.get(&3).expect("sync_pull");
    assert_eq!(
        pull["result"]["isError"], false,
        "sync_pull should succeed: {pull:?}"
    );

    let conflicts = by_id.get(&4).expect("sync_conflicts");
    assert_eq!(
        conflicts["result"]["isError"], false,
        "sync_conflicts should succeed: {conflicts:?}"
    );
}