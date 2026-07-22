use application::ApplicationService;
use mcp::McpServer;
use serde_json::{Value, json};
use std::fs;
use std::io::Cursor;
use storage::SqliteProjection;

#[test]
fn mcp_source_add_and_list() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_root = temp_dir.path();

    let vault_dir = space_root.join("vault");
    fs::create_dir_all(&vault_dir).unwrap();
    fs::write(
        vault_dir.join("note.md"),
        "---\ntitle: Vault Note\nid: 01J00000000000000000000951\n---\n",
    )
    .unwrap();

    let store = SqliteProjection::in_memory().unwrap();
    let mut service = ApplicationService::new(store);

    let input_lines = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "source_add", "arguments": {"space": space_root.to_string_lossy(), "id": "vault", "kind": "obsidian", "path": vault_dir.to_string_lossy(), "read_only": true}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "source_list", "arguments": {"space": space_root.to_string_lossy()}}}).to_string(),
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

    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[1]["id"], 2);
    assert!(
        responses[1]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("vault")
    );
}
