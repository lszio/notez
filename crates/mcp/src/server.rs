use application::{ApplicationService, ResolveResult};
use domain::{ProjectionStore, ResourceKind, ResourceRef, Selector};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum McpError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct McpServer;

impl McpServer {
    pub fn serve<R: BufRead, W: Write, S: ProjectionStore>(
        mut reader: R,
        mut writer: W,
        service: &mut ApplicationService<S>,
    ) -> Result<(), McpError> {
        let mut line = String::new();

        while reader.read_line(&mut line)? > 0 {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                line.clear();
                continue;
            }

            let req_value: Result<Value, _> = serde_json::from_str(trimmed);
            match req_value {
                Ok(req) => {
                    let id = req.get("id").cloned();
                    let method = req
                        .get("method")
                        .and_then(|m| m.as_str())
                        .unwrap_or_default();
                    let params = req.get("params").cloned().unwrap_or(Value::Null);

                    if method == "notifications/initialized" {
                        line.clear();
                        continue;
                    }

                    let response = Self::handle_request(id, method, params, service);
                    if let Some(resp) = response {
                        let resp_json = serde_json::to_string(&resp)?;
                        writeln!(writer, "{resp_json}")?;
                        writer.flush()?;
                    }
                }
                Err(e) => {
                    let err_resp = json!({
                        "jsonrpc": "2.0",
                        "id": Value::Null,
                        "error": {
                            "code": -32700,
                            "message": format!("Parse error: {e}")
                        }
                    });
                    writeln!(writer, "{}", serde_json::to_string(&err_resp)?)?;
                    writer.flush()?;
                }
            }

            line.clear();
        }

