mod common;

use application::ApplicationService;
use common::{initialize_request, run_session};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use storage::SqliteProjection;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_community_create_derive_and_export_skill() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_root = temp_dir.path();

    let doc = space_root.join("note.org");
    fs::write(
        &doc,
        "#+title: MCP Dev Note\n#+ID: 01J00000000000000000000065\n* NEXT MCP Sync\n:PROPERTIES:\n:ID: 01J00000000000000000000066\n:END:\n",
    )
    .unwrap();

    let store = SqliteProjection::in_memory().unwrap();
    let mut service = ApplicationService::new(store);
    service.scan_native(space_root).unwrap();

    let skill_out = space_root.join("mcp_skill");

    let requests = vec![
        initialize_request(1).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "community_create", "arguments": {"space": space_root.to_string_lossy(), "id": "mcp_comm", "name": "MCP Comm", "title_contains": "Sync"}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "derive_artifact", "arguments": {"space": space_root.to_string_lossy(), "community": "mcp_comm", "recipe": "summary"}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "export_skill", "arguments": {"space": space_root.to_string_lossy(), "community": "mcp_comm", "description": "MCP Exported Skill", "out": skill_out.to_string_lossy()}}}).to_string(),
    ];

    let responses = run_session(service, requests).await;
    assert_eq!(responses.len(), 4);

    let by_id: HashMap<u64, &Value> = responses
        .iter()
        .map(|v| (v["id"].as_u64().expect("id is u64"), v))
        .collect();

    let init = by_id.get(&1).expect("initialize");
    assert!(init["result"].is_object());

    let create = by_id.get(&2).expect("community_create");
    assert_eq!(create["result"]["isError"], false);

    let derive = by_id.get(&3).expect("derive_artifact");
    let text = derive["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("Community Summary"),
        "derive_artifact response missing 'Community Summary': {text}"
    );

    let export = by_id.get(&4).expect("export_skill");
    assert_eq!(
        export["result"]["isError"], false,
        "export_skill should succeed: {export:?}"
    );
}