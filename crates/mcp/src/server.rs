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

fn parse_path_list(value: Option<&Value>) -> Vec<std::path::PathBuf> {
    let Some(v) = value else {
        return Vec::new();
    };
    if let Some(arr) = v.as_array() {
        arr.iter()
            .filter_map(|item| item.as_str().map(std::path::PathBuf::from))
            .collect()
    } else if let Some(s) = v.as_str() {
        vec![std::path::PathBuf::from(s)]
    } else {
        Vec::new()
    }
}

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
                            "name": "link_list",
                            "description": "List link occurrences for a resource",
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
                            "name": "link_resolve",
                            "description": "Re-resolve link occurrences for a resource and return relations",
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
                            "name": "link_diagnose",
                            "description": "Diagnose link occurrences (status + candidates) for a resource",
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
                            "name": "link_reindex",
                            "description": "Reindex link status counts for the whole space",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "space": { "type": "string" }
                                },
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "inspect",
                            "description": "Inspect projection status and resources",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "ref": { "type": "string" }
                                },
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "agenda",
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
                        },
                        {
                            "name": "community_create",
                            "description": "Create a community in space",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "space": { "type": "string" },
                                    "id": { "type": "string" },
                                    "name": { "type": "string" },
                                    "kind": { "type": "string" },
                                    "title_contains": { "type": "string" }
                                },
                                "required": ["id", "name"],
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "derive_artifact",
                            "description": "Derive a recipe artifact (summary, llms.txt, context-pack, skill-ir)",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "space": { "type": "string" },
                                    "community": { "type": "string" },
                                    "recipe": { "type": "string" }
                                },
                                "required": ["community", "recipe"],
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "export_skill",
                            "description": "Export a SKILL.md package for a community",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "space": { "type": "string" },
                                    "community": { "type": "string" },
                                    "description": { "type": "string" },
                                    "out": { "type": "string" }
                                },
                                "required": ["community", "out"],
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "sync_push",
                            "description": "Push space changes to shared folder",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "space": { "type": "string" },
                                    "actor": { "type": "string" },
                                    "folder": { "type": "string" }
                                },
                                "required": ["folder"],
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "sync_pull",
                            "description": "Pull changes from shared folder into space",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "space": { "type": "string" },
                                    "actor": { "type": "string" },
                                    "folder": { "type": "string" }
                                },
                                "required": ["folder"],
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "sync_conflicts",
                            "description": "List active sync conflicts in space",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "space": { "type": "string" }
                                },
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "space_doctor",
                            "description": "Run space integrity diagnostics",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "space": { "type": "string" }
                                },
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "job_list",
                            "description": "List background jobs and tasks",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "space": { "type": "string" }
                                },
                                "additionalProperties": false
                            }
                        },
                        {
                            "name": "artifact_stale",
                            "description": "Check artifact freshness against source files",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "space": { "type": "string" }
                                },
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
                let address_str = args
                    .get("address")
                    .or_else(|| args.get("query"))
                    .and_then(|q| q.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'address'".to_string(), false))?;
                if let Ok(addr) = domain::ResourceAddress::parse(address_str) {
                    match service.resolve_address(&addr) {
                        Ok(ResolveResult::Found(r_ref)) => {
                            Ok(json!({ "ref": r_ref.to_string() }).to_string())
                        }
                        Ok(ResolveResult::NotFound) => {
                            Err((format!("Resource not found: {address_str}"), true))
                        }
                        Ok(ResolveResult::Ambiguous(refs)) => {
                            let str_refs: Vec<String> = refs.iter().map(|r| r.to_string()).collect();
                            Err((
                                format!("Ambiguous resolve '{address_str}': matches {str_refs:?}"),
                                true,
                            ))
                        }
                        Err(e) => Err((format!("Internal resolve error: {e}"), true)),
                    }
                } else {
                    match service.resolve(address_str) {
                        Ok(ResolveResult::Found(r_ref)) => {
                            Ok(json!({ "ref": r_ref.to_string() }).to_string())
                        }
                        Ok(ResolveResult::NotFound) => {
                            Err((format!("Resource not found: {address_str}"), true))
                        }
                        Ok(ResolveResult::Ambiguous(refs)) => {
                            let str_refs: Vec<String> = refs.iter().map(|r| r.to_string()).collect();
                            Err((
                                format!("Ambiguous resolve '{address_str}': matches {str_refs:?}"),
                                true,
                            ))
                        }
                        Err(e) => Err((format!("Internal resolve error: {e}"), true)),
                    }
                }
            }
            "link_list" => {
                let ref_str = args
                    .get("ref")
                    .and_then(|r| r.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'ref'".to_string(), false))?;
                let parsed = ResourceRef::parse(ref_str)
                    .map_err(|e| (format!("Invalid resource ref '{ref_str}': {e}"), false))?;
                match service.list_links(&parsed) {
                    Ok(occs) => Ok(serde_json::to_string(&occs).unwrap()),
                    Err(e) => Err((format!("Internal link_list error: {e}"), true)),
                }
            }
            "link_resolve" => {
                let ref_str = args
                    .get("ref")
                    .and_then(|r| r.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'ref'".to_string(), false))?;
                let parsed = ResourceRef::parse(ref_str)
                    .map_err(|e| (format!("Invalid resource ref '{ref_str}': {e}"), false))?;
                match service.resolve_links(&parsed) {
                    Ok(rels) => Ok(serde_json::to_string(&rels).unwrap()),
                    Err(e) => Err((format!("Internal link_resolve error: {e}"), true)),
                }
            }
            "link_diagnose" => {
                let ref_str = args
                    .get("ref")
                    .and_then(|r| r.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'ref'".to_string(), false))?;
                let parsed = ResourceRef::parse(ref_str)
                    .map_err(|e| (format!("Invalid resource ref '{ref_str}': {e}"), false))?;
                match service.diagnose_link(&parsed) {
                    Ok(diags) => Ok(serde_json::to_string(&diags).unwrap()),
                    Err(e) => Err((format!("Internal link_diagnose error: {e}"), true)),
                }
            }
            "link_reindex" => {
                let space_str = args
                    .get("space")
                    .and_then(|s| s.as_str())
                    .unwrap_or(".");
                let space_path = std::path::Path::new(space_str);
                match service.reindex_links(space_path) {
                    Ok(report) => Ok(serde_json::to_string(&report).unwrap()),
                    Err(e) => Err((format!("Internal link_reindex error: {e}"), true)),
                }
            }
            "inspect" => {
                if let Some(ref_str) = args.get("ref").and_then(|r| r.as_str()) {
                    let parsed = ResourceRef::parse(ref_str).map_err(|e| {
                        (format!("Invalid resource ref '{ref_str}': {e}"), false)
                    })?;
                    let occs = service.list_links(&parsed).map_err(|e| {
                        (format!("Internal inspect error: {e}"), true)
                    })?;
                    let resolved = service.resolve_links(&parsed).map_err(|e| {
                        (format!("Internal inspect error: {e}"), true)
                    })?;
                    let diags = service.diagnose_link(&parsed).map_err(|e| {
                        (format!("Internal inspect error: {e}"), true)
                    })?;
                    let mut by_status = serde_json::Map::new();
                    for d in &diags {
                        let key = format!("{:?}", d.status);
                        let count = by_status
                            .get(&key)
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        by_status.insert(key, serde_json::json!(count + 1));
                    }
                    Ok(json!({
                        "ref": parsed.to_string(),
                        "occurrences": occs,
                        "resolved_relations": resolved,
                        "diagnostics": diags,
                        "occurrences_by_status": by_status,
                    })
                    .to_string())
                } else {
                    // No ref provided: project-level snapshot.
                    let page = service.query(&Selector::new()).map_err(|e| {
                        (format!("Internal inspect error: {e}"), true)
                    })?;
                    Ok(json!({
                        "status": "ok",
                        "total_items": page.items.len(),
                    })
                    .to_string())
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
                let include_paths = parse_path_list(args.get("include_paths"));
                let exclude_paths = parse_path_list(args.get("exclude_paths"));

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
                    include_paths,
                    exclude_paths,
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
            "community_create" => {
                let space_str = args.get("space").and_then(|s| s.as_str()).unwrap_or(".");
                let space_path = std::path::Path::new(space_str);

                let id = args
                    .get("id")
                    .and_then(|i| i.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'id'".to_string(), false))?;
                let name = args
                    .get("name")
                    .and_then(|n| n.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'name'".to_string(), false))?;

                let mut selector = Selector::new();
                if let Some(kind_str) = args.get("kind").and_then(|k| k.as_str()) {
                    match kind_str {
                        "document" => selector.kind = Some(ResourceKind::Document),
                        "heading" => selector.kind = Some(ResourceKind::Heading),
                        "attachment" => selector.kind = Some(ResourceKind::Attachment),
                        _ => return Err((format!("Unknown resource kind: {kind_str}"), false)),
                    }
                }
                if let Some(title_sub) = args.get("title_contains").and_then(|t| t.as_str()) {
                    selector.title_contains = Some(title_sub.to_string());
                }

                let comm = domain::community::Community {
                    id: id.to_string(),
                    name: name.to_string(),
                    selector,
                    pinned_members: vec![],
                    excluded_members: vec![],
                };

                match service.create_community(space_path, comm) {
                    Ok(_) => Ok(json!({ "created": true }).to_string()),
                    Err(e) => Err((format!("Internal community_create error: {e}"), true)),
                }
            }

            "derive_artifact" => {
                let space_str = args.get("space").and_then(|s| s.as_str()).unwrap_or(".");
                let space_path = std::path::Path::new(space_str);

                let community_id = args
                    .get("community")
                    .and_then(|c| c.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'community'".to_string(), false))?;
                let recipe_name = args
                    .get("recipe")
                    .and_then(|r| r.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'recipe'".to_string(), false))?;

                match service.derive_artifact(space_path, community_id, recipe_name) {
                    Ok(derived) => Ok(json!({
                        "recipe": recipe_name,
                        "content": derived.content
                    })
                    .to_string()),
                    Err(e) => Err((format!("Internal derive_artifact error: {e}"), true)),
                }
            }

            "export_skill" => {
                let space_str = args.get("space").and_then(|s| s.as_str()).unwrap_or(".");
                let space_path = std::path::Path::new(space_str);

                let community_id = args
                    .get("community")
                    .and_then(|c| c.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'community'".to_string(), false))?;
                let description = args
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("Exported Agent Skill");
                let out_str = args
                    .get("out")
                    .and_then(|o| o.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'out'".to_string(), false))?;
                let out_path = std::path::Path::new(out_str);

                match service.export_skill(space_path, community_id, description, out_path) {
                    Ok(package) => Ok(json!({
                        "name": package.name,
                        "path": package.package_path.to_string_lossy()
                    })
                    .to_string()),
                    Err(e) => Err((format!("Internal export_skill error: {e}"), true)),
                }
            }
            "sync_push" => {
                let space_str = args.get("space").and_then(|s| s.as_str()).unwrap_or(".");
                let space_path = std::path::Path::new(space_str);
                let actor = args
                    .get("actor")
                    .and_then(|a| a.as_str())
                    .unwrap_or("mcp_actor");
                let folder_str = args
                    .get("folder")
                    .and_then(|f| f.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'folder'".to_string(), false))?;
                let folder_path = std::path::Path::new(folder_str);

                match service.sync_push(actor, space_path, folder_path) {
                    Ok(report) => Ok(json!({
                        "pushed_files": report.pushed_files,
                        "pushed_objects": report.pushed_objects
                    })
                    .to_string()),
                    Err(e) => Err((format!("Internal sync_push error: {e}"), true)),
                }
            }

            "sync_pull" => {
                let space_str = args.get("space").and_then(|s| s.as_str()).unwrap_or(".");
                let space_path = std::path::Path::new(space_str);
                let actor = args
                    .get("actor")
                    .and_then(|a| a.as_str())
                    .unwrap_or("mcp_actor");
                let folder_str = args
                    .get("folder")
                    .and_then(|f| f.as_str())
                    .ok_or_else(|| ("Invalid params: missing 'folder'".to_string(), false))?;
                let folder_path = std::path::Path::new(folder_str);

                match service.sync_pull(actor, space_path, folder_path) {
                    Ok(report) => Ok(json!({
                        "pulled_files": report.pulled_files,
                        "merged_files": report.merged_files,
                        "conflicts_count": report.conflicts.len()
                    })
                    .to_string()),
                    Err(e) => Err((format!("Internal sync_pull error: {e}"), true)),
                }
            }

            "sync_conflicts" => match service.list_conflicts() {
                Ok(conflicts) => Ok(serde_json::to_string(&conflicts).unwrap()),
                Err(e) => Err((format!("Internal sync_conflicts error: {e}"), true)),
            },
            "space_doctor" => {
                let space_str = args.get("space").and_then(|s| s.as_str()).unwrap_or(".");
                let space_path = std::path::Path::new(space_str);
                match service.space_doctor(space_path) {
                    Ok(report) => Ok(serde_json::to_string(&report).unwrap()),
                    Err(e) => Err((format!("Internal space_doctor error: {e}"), true)),
                }
            }

            "job_list" => match service.list_jobs() {
                Ok(jobs) => Ok(serde_json::to_string(&jobs).unwrap()),
                Err(e) => Err((format!("Internal job_list error: {e}"), true)),
            },

            "artifact_stale" => {
                let space_str = args.get("space").and_then(|s| s.as_str()).unwrap_or(".");
                let space_path = std::path::Path::new(space_str);
                match service.check_artifact_freshness(space_path) {
                    Ok(report) => Ok(serde_json::to_string(&report).unwrap()),
                    Err(e) => Err((format!("Internal artifact_stale error: {e}"), true)),
                }
            }
            _ => Err((format!("Unknown tool: {name}"), false)),
        }
    }
}
