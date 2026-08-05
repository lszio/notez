mod common;

use notez_core::application::ApplicationService;
use common::{initialize_request, run_session};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use notez_core::storage::SqliteProjection;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_attachment_add_extract_and_query_segments() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_root = temp_dir.path();

    let file_path = space_root.join("mcp_sample.txt");
    fs::write(&file_path, "MCP attachment content for extraction testing.").unwrap();

    let store = SqliteProjection::in_memory().unwrap();
    let mut service = ApplicationService::new(store);

    let att_ref = service
        .add_attachment(space_root, &file_path, "text/plain")
        .unwrap();

    let requests = vec![
        initialize_request(1).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "attachment_extract", "arguments": {"space": space_root.to_string_lossy(), "ref": att_ref.to_string()}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "query_segments", "arguments": {"ref": att_ref.to_string()}}}).to_string(),
    ];

    let responses = run_session(service, requests).await;
    assert_eq!(responses.len(), 3);

    let by_id: HashMap<u64, &Value> = responses
        .iter()
        .map(|v| (v["id"].as_u64().expect("id is u64"), v))
        .collect();

    let init = by_id.get(&1).expect("initialize");
    assert!(init["result"].is_object());

    let extract = by_id.get(&2).expect("attachment_extract");
    assert_eq!(
        extract["result"]["isError"], false,
        "attachment_extract should succeed: {extract:?}"
    );

    let segments = by_id.get(&3).expect("query_segments");
    let text = segments["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        text.contains("extraction testing"),
        "query_segments response missing 'extraction testing': {text}"
    );
}