        Ok(())
    }

    fn handle_request<S: ProjectionStore>(
        id: Option<Value>,
        method: &str,
        params: Value,
        service: &mut ApplicationService<S>,
    ) -> Option<Value> {
        let req_id = id?;

        match method {
            "initialize" => Some(json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "notez-mcp",
                        "version": "0.1.0"
                    }
                }
            })),

            "tools/list" => Some(json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "result": {
                    "tools": [
                        {
                            "name": "resolve",
                            "description": "Resolve a query string to a ResourceRef",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "query": { "type": "string" }
                                },
                                "required": ["query"],
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "query",
                            "description": "Query resources in the space",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "kind": { "type": "string" },
                                    "title_contains": { "type": "string" }
                                },
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "read",
                            "description": "Read details of a resource by ref",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "ref": { "type": "string" }
                                },
                                "required": ["ref"],
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "inspect",
                            "description": "Inspect projection status and resources",
                            "inputSchema": {
                                "type": "object",
                                "properties": {},
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "inspect_rules",
                            "description": "Inspect rule traces for a resource ref",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "ref": { "type": "string" }
                                },
                                "required": ["ref"],
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "agenda",
                            "description": "Query agenda view for scheduled or deadline items",
                            "inputSchema": {
                                "type": "object",
                                "properties": {},
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "task_transition",
                            "description": "Transition task state",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "ref": { "type": "string" },
                                    "to": { "type": "string" },
                                    "timestamp": { "type": "string" }
                                },
                                "required": ["ref", "to"],
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "source_list",
                            "description": "List configured sources in the space",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "space": { "type": "string" }
                                },
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "source_add",
                            "description": "Add an external source to the space",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "space": { "type": "string" },
                                    "id": { "type": "string" },
                                    "kind": { "type": "string" },
                                    "path": { "type": "string" },
                                    "read_only": { "type": "boolean" }
                                },
                                "required": ["id", "kind", "path"],
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "attachment_add",
                            "description": "Add an attachment file to space",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "space": { "type": "string" },
                                    "path": { "type": "string" },
                                    "mime": { "type": "string" }
                                },
                                "required": ["path"],
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "attachment_extract",
                            "description": "Run text/metadata extraction job on attachment ref",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "space": { "type": "string" },
                                    "ref": { "type": "string" }
                                },
                                "required": ["ref"],
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "query_segments",
                            "description": "Query extracted text segments for attachment ref",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "ref": { "type": "string" }
                                },
                                "required": ["ref"],
                                "additionalProperties": false
                            }
                        }
                    ]
                }
            })),

            "tools/call" => {
                let tool_name = params
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or_default();
                let args = params.get("arguments").cloned().unwrap_or(json!({}));

                let tool_res = Self::call_tool(tool_name, args, service);
                Some(match tool_res {
                    Ok(text) => json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": text
                                }
                            ]
                        }
                    }),
                    Err((err_msg, is_tool_err)) => {
                        if is_tool_err {
                            json!({
                                "jsonrpc": "2.0",
                                "id": req_id,
                                "result": {
                                    "content": [
                                        {
                                            "type": "text",
                                            "text": err_msg
                                        }
                                    ],
                                    "isError": true
                                }
                            })
                        } else {
                            json!({
                                "jsonrpc": "2.0",
                                "id": req_id,
                                "error": {
                                    "code": -32602,
                                    "message": err_msg
                                }
                            })
                        }
                    }
                })
            }

            _ => Some(json!({
                "jsonrpc": "2.0",
                "id": req_id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {method}")
                }
            })),
        }
    }

    fn call_tool<S: ProjectionStore>(
        name: &str,
        args: Value,
        service: &mut ApplicationService<S>,
    ) -> Result<String, (String, bool)> {
        match name {
            "resolve" => {
                let query = args
                    .get("query")
                    .and_then(|q| q.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'query'".to_string(), false))?;
                match service.resolve(query) {
                    Ok(ResolveResult::Found(r_ref)) => {
                        Ok(json!({ "ref": r_ref.to_string() }).to_string())
                    }
                    Ok(ResolveResult::NotFound) => {
                        Err((format!("Resource not found: {query}"), true))
                    }
                    Ok(ResolveResult::Ambiguous(refs)) => {
                        let str_refs: Vec<String> = refs.iter().map(|r| r.to_string()).collect();
                        Err((
                            format!("Ambiguous query '{query}': matches {str_refs:?}"),
                            true,
                        ))
                    }
                    Err(e) => Err((format!("Internal resolve error: {e}"), true)),
                }
            }

            "query" => {
                let mut selector = Selector::new();
                if let Some(kind_str) = args.get("kind").and_then(|k| k.as_str()) {
                    match kind_str {
                        "document" => selector.kind = Some(ResourceKind::Document),
                        "heading" => selector.kind = Some(ResourceKind::Heading),
                        _ => return Err((format!("Unknown resource kind: {kind_str}"), false)),
                    }
                }
                if let Some(title_sub) = args.get("title_contains").and_then(|t| t.as_str()) {
                    selector.title_contains = Some(title_sub.to_string());
                }

                match service.query(&selector) {
                    Ok(page) => Ok(serde_json::to_string(&page).unwrap()),
                    Err(e) => Err((format!("Internal query error: {e}"), true)),
                }
            }

            "read" => {
                let ref_str = args
                    .get("ref")
                    .and_then(|r| r.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'ref'".to_string(), false))?;
                let r_ref = ResourceRef::parse(ref_str)
                    .map_err(|e| (format!("Invalid resource ref '{ref_str}': {e}"), false))?;

                match service.read(&r_ref) {
                    Ok(Some(res)) => Ok(serde_json::to_string(&res).unwrap()),
                    Ok(None) => Err((format!("Resource not found: {ref_str}"), true)),
                    Err(e) => Err((format!("Internal read error: {e}"), true)),
                }
            }

            "inspect" => match service.query(&Selector::new()) {
                Ok(page) => Ok(json!({
                    "status": "ok",
                    "total_items": page.items.len(),
                    "items": page.items
                })
                .to_string()),
                Err(e) => Err((format!("Internal inspect error: {e}"), true)),
            },

            "inspect_rules" => {
                let ref_str = args
                    .get("ref")
                    .and_then(|r| r.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'ref'".to_string(), false))?;
                let r_ref = ResourceRef::parse(ref_str)
                    .map_err(|e| (format!("Invalid resource ref '{ref_str}': {e}"), false))?;

                match service.inspect_rules(&r_ref) {
                    Ok(Some(inspect_res)) => Ok(serde_json::to_string(&inspect_res).unwrap()),
                    Ok(None) => Err((format!("Resource not found: {ref_str}"), true)),
                    Err(e) => Err((format!("Internal inspect_rules error: {e}"), true)),
                }
            }

            "agenda" => match service.agenda() {
                Ok(agenda) => Ok(serde_json::to_string(&agenda).unwrap()),
                Err(e) => Err((format!("Internal agenda error: {e}"), true)),
            },

            "task_transition" => {
                let ref_str = args
                    .get("ref")
                    .and_then(|r| r.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'ref'".to_string(), false))?;
                let to_state = args
                    .get("to")
                    .and_then(|t| t.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'to'".to_string(), false))?;
                let timestamp = args
                    .get("timestamp")
                    .and_then(|ts| ts.as_str())
                    .unwrap_or("2026-07-22 Wed 16:00");

                let r_ref = ResourceRef::parse(ref_str)
                    .map_err(|e| (format!("Invalid resource ref '{ref_str}': {e}"), false))?;

                match service.transition_task(&r_ref, to_state, timestamp) {
                    Ok(transition) => Ok(serde_json::to_string(&transition).unwrap()),
                    Err(e) => Err((format!("Internal transition error: {e}"), true)),
                }
            }
            "source_list" => {
                let space_str = args.get("space").and_then(|s| s.as_str()).unwrap_or(".");
                let space_path = std::path::Path::new(space_str);
                match service.list_sources(space_path) {
                    Ok(sources) => Ok(serde_json::to_string(&sources).unwrap()),
                    Err(e) => Err((format!("Internal source_list error: {e}"), true)),
                }
            }

            "source_add" => {
                let space_str = args.get("space").and_then(|s| s.as_str()).unwrap_or(".");
                let space_path = std::path::Path::new(space_str);

                let id = args
                    .get("id")
                    .and_then(|i| i.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'id'".to_string(), false))?;
                let kind_str = args
                    .get("kind")
                    .and_then(|k| k.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'kind'".to_string(), false))?;
                let path_str = args
                    .get("path")
                    .and_then(|p| p.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'path'".to_string(), false))?;
                let read_only = args
                    .get("read_only")
                    .and_then(|r| r.as_bool())
                    .unwrap_or(false);

                let kind = match kind_str {
                    "native" => source::SourceKind::Native,
                    "git" => source::SourceKind::Git,
                    "obsidian" => source::SourceKind::Obsidian,
                    _ => return Err((format!("Unknown source kind: {kind_str}"), false)),
                };

                let config = source::SourceConfig {
                    id: id.to_string(),
                    kind,
                    path: std::path::PathBuf::from(path_str),
                    read_only,
                    exclude_paths: vec![],
                };

                match service.add_source(space_path, config) {
                    Ok(_) => Ok(json!({ "added": true }).to_string()),
                    Err(e) => Err((format!("Internal source_add error: {e}"), true)),
                }
            }
            "attachment_add" => {
                let space_str = args.get("space").and_then(|s| s.as_str()).unwrap_or(".");
                let space_path = std::path::Path::new(space_str);

                let path_str = args
                    .get("path")
                    .and_then(|p| p.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'path'".to_string(), false))?;
                let default_mime = args
                    .get("mime")
                    .and_then(|m| m.as_str())
                    .unwrap_or("application/octet-stream");

                let file_path = std::path::Path::new(path_str);
                match service.add_attachment(space_path, file_path, default_mime) {
                    Ok(att_ref) => Ok(json!({ "ref": att_ref.to_string() }).to_string()),
                    Err(e) => Err((format!("Internal attachment_add error: {e}"), true)),
                }
            }

            "attachment_extract" => {
                let space_str = args.get("space").and_then(|s| s.as_str()).unwrap_or(".");
                let space_path = std::path::Path::new(space_str);

                let ref_str = args
                    .get("ref")
                    .and_then(|r| r.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'ref'".to_string(), false))?;
                let r_ref = ResourceRef::parse(ref_str)
                    .map_err(|e| (format!("Invalid resource ref '{ref_str}': {e}"), false))?;

                match service.run_extraction(space_path, &r_ref) {
                    Ok(segments) => Ok(json!({
                        "attachment_ref": ref_str,
                        "segments_count": segments.len()
                    })
                    .to_string()),
                    Err(e) => Err((format!("Internal attachment_extract error: {e}"), true)),
                }
            }

            "query_segments" => {
                let ref_str = args
                    .get("ref")
                    .and_then(|r| r.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'ref'".to_string(), false))?;
                let r_ref = ResourceRef::parse(ref_str)
                    .map_err(|e| (format!("Invalid resource ref '{ref_str}': {e}"), false))?;

                match service.query_segments(&r_ref) {
                    Ok(segments) => Ok(serde_json::to_string(&segments).unwrap()),
                    Err(e) => Err((format!("Internal query_segments error: {e}"), true)),
                }
            }
            _ => Err((format!("Unknown tool: {name}"), false)),
        }
    }
}
