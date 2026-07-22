use application::ApplicationService;
use mcp::McpServer;
use serde_json::{Value, json};
use std::fs;
use std::io::Cursor;
use storage::SqliteProjection;

#[test]
fn mcp_stdio_jsonrpc_transcript() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_root = temp_dir.path();

    let file1 = space_root.join("file1.org");
    fs::write(
        &file1,
        "#+title: MCP Test Doc\n#+ID: 01J00000000000000000000200\n* NEXT Sync mcp heading\n:PROPERTIES:\n:ID: 01J00000000000000000000201\n:END:\n",
    ).unwrap();

    let store = SqliteProjection::in_memory().unwrap();
    let mut service = ApplicationService::new(store);
    service.scan_native(space_root).unwrap();

    let input_lines = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "query", "arguments": {"kind": "heading", "title_contains": "sync"}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "resolve", "arguments": {"query": "nonexistent"}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "read", "arguments": {"ref": "heading:01J00000000000000000000201"}}}).to_string(),
        json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "inspect", "arguments": {}}}).to_string(),
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

    assert_eq!(responses.len(), 6);

    // Response 1: initialize
    assert_eq!(responses[0]["id"], 1);
    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "notez-mcp");

    // Response 2: tools/list
    assert_eq!(responses[1]["id"], 2);
    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|t| t["name"] == "resolve"));
    assert!(tools.iter().any(|t| t["name"] == "query"));
    assert!(tools.iter().any(|t| t["name"] == "read"));
    assert!(tools.iter().any(|t| t["name"] == "inspect"));

    // Response 3: tools/call query
    assert_eq!(responses[2]["id"], 3);
    let text = responses[2]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(text.contains("Sync mcp heading"));

    // Response 4: tools/call resolve nonexistent -> isError: true
    assert_eq!(responses[3]["id"], 4);
    assert_eq!(responses[3]["result"]["isError"], true);

    // Response 5: tools/call read heading:01J...
    assert_eq!(responses[4]["id"], 5);
    let read_text = responses[4]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(read_text.contains("Sync mcp heading"));

    // Response 6: tools/call inspect
    assert_eq!(responses[5]["id"], 6);
    assert!(
        responses[5]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("items")
    );
}
#[test]
fn mcp_stdio_malformed_json_recovery() {
    let _temp_dir = tempfile::tempdir().unwrap();
    let store = SqliteProjection::in_memory().unwrap();
    let mut service = ApplicationService::new(store);

    let input_lines = [
        "{ invalid json line }",
        &json!({"jsonrpc": "2.0", "id": 42, "method": "initialize", "params": {}}).to_string(),
    ]
    .join("\n")
        + "\n";

    let reader = Cursor::new(input_lines);
    let mut output = Vec::new();

    McpServer::serve(reader, &mut output, &mut service).unwrap();

    let output_str = String::from_utf8(output).unwrap();
    let lines: Vec<&str> = output_str
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    assert_eq!(lines.len(), 2);

    let parse_err: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(parse_err["error"]["code"], -32700);

    let init_resp: Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(init_resp["id"], 42);
}
