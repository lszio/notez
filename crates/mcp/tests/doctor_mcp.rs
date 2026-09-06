mod common;

use common::{initialize_request, run_session};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_doctor_jobs_stale_report_unsupported() {
    let temp_dir = tempfile::tempdir().unwrap();
    let source_root = temp_dir.path();

    fs::create_dir_all(source_root.join(".notez")).unwrap();

    let doc = source_root.join("note.org");
    fs::write(
        &doc,
        "#+title: MCP Doctor Note\n#+ID: 01J00000000000000000000002\n",
    )
    .unwrap();

    let mut service = common::make_mcp_engine(source_root);
    service.scan_native().unwrap();

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

    // space_doctor / task jobs / artifact stale are explicitly not yet
    // implemented — the CLI contract test locks the same "unsupported
    // capability" failure shape. The MCP surface must mirror it, never
    // fake success.
    let doctor = by_id.get(&2).expect("source_doctor");
    let text = doctor["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        doctor["result"]["isError"] == true && text.contains("unsupported capability"),
        "source_doctor should surface unsupported capability: {doctor:?}"
    );

    let jobs = by_id.get(&3).expect("job_list");
    let jobs_text = jobs["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        jobs["result"]["isError"] == true && jobs_text.contains("unsupported capability"),
        "job_list should surface unsupported capability: {jobs:?}"
    );

    let stale = by_id.get(&4).expect("artifact_stale");
    let stale_text = stale["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        stale["result"]["isError"] == true && stale_text.contains("unsupported capability"),
        "artifact_stale should surface unsupported capability: {stale:?}"
    );
}