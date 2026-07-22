use application::ApplicationService;
use mcp::McpServer;
use serde_json::{Value, json};
use std::fs;
use std::io::Cursor;
use storage::SqliteProjection;

#[test]
fn mcp_sync_push_pull_and_conflicts() {
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
    let mut service_a = ApplicationService::new(store_a);

    let input_lines = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "sync_push", "arguments": {"space": space_a.to_string_lossy(), "actor": "actor_a", "folder": shared.to_string_lossy()}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "sync_pull", "arguments": {"space": space_b.to_string_lossy(), "actor": "actor_b", "folder": shared.to_string_lossy()}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "sync_conflicts", "arguments": {"space": space_b.to_string_lossy()}}}).to_string(),
    ].join("\n") + "\n";

    let reader = Cursor::new(input_lines);
    let mut output = Vec::new();

    McpServer::serve(reader, &mut output, &mut service_a).unwrap();

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
}
