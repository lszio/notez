mod common;

use common::{initialize_request, run_session};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_source_add_and_list() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source_root = temp_dir.path();

    let vault_dir = source_root.join("vault");
    fs::create_dir_all(&vault_dir).unwrap();
    fs::write(
        vault_dir.join("note.md"),
        "---\ntitle: Vault Note\nid: 01J00000000000000000000951\n---\n",
    )
    .unwrap();

    let mut service = common::make_mcp_engine(source_root);

    let requests = vec![
        initialize_request(1).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "source_add", "arguments": {"id": "vault", "kind": "obsidian", "path": vault_dir.to_string_lossy(), "read_only": true}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "source_list", "arguments": {}}}).to_string(),
    ];

    let responses = run_session(service, requests).await;
    assert_eq!(responses.len(), 3);

    let by_id: HashMap<u64, &Value> = responses
        .iter()
        .map(|v| (v["id"].as_u64().expect("id is u64"), v))
        .collect();

    let init = by_id.get(&1).expect("initialize");
    assert!(init["result"].is_object());

    let add_resp = by_id.get(&2).expect("source_add");
    assert_eq!(
        add_resp["result"]["isError"], false,
        "source_add should succeed: {add_resp:?}"
    );

    let list_resp = by_id.get(&3).expect("source_list");
    let text = list_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("source_list response shape: {list_resp:?}"));
    assert!(
        text.contains("vault"),
        "source_list response missing 'vault': {text}"
    );
}