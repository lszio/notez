use application::ApplicationService;
use mcp::McpServer;
use serde_json::{Value, json};
use std::fs;
use std::io::Cursor;
use storage::SqliteProjection;

#[test]
fn mcp_space_doctor_job_list_and_artifact_stale() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_root = temp_dir.path();

    fs::create_dir_all(space_root.join(".notez")).unwrap();

    let doc = space_root.join("note.org");
    fs::write(
        &doc,
        "#+title: MCP Doctor Note\n#+ID: 01J00000000000000000000002\n",
    )
    .unwrap();

    let store = SqliteProjection::open(&space_root.join(".notez/index.sqlite")).unwrap();
    let mut service = ApplicationService::new(store);
    service.scan_native(space_root).unwrap();

    let input_lines = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "space_doctor", "arguments": {"space": space_root.to_string_lossy()}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "job_list", "arguments": {"space": space_root.to_string_lossy()}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "artifact_stale", "arguments": {"space": space_root.to_string_lossy()}}}).to_string(),
    ].join("\n") + "\n";

    let reader = Cursor::new(input_lines);
    let mut output = Vec::new();

    McpServer::serve(reader, &mut output, &mut service).unwrap();

    let output_str = String::from_utf8(output).unwrap();
    let responses: Vec<Value> = output_str
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[1]["id"], 2);
    assert_eq!(responses[2]["id"], 3);
    assert!(
        responses[0]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("healthy")
    );
}
