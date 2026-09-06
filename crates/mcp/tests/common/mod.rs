//! Shared test helpers for the MCP integration tests.
//!
//! Each integration test (`<crate>/tests/*.rs`) gets its own binary, so
//! the helper functions below are duplicated into each test via
//! `mod common;` and `use common::*;` resolve to this file from each test
//! crate.
use notez_core::application::Engine;
use notez_mcp::NotezMcpServer;
use rmcp::service::ServiceExt;
use serde_json::{Value, json};
use std::time::Duration;
use notez_core::storage::SqliteProjection;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::timeout;
/// Build a bound engine over `space` with the Org/Markdown parsers
/// registered — the same engine shape real transports hand to the MCP
/// tools (a source-bound engine from the composition root).
pub fn make_mcp_engine(space: &std::path::Path) -> Engine<SqliteProjection> {
    std::fs::create_dir_all(space.join(".notez")).unwrap();
    let store = SqliteProjection::open(&space.join(".notez/idx.sqlite")).unwrap();
    let config = notez_core::config::model::SourceConfig {
        version: 2,
        source: notez_core::config::model::SourceIdentity {
            name: "test".into(),
            database: std::path::PathBuf::from(".notez/index.sqlite"),
        },
        workflow: Default::default(),
        sources: vec![],
        link_overrides: serde_json::Value::Null,
        scan: Default::default(),
    };
    let ctx = notez_core::application::context::SourceContext::new(
        "test",
        space.to_path_buf(),
        config,
    );
    let mut engine = Engine::with_source(store, ctx);
    engine.register_format_parser(Box::new(orgmode::OrgParser::new()));
    engine.register_format_parser(Box::new(markdown::MarkdownParser::new()));
    engine
}

/// Build a proper MCP `initialize` request payload.
///
/// Real MCP clients (Claude Desktop, Cursor, etc.) always send a fully
/// populated params object — rmcp's transport requires it. Empty `{}`
/// (which the legacy hand-rolled server accepted) is rejected by the SDK.
pub fn initialize_request(id: u32) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {
                "name": "notez-test-client",
                "version": "0.0.1",
            }
        }
    })
}

/// Drive an in-memory MCP server: send the given newline-delimited JSON-RPC
/// requests, draining each response before sending the next. This preserves
/// the strict request/response ordering the legacy hand-rolled server
/// provided, so tests that depend on side-effects of an earlier call (e.g.
/// `community_create` before `export_skill`) keep working.
///
/// Returns the responses in arrival order.
pub async fn run_session(
    service: Engine<SqliteProjection>,
    requests: Vec<String>,
) -> Vec<Value> {
    let (server_stream, client_stream) = tokio::io::duplex(8192);

    let server_task = tokio::spawn(async move {
        let server = NotezMcpServer::new(service);
        let running = match server.serve(server_stream).await {
            Ok(r) => r,
            Err(e) => panic!("server failed to start: {e}"),
        };
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

        // Read the next response line before sending the next request.
        let mut buf = String::new();
        loop {
            buf.clear();
            let read = timeout(Duration::from_secs(5), reader.read_line(&mut buf)).await;
            match read {
                Ok(Ok(0)) => break, // EOF
                Ok(Ok(_)) => {
                    let trimmed = buf.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    responses.push(serde_json::from_str(trimmed).unwrap());
                    break;
                }
                _ => break,
            }
        }
    }
    // Drop the writer so the server sees EOF and exits.
    drop(client_write);
    let _ = timeout(Duration::from_secs(2), server_task).await;
    responses
}