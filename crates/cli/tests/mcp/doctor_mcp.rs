mod common;

use notez_core::application::ApplicationService;
use common::{initialize_request, run_session};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use notez_core::storage::SqliteProjection;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_space_doctor_job_list_and_artifact_stale() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source_root = temp_dir.path();

    fs::create_dir_all(source_root.join(".notez")).unwrap();

    let doc = source_root.join("note.org");
    fs::write(
        &doc,
        "#+title: MCP Doctor Note\n#+ID: 01J00000000000000000000002\n",
    )
    .unwrap();

    let store = SqliteProjection::open(&source_root.join(".notez/index.sqlite")).unwrap();
    let mut service = ApplicationService::new(store);
    service.scan_native(source_root).unwrap();

    let requests = vec![
        initialize_request(1).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "source_doctor", "arguments": {"space": source_root.to_string_lossy()}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "job_list", "arguments": {"space": source_root.to_string_lossy()}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "artifact_stale", "arguments": {"space": source_root.to_string_lossy()}}}).to_string(),
    ];

    let responses = run_session(service, requests).await;
    assert_eq!(responses.len(), 4);

    let by_id: HashMap<u64, &Value> = responses
        .iter()
        .map(|v| (v["id"].as_u64().expect("id is u64"), v))
        .collect();

    let init = by_id.get(&1).expect("initialize");
    assert!(init["result"].is_object());

    let doctor = by_id.get(&2).expect("source_doctor");
    let text = doctor["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        text.contains("healthy"),
        "source_doctor response missing 'healthy': {text}"
    );

    let jobs = by_id.get(&3).expect("job_list");
    assert_eq!(
        jobs["result"]["isError"], false,
        "job_list should succeed: {jobs:?}"
    );

    let stale = by_id.get(&4).expect("artifact_stale");
    assert_eq!(
        stale["result"]["isError"], false,
        "artifact_stale should succeed: {stale:?}"
    );
}