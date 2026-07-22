use application::ApplicationService;
use mcp::McpServer;
use serde_json::{Value, json};
use std::fs;
use std::io::Cursor;
use storage::SqliteProjection;

#[test]
fn mcp_community_create_derive_and_export_skill() {
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

    let input_lines = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "community_create", "arguments": {"space": space_root.to_string_lossy(), "id": "mcp_comm", "name": "MCP Comm", "title_contains": "Sync"}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "derive_artifact", "arguments": {"space": space_root.to_string_lossy(), "community": "mcp_comm", "recipe": "summary"}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "export_skill", "arguments": {"space": space_root.to_string_lossy(), "community": "mcp_comm", "description": "MCP Exported Skill", "out": skill_out.to_string_lossy()}}}).to_string(),
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
        responses[1]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Community Summary")
    );
}
