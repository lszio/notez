use application::ApplicationService;
use mcp::McpServer;
use serde_json::{Value, json};
use std::fs;
use std::io::Cursor;
use storage::SqliteProjection;

#[test]
fn mcp_rules_agenda_and_task_transition() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_root = temp_dir.path();

    let file = space_root.join("tasks.org");
    fs::write(
        &file,
        "#+title: MCP Rules\n#+ID: 01J00000000000000000000800\n* TODO MCP Task\n:PROPERTIES:\n:ID: 01J00000000000000000000801\n:SCHEDULED: <2026-07-22 Wed>\n:TYPE: project\n:END:\n",
    ).unwrap();

    let store = SqliteProjection::in_memory().unwrap();
    let mut service = ApplicationService::new(store);
    service.scan_native(space_root).unwrap();

    let input_lines = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "agenda", "arguments": {}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "inspect_rules", "arguments": {"ref": "heading:01J00000000000000000000801"}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "task_transition", "arguments": {"ref": "heading:01J00000000000000000000801", "to": "DONE"}}}).to_string(),
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

    // 1. agenda
    assert_eq!(responses[0]["id"], 1);
    assert!(
        responses[0]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("MCP Task")
    );

    // 2. inspect_rules
    assert_eq!(responses[1]["id"], 2);
    assert!(
        responses[1]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("classified_type")
    );

    // 3. task_transition
    assert_eq!(responses[2]["id"], 3);
    assert!(
        responses[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("to_state")
    );
}
