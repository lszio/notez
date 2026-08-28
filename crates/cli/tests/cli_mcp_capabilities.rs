//! MCP `list_capabilities` end-to-end test.
//!
//! Drives an in-process `NotezMcpServer` over a stdio-like pipe
//! (tokio::io::duplex) and asserts that the `list_capabilities` tool
//! emits the same JSON array as the CLI `list-capabilities` subcommand.
//!
//! This locks in the equivalence contract: every capability listed by
//! the CLI must also be observable via the MCP `tools/list` and
//! `list_capabilities` tool surface.

use notez_cli::mcp::NotezMcpServer;
use notez_core::application::Engine;
use notez_core::storage::SqliteProjection;
use rmcp::service::ServiceExt;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::timeout;

const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

fn initialize_request(id: u32) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "notez-cap-test", "version": "0.0.1" }
        }
    })
}

/// Drive an in-process MCP server with a list of newline-delimited
/// JSON-RPC requests and collect the responses in arrival order.
async fn run_session(
    service: Engine<SqliteProjection>,
    requests: Vec<String>,
) -> Vec<Value> {
    let (server_stream, client_stream) = tokio::io::duplex(8192);

    let server_task = tokio::spawn(async move {
        let server = NotezMcpServer::new(service);
        let running = server
            .serve(server_stream)
            .await
            .expect("server failed to start");
        let _ = running.waiting().await;
    });

    let (client_read, mut client_write) = tokio::io::split(client_stream);
    let mut reader = BufReader::new(client_read);
    let mut responses: Vec<Value> = Vec::new();

    for req in &requests {
        client_write
            .write_all(req.as_bytes())
            .await
            .expect("write request");
        client_write.write_all(b"\n").await.expect("write newline");
        client_write.flush().await.expect("flush");

        let mut buf = String::new();
        loop {
            buf.clear();
            let read = timeout(Duration::from_secs(5), reader.read_line(&mut buf)).await;
            match read {
                Ok(Ok(0)) => break,
                Ok(Ok(_)) => {
                    let trimmed = buf.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let value: Value = serde_json::from_str(trimmed)
                        .unwrap_or_else(|e| panic!("invalid JSON `{trimmed}`: {e}"));
                    if value.get("id").is_some() {
                        responses.push(value);
                        break;
                    }
                    // Notifications: skip.
                }
                Ok(Err(e)) => panic!("read error: {e}"),
                Err(_) => panic!("timeout reading response"),
            }
        }
    }

    server_task.abort();
    let _ = server_task.await;
    responses
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_list_capabilities_payload_matches_facade_helper() {
    let store = SqliteProjection::in_memory().expect("in-memory store");
    let service = Engine::new(store);
    let expected = service.capabilities_json();

    let requests = vec![
        initialize_request(1).to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        })
        .to_string(),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "list_capabilities",
                "arguments": {}
            }
        })
        .to_string(),
    ];

    let responses = run_session(service, requests).await;
    assert_eq!(responses.len(), 3, "expected 3 responses");

    let init = &responses[0];
    assert!(init["result"].is_object(), "initialize result missing");

    let list = &responses[1];
    let tools = list["result"]["tools"]
        .as_array()
        .expect("tools/list result.tools is an array");
    assert!(
        tools.iter().any(|t| t["name"] == "list_capabilities"),
        "tools/list must expose `list_capabilities`"
    );

    let call = &responses[2];
    assert_eq!(
        call["result"]["isError"], false,
        "list_capabilities tool call should succeed: {call:?}"
    );
    let text = call["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    let payload: Value = serde_json::from_str(text)
        .unwrap_or_else(|e| panic!("response text is not valid JSON: {e}: {text}"));

    assert_eq!(
        payload, expected,
        "MCP `list_capabilities` payload must equal facade capability listing"
    );
}
