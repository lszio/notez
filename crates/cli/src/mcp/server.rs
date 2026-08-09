//! notez MCP stdio server.
//!
//! Built on the `rmcp` SDK. Speaks MCP protocol version `2025-11-25`
//! (rmcp's `LATEST`). All tools are callable immediately after `initialize`
//! returns; the server does NOT track `notifications/initialized` and does
//! NOT gate tool calls on any client handshake.

use std::sync::{Arc, Mutex};

use notez_core::application::{
    use_cases::{
        ArtifactUseCase, AttachmentUseCase, CommunityUseCase, InspectUseCase, LinkUseCase,
        ResourceUseCase, ScanUseCase, SyncUseCase, TaskUseCase,
    },
    ApplicationService, ResolveResult,
};
use notez_core::domain::{ResourceKind, ResourceRef, Selector};
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{
        tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::*,
    tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};
use notez_core::storage::SqliteProjection;

// ─────────────────────────────────────────────────────────────────────────────
// Argument DTOs
//
// Each `schemars`-derived struct becomes the JSON schema exposed via
// `tools/list`. Field names match the legacy hand-rolled server EXACTLY —
// clients depend on them.
// ─────────────────────────────────────────────────────────────────────────────

use std::borrow::Cow;
use schemars::{Schema, SchemaGenerator};

// Hand-rolled `schemars::JsonSchema` impls for the 13 MCP tool DTOs.
//
// We cannot use `#[derive(schemars::JsonSchema)]` because the workspace's
// core crate is also named `core`, and schemars' 1.x derive macro emits
// absolute `::core::*` paths (convert, concat, marker, stringify, module_path)
// that resolve to our local package rather than the standard library.
// Hand-rolled impls dodge the macro entirely.
//
// Each schema mirrors the JSON the legacy derive produced: a top-level
// object with `type: "object"`, a `properties` map keyed by snake_case
// field name, and a `required` array listing the non-Option fields.
// Doc comments are preserved as `description` strings to keep the
// existing `tools/list` output stable for clients.

fn object_schema(properties: serde_json::Value, required: &[&str]) -> Schema {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), serde_json::json!("object"));
    map.insert("properties".to_string(), properties);
    if !required.is_empty() {
        map.insert(
            "required".to_string(),
            serde_json::json!(required),
        );
    }
    Schema::from(map)
}

fn prop_str(desc: &str) -> serde_json::Value {
    serde_json::json!({ "type": "string", "description": desc })
}
fn prop_opt_str() -> serde_json::Value {
    serde_json::json!({ "type": "string" })
}
fn prop_bool() -> serde_json::Value {
    serde_json::json!({ "type": "boolean" })
}
fn prop_str_array() -> serde_json::Value {
    serde_json::json!({ "type": "array", "items": { "type": "string" } })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ResolveArgs {
    /// Query string to resolve. Accepts bare IDs and fully qualified
    /// addresses (`document:01J...`).
    pub query: String,
}
impl schemars::JsonSchema for ResolveArgs {
    fn schema_name() -> Cow<'static, str> { "ResolveArgs".into() }
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut p = serde_json::Map::new();
        p.insert("query".into(), prop_str(
            "Query string to resolve. Accepts bare IDs and fully qualified addresses (`document:01J...`)."));
        object_schema(serde_json::Value::Object(p), &["query"])
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct QueryArgs {
    /// Optional resource kind filter (`document`, `heading`, `attachment`).
    pub kind: Option<String>,
    /// Optional case-sensitive substring filter on the resource title.
    pub title_contains: Option<String>,
}
impl schemars::JsonSchema for QueryArgs {
    fn schema_name() -> Cow<'static, str> { "QueryArgs".into() }
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut p = serde_json::Map::new();
        p.insert("kind".into(), prop_opt_str());
        p.insert("title_contains".into(), prop_opt_str());
        object_schema(serde_json::Value::Object(p), &[])
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RefArgs {
    /// A `ResourceRef` string (e.g. `heading:01J...`).
    pub r#ref: String,
}
impl schemars::JsonSchema for RefArgs {
    fn schema_name() -> Cow<'static, str> { "RefArgs".into() }
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut p = serde_json::Map::new();
        p.insert("ref".into(), prop_str("A `ResourceRef` string (e.g. `heading:01J...`)."));
        object_schema(serde_json::Value::Object(p), &["ref"])
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct InspectArgs {
    /// Optional resource ref. When present, returns occurrences, resolved
    /// relations, and diagnostics for that resource.
    pub r#ref: Option<String>,
}
impl schemars::JsonSchema for InspectArgs {
    fn schema_name() -> Cow<'static, str> { "InspectArgs".into() }
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut p = serde_json::Map::new();
        p.insert("ref".into(), prop_opt_str());
        object_schema(serde_json::Value::Object(p), &[])
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SpaceArgs {
    /// Filesystem path to the space root. Defaults to `.` when omitted.
    pub space: Option<String>,
}
impl schemars::JsonSchema for SpaceArgs {
    fn schema_name() -> Cow<'static, str> { "SpaceArgs".into() }
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut p = serde_json::Map::new();
        p.insert("space".into(), prop_opt_str());
        object_schema(serde_json::Value::Object(p), &[])
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceAddArgs {
    pub space: Option<String>,
    pub id: String,
    pub kind: String,
    pub path: String,
    pub read_only: Option<bool>,
    pub include_paths: Option<Vec<String>>,
    pub exclude_paths: Option<Vec<String>>,
}
impl schemars::JsonSchema for SourceAddArgs {
    fn schema_name() -> Cow<'static, str> { "SourceAddArgs".into() }
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut p = serde_json::Map::new();
        p.insert("space".into(), prop_opt_str());
        p.insert("id".into(), prop_str(""));
        p.insert("kind".into(), prop_str(""));
        p.insert("path".into(), prop_str(""));
        p.insert("read_only".into(), prop_bool());
        p.insert("include_paths".into(), prop_str_array());
        p.insert("exclude_paths".into(), prop_str_array());
        object_schema(
            serde_json::Value::Object(p),
            &["id", "kind", "path"],
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AttachmentAddArgs {
    pub space: Option<String>,
    pub path: String,
    pub mime: Option<String>,
}
impl schemars::JsonSchema for AttachmentAddArgs {
    fn schema_name() -> Cow<'static, str> { "AttachmentAddArgs".into() }
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut p = serde_json::Map::new();
        p.insert("space".into(), prop_opt_str());
        p.insert("path".into(), prop_str(""));
        p.insert("mime".into(), prop_opt_str());
        object_schema(serde_json::Value::Object(p), &["path"])
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AttachmentExtractArgs {
    pub space: Option<String>,
    pub r#ref: String,
}
impl schemars::JsonSchema for AttachmentExtractArgs {
    fn schema_name() -> Cow<'static, str> { "AttachmentExtractArgs".into() }
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut p = serde_json::Map::new();
        p.insert("space".into(), prop_opt_str());
        p.insert("ref".into(), prop_str(""));
        object_schema(serde_json::Value::Object(p), &["ref"])
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CommunityCreateArgs {
    pub space: Option<String>,
    pub id: String,
    pub name: String,
    pub kind: Option<String>,
    pub title_contains: Option<String>,
}
impl schemars::JsonSchema for CommunityCreateArgs {
    fn schema_name() -> Cow<'static, str> { "CommunityCreateArgs".into() }
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut p = serde_json::Map::new();
        p.insert("space".into(), prop_opt_str());
        p.insert("id".into(), prop_str(""));
        p.insert("name".into(), prop_str(""));
        p.insert("kind".into(), prop_opt_str());
        p.insert("title_contains".into(), prop_opt_str());
        object_schema(
            serde_json::Value::Object(p),
            &["id", "name"],
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DeriveArtifactArgs {
    pub space: Option<String>,
    pub community: String,
    pub recipe: String,
}
impl schemars::JsonSchema for DeriveArtifactArgs {
    fn schema_name() -> Cow<'static, str> { "DeriveArtifactArgs".into() }
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut p = serde_json::Map::new();
        p.insert("space".into(), prop_opt_str());
        p.insert("community".into(), prop_str(""));
        p.insert("recipe".into(), prop_str(""));
        object_schema(
            serde_json::Value::Object(p),
            &["community", "recipe"],
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ExportSkillArgs {
    pub space: Option<String>,
    pub community: String,
    pub description: Option<String>,
    pub out: String,
}
impl schemars::JsonSchema for ExportSkillArgs {
    fn schema_name() -> Cow<'static, str> { "ExportSkillArgs".into() }
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut p = serde_json::Map::new();
        p.insert("space".into(), prop_opt_str());
        p.insert("community".into(), prop_str(""));
        p.insert("description".into(), prop_opt_str());
        p.insert("out".into(), prop_str(""));
        object_schema(
            serde_json::Value::Object(p),
            &["community", "out"],
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SyncArgs {
    pub space: Option<String>,
    pub actor: Option<String>,
    pub folder: String,
}
impl schemars::JsonSchema for SyncArgs {
    fn schema_name() -> Cow<'static, str> { "SyncArgs".into() }
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut p = serde_json::Map::new();
        p.insert("space".into(), prop_opt_str());
        p.insert("actor".into(), prop_opt_str());
        p.insert("folder".into(), prop_str(""));
        object_schema(serde_json::Value::Object(p), &["folder"])
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TaskTransitionArgs {
    pub r#ref: String,
    pub to: String,
    pub timestamp: Option<String>,
}
impl schemars::JsonSchema for TaskTransitionArgs {
    fn schema_name() -> Cow<'static, str> { "TaskTransitionArgs".into() }
    fn json_schema(g: &mut SchemaGenerator) -> Schema {
        let mut p = serde_json::Map::new();
        p.insert("ref".into(), prop_str(""));
        p.insert("to".into(), prop_str(""));
        p.insert("timestamp".into(), prop_opt_str());
        object_schema(
            serde_json::Value::Object(p),
            &["ref", "to"],
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Result helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a JSON value into a text content block wrapped in a successful
/// tool result.
fn text_ok(value: Value) -> Result<CallToolResult, McpError> {
    let text = serde_json::to_string(&value)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::success(vec![Content::text(text)]))
}

/// Wrap a tool-domain error into a `CallToolResult::error` so the SDK
/// reports it as `isError: true` rather than a JSON-RPC error.
fn text_err(message: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::error(vec![Content::text(message.into())]))
}

/// Build an MCP tool error whose payload is a structured JSON object with
/// `kind` (snake_case) and `message` (the Display text). This is the
/// stable MCP error contract; see the error design spec.
fn struct_err(err: &notez_core::application::ApplicationError) -> Result<CallToolResult, McpError> {
    let kind = match err {
        notez_core::application::ApplicationError::NotFound { .. } => "not_found",
        notez_core::application::ApplicationError::Storage { .. } => "storage",
        notez_core::application::ApplicationError::Document { .. } => "document",
        notez_core::application::ApplicationError::Io { .. } => "io",
        notez_core::application::ApplicationError::UnsupportedCapability { .. } => {
            "unsupported_capability"
        }
        notez_core::application::ApplicationError::ReadOnlySource { .. } => "read_only_source",
        notez_core::application::ApplicationError::SourceNotFound { .. } => "source_not_found",
        notez_core::application::ApplicationError::RevisionConflict { .. } => "revision_conflict",
    };
    text_err(serde_json::json!({ "kind": kind, "message": err.to_string() }).to_string())
}

fn space_path(arg: Option<&str>) -> std::path::PathBuf {
    std::path::PathBuf::from(arg.unwrap_or("."))
}

// ─────────────────────────────────────────────────────────────────────────────
// Server
// ─────────────────────────────────────────────────────────────────────────────

/// MCP stdio server exposing notez operations as tools.
///
/// Wraps an `ApplicationService<SqliteProjection>` (the production
/// concrete type) behind a `Mutex` because several notez methods (e.g.
/// `transition_task`, `sync_push`, `resolve_links`) require `&mut self`.
/// The mutex is never held across an `.await` point — tool handlers call
/// sync methods, serialize the result, and return — so a `std::sync::Mutex`
/// is sufficient.
#[derive(Clone)]
pub struct NotezMcpServer {
    service: Arc<Mutex<ApplicationService<SqliteProjection>>>,
    tool_router: ToolRouter<Self>,
}

impl NotezMcpServer {
    pub fn new(service: ApplicationService<SqliteProjection>) -> Self {
        Self {
            service: Arc::new(Mutex::new(service)),
            tool_router: Self::tool_router(),
        }
    }

    /// Read-only view over the wrapped service.
    fn with_service<R>(
        &self,
        f: impl FnOnce(&ApplicationService<SqliteProjection>) -> R,
    ) -> R {
        let guard = self.service.lock().expect("service mutex poisoned");
        f(&*guard)
    }

    /// Mutable view, for tool handlers that need `&mut self`.
    fn with_service_mut<R>(
        &self,
        f: impl FnOnce(&mut ApplicationService<SqliteProjection>) -> R,
    ) -> R {
        let mut guard = self.service.lock().expect("service mutex poisoned");
        f(&mut *guard)
    }
}

#[tool_router]
impl NotezMcpServer {
    #[tool(description = "Resolve a query string to a ResourceRef")]
    fn resolve(
        &self,
        Parameters(args): Parameters<ResolveArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.with_service(|svc| {
            let address_str: &str = &args.query;
            if let Ok(addr) = notez_core::domain::ResourceAddress::parse(address_str) {
                match ResourceUseCase::resolve_address(svc, &addr) {
                    Ok(ResolveResult::Found(r_ref)) => {
                        text_ok(json!({ "ref": r_ref.to_string() }))
                    }
                    Ok(ResolveResult::NotFound) => {
                        text_err(format!("Resource not found: {address_str}"))
                    }
                    Ok(ResolveResult::Ambiguous(refs)) => {
                        let str_refs: Vec<String> =
                            refs.iter().map(|r| r.to_string()).collect();
                        text_err(format!(
                            "Ambiguous resolve '{address_str}': matches {str_refs:?}"
                        ))
                    }
                    Err(e) => struct_err(&e),
                }
            } else {
                match ResourceUseCase::resolve(svc, address_str) {
                    Ok(ResolveResult::Found(r_ref)) => {
                        text_ok(json!({ "ref": r_ref.to_string() }))
                    }
                    Ok(ResolveResult::NotFound) => {
                        text_err(format!("Resource not found: {address_str}"))
                    }
                    Ok(ResolveResult::Ambiguous(refs)) => {
                        let str_refs: Vec<String> =
                            refs.iter().map(|r| r.to_string()).collect();
                        text_err(format!(
                            "Ambiguous resolve '{address_str}': matches {str_refs:?}"
                        ))
                    }
                    Err(e) => struct_err(&e),
                }
            }
        })
    }

    #[tool(description = "Query resources in the space")]
    fn query(
        &self,
        Parameters(args): Parameters<QueryArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.with_service(|svc| {
            let mut selector: Selector = Selector::default();
            if let Some(kind_str) = args.kind.as_deref() {
                match kind_str {
                    "document" => selector.kind = Some(ResourceKind::Document),
                    "heading" => selector.kind = Some(ResourceKind::Heading),
                    "attachment" => selector.kind = Some(ResourceKind::Attachment),
                    other => return text_err(format!("Unknown resource kind: {other}")),
                }
            }
            if let Some(title_sub) = args.title_contains.as_deref() {
                selector.title_contains = Some(title_sub.to_owned());
            }
            match ResourceUseCase::query(svc, &selector) {
                Ok(page) => match serde_json::to_value(&page) {
                    Ok(v) => text_ok(v),
                    Err(e) => text_err(format!("Internal query serialization error: {e}")),
                },
                Err(e) => struct_err(&e),
            }
        })
    }

    #[tool(description = "Read details of a resource by ref")]
    fn read(
        &self,
        Parameters(args): Parameters<RefArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.with_service(|svc| {
            let r_ref = match ResourceRef::parse(&args.r#ref) {
                Ok(r) => r,
                Err(e) => {
                    return text_err(format!(
                        "Invalid resource ref '{}': {e}",
                        args.r#ref
                    ));
                }
            };
            match ResourceUseCase::read(svc, &r_ref) {
                Ok(Some(res)) => match serde_json::to_value(&res) {
                    Ok(v) => text_ok(v),
                    Err(e) => text_err(format!("Internal read serialization error: {e}")),
                },
                Ok(None) => text_err(format!("Resource not found: {}", args.r#ref)),
                Err(e) => struct_err(&e),
            }
        })
    }

    #[tool(description = "Inspect projection status and resources. With no ref, returns a project-level snapshot.")]
    fn inspect(
        &self,
        Parameters(args): Parameters<InspectArgs>,
    ) -> Result<CallToolResult, McpError> {
        // resolve_links + reindex_links need &mut self, so take the mutex
        // mutably. The link_list + diagnose_link + query variants are read-only
        // but we run them all through the same mutex to keep the snapshot
        // consistent.
        self.with_service_mut(|svc| {
            if let Some(ref_str) = args.r#ref.as_deref() {
                let parsed = match ResourceRef::parse(ref_str) {
                    Ok(r) => r,
                    Err(e) => return text_err(format!("Invalid resource ref '{ref_str}': {e}")),
                };
                let occs = match LinkUseCase::list_links(svc, &parsed) {
                    Ok(o) => o,
                    Err(e) => return struct_err(&e),
                };
                let resolved = match LinkUseCase::resolve_links(svc, &parsed) {
                    Ok(r) => r,
                    Err(e) => return struct_err(&e),
                };
                let diags = match LinkUseCase::diagnose_link(svc, &parsed) {
                    Ok(d) => d,
                    Err(e) => return struct_err(&e),
                };
                let mut by_status = serde_json::Map::new();
                for d in &diags {
                    let key = format!("{:?}", d.status);
                    let count = by_status.get(&key).and_then(|v| v.as_u64()).unwrap_or(0);
                    by_status.insert(key, serde_json::json!(count + 1));
                }
                text_ok(json!({
                    "ref": parsed.to_string(),
                    "occurrences": occs,
                    "resolved_relations": resolved,
                    "diagnostics": diags,
                    "occurrences_by_status": by_status,
                }))
            } else {
                let selector: Selector = Selector::default();
                match ResourceUseCase::query(svc, &selector) {
                    Ok(page) => text_ok(json!({
                        "status": "ok",
                        "total_items": page.items.len(),
                    })),
                    Err(e) => struct_err(&e),
                }
            }
        })
    }

    #[tool(description = "Inspect rule traces for a resource ref")]
    fn inspect_rules(
        &self,
        Parameters(args): Parameters<RefArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.with_service(|svc| {
            let r_ref = match ResourceRef::parse(&args.r#ref) {
                Ok(r) => r,
                Err(e) => {
                    return text_err(format!(
                        "Invalid resource ref '{}': {e}",
                        args.r#ref
                    ));
                }
            };
            match InspectUseCase::inspect_rules(svc, &r_ref) {
                Ok(Some(inspect_res)) => match serde_json::to_value(&inspect_res) {
                    Ok(v) => text_ok(v),
                    Err(e) => {
                        text_err(format!("Internal inspect_rules serialization error: {e}"))
                    }
                },
                Ok(None) => text_err(format!("Resource not found: {}", args.r#ref)),
                Err(e) => struct_err(&e),
            }
        })
    }

    #[tool(description = "List link occurrences for a resource")]
    fn link_list(
        &self,
        Parameters(args): Parameters<RefArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.with_service(|svc| {
            let r_ref = match ResourceRef::parse(&args.r#ref) {
                Ok(r) => r,
                Err(e) => {
                    return text_err(format!(
                        "Invalid resource ref '{}': {e}",
                        args.r#ref
                    ));
                }
            };
            match LinkUseCase::list_links(svc, &r_ref) {
                Ok(occs) => match serde_json::to_value(&occs) {
                    Ok(v) => text_ok(v),
                    Err(e) => text_err(format!("Internal link_list serialization error: {e}")),
                },
                Err(e) => struct_err(&e),
            }
        })
    }

    #[tool(description = "Re-resolve link occurrences for a resource and return relations")]
    fn link_resolve(
        &self,
        Parameters(args): Parameters<RefArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.with_service_mut(|svc| {
            let r_ref = match ResourceRef::parse(&args.r#ref) {
                Ok(r) => r,
                Err(e) => {
                    return text_err(format!(
                        "Invalid resource ref '{}': {e}",
                        args.r#ref
                    ));
                }
            };
            match LinkUseCase::resolve_links(svc, &r_ref) {
                Ok(rels) => match serde_json::to_value(&rels) {
                    Ok(v) => text_ok(v),
                    Err(e) => {
                        text_err(format!("Internal link_resolve serialization error: {e}"))
                    }
                },
                Err(e) => struct_err(&e),
            }
        })
    }

    #[tool(description = "Diagnose link occurrences (status + candidates) for a resource")]
    fn link_diagnose(
        &self,
        Parameters(args): Parameters<RefArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.with_service(|svc| {
            let r_ref = match ResourceRef::parse(&args.r#ref) {
                Ok(r) => r,
                Err(e) => {
                    return text_err(format!(
                        "Invalid resource ref '{}': {e}",
                        args.r#ref
                    ));
                }
            };
            match LinkUseCase::diagnose_link(svc, &r_ref) {
                Ok(diags) => match serde_json::to_value(&diags) {
                    Ok(v) => text_ok(v),
                    Err(e) => {
                        text_err(format!("Internal link_diagnose serialization error: {e}"))
                    }
                },
                Err(e) => struct_err(&e),
            }
        })
    }

    #[tool(description = "Reindex link status counts for the whole space")]
    fn link_reindex(
        &self,
        Parameters(args): Parameters<SpaceArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.with_service_mut(|svc| {
            let space = space_path(args.space.as_deref());
            match LinkUseCase::reindex_links(svc, &space) {
                Ok(report) => match serde_json::to_value(&report) {
                    Ok(v) => text_ok(v),
                    Err(e) => {
                        text_err(format!("Internal link_reindex serialization error: {e}"))
                    }
                },
                Err(e) => struct_err(&e),
            }
        })
    }

    #[tool(description = "Show the agenda (tasks + scheduled + deadline windows)")]
    fn agenda(&self) -> Result<CallToolResult, McpError> {
        self.with_service(|svc| match TaskUseCase::agenda(svc) {
            Ok(agenda) => match serde_json::to_value(&agenda) {
                Ok(v) => text_ok(v),
                Err(e) => text_err(format!("Internal agenda serialization error: {e}")),
            },
            Err(e) => struct_err(&e),
        })
    }

    #[tool(description = "Transition task state")]
    fn task_transition(
        &self,
        Parameters(args): Parameters<TaskTransitionArgs>,
    ) -> Result<CallToolResult, McpError> {
        let r_ref = match ResourceRef::parse(&args.r#ref) {
            Ok(r) => r,
            Err(e) => {
                return text_err(format!(
                    "Invalid resource ref '{}': {e}",
                    args.r#ref
                ));
            }
        };
        let timestamp = args.timestamp.as_deref().unwrap_or("2026-07-22 Wed 16:00");
        self.with_service_mut(|svc| match TaskUseCase::transition_task(svc, &r_ref, &args.to, timestamp) {
            Ok(transition) => match serde_json::to_value(&transition) {
                Ok(v) => text_ok(v),
                Err(e) => text_err(format!("Internal transition serialization error: {e}")),
            },
            Err(e) => struct_err(&e),
        })
    }

    #[tool(description = "List configured sources in the space")]
    fn source_list(
        &self,
        Parameters(args): Parameters<SpaceArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.with_service(|svc| {
            let space = space_path(args.space.as_deref());
            match svc.list_sources(&space) {
                Ok(sources) => match serde_json::to_value(&sources) {
                    Ok(v) => text_ok(v),
                    Err(e) => text_err(format!("Internal source_list serialization error: {e}")),
                },
                Err(e) => struct_err(&e),
            }
        })
    }

    #[tool(description = "Add an external source to the space")]
    fn source_add(
        &self,
        Parameters(args): Parameters<SourceAddArgs>,
    ) -> Result<CallToolResult, McpError> {
        let kind = match args.kind.as_str() {
            "native" => notez_core::source::SourceKind::Native,
            "git" => notez_core::source::SourceKind::Git,
            "obsidian" => notez_core::source::SourceKind::Obsidian,
            other => return text_err(format!("Unknown source kind: {other}")),
        };
        let include_paths = args
            .include_paths
            .unwrap_or_default()
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect();
        let exclude_paths = args
            .exclude_paths
            .unwrap_or_default()
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect();
        let config = notez_core::source::SourceConfig {
            id: args.id.clone(),
            kind,
            path: std::path::PathBuf::from(&args.path),
            read_only: args.read_only.unwrap_or(false),
            include_paths,
            exclude_paths,
        };
        let space = space_path(args.space.as_deref());
        self.with_service_mut(|svc| match svc.add_source(&space, config) {
            Ok(_) => text_ok(json!({ "added": true })),
            Err(e) => struct_err(&e),
        })
    }

    #[tool(description = "Add an attachment file to space")]
    fn attachment_add(
        &self,
        Parameters(args): Parameters<AttachmentAddArgs>,
    ) -> Result<CallToolResult, McpError> {
        let space = space_path(args.space.as_deref());
        let file_path = std::path::Path::new(&args.path);
        let default_mime = args.mime.as_deref().unwrap_or("application/octet-stream");
        self.with_service_mut(|svc| match AttachmentUseCase::add_attachment(svc, &space, file_path, default_mime) {
            Ok(att_ref) => text_ok(json!({ "ref": att_ref.to_string() })),
            Err(e) => struct_err(&e),
        })
    }

    #[tool(description = "Run text/metadata extraction job on attachment ref")]
    fn attachment_extract(
        &self,
        Parameters(args): Parameters<AttachmentExtractArgs>,
    ) -> Result<CallToolResult, McpError> {
        let space = space_path(args.space.as_deref());
        let r_ref = match ResourceRef::parse(&args.r#ref) {
            Ok(r) => r,
            Err(e) => {
                return text_err(format!(
                    "Invalid resource ref '{}': {e}",
                    args.r#ref
                ));
            }
        };
        self.with_service_mut(|svc| match AttachmentUseCase::run_extraction(svc, &space, &r_ref) {
            Ok(segments) => text_ok(json!({
                "attachment_ref": args.r#ref,
                "segments_count": segments.len(),
            })),
            Err(e) => struct_err(&e),
        })
    }

    #[tool(description = "Query extracted text segments for attachment ref")]
    fn query_segments(
        &self,
        Parameters(args): Parameters<RefArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.with_service(|svc| {
            let r_ref = match ResourceRef::parse(&args.r#ref) {
                Ok(r) => r,
                Err(e) => {
                    return text_err(format!(
                        "Invalid resource ref '{}': {e}",
                        args.r#ref
                    ));
                }
            };
            match AttachmentUseCase::query_segments(svc, &r_ref) {
                Ok(segments) => match serde_json::to_value(&segments) {
                    Ok(v) => text_ok(v),
                    Err(e) => {
                        text_err(format!("Internal query_segments serialization error: {e}"))
                    }
                },
                Err(e) => struct_err(&e),
            }
        })
    }

    #[tool(description = "Create a community in space")]
    fn community_create(
        &self,
        Parameters(args): Parameters<CommunityCreateArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut selector: Selector = Selector::default();
        if let Some(kind_str) = args.kind.as_deref() {
            match kind_str {
                "document" => selector.kind = Some(ResourceKind::Document),
                "heading" => selector.kind = Some(ResourceKind::Heading),
                "attachment" => selector.kind = Some(ResourceKind::Attachment),
                other => return text_err(format!("Unknown resource kind: {other}")),
            }
        }
        if let Some(title_sub) = args.title_contains.as_deref() {
            selector.title_contains = Some(title_sub.to_owned());
        }
        let comm = notez_core::domain::community::Community {
            id: args.id.clone(),
            name: args.name.clone(),
            selector,
            pinned_members: vec![],
            excluded_members: vec![],
        };
        let space = space_path(args.space.as_deref());
        self.with_service_mut(|svc| match CommunityUseCase::create_community(svc, &space, comm) {
            Ok(_) => text_ok(json!({ "created": true })),
            Err(e) => struct_err(&e),
        })
    }

    #[tool(description = "Derive a recipe artifact (summary, llms.txt, context-pack, skill-ir)")]
    fn derive_artifact(
        &self,
        Parameters(args): Parameters<DeriveArtifactArgs>,
    ) -> Result<CallToolResult, McpError> {
        let space = space_path(args.space.as_deref());
        let recipe_name = args.recipe.clone();
        self.with_service_mut(|svc| {
            match ArtifactUseCase::derive_artifact(svc, &space, &args.community, &recipe_name) {
                Ok(derived) => text_ok(json!({
                    "recipe": recipe_name,
                    "content": derived.content,
                })),
                Err(e) => struct_err(&e),
            }
        })
    }

    #[tool(description = "Export a SKILL.md package for a community")]
    fn export_skill(
        &self,
        Parameters(args): Parameters<ExportSkillArgs>,
    ) -> Result<CallToolResult, McpError> {
        let space = space_path(args.space.as_deref());
        let description = args.description.as_deref().unwrap_or("Exported Agent Skill");
        let out_path = std::path::Path::new(&args.out);
        self.with_service_mut(|svc| {
            match ArtifactUseCase::export_skill(svc, &space, &args.community, description, out_path) {
                Ok(package) => text_ok(json!({
                    "name": package.name,
                    "path": package.package_path.to_string_lossy(),
                })),
                Err(e) => struct_err(&e),
            }
        })
    }

    #[tool(description = "Push space changes to shared folder")]
    fn sync_push(
        &self,
        Parameters(args): Parameters<SyncArgs>,
    ) -> Result<CallToolResult, McpError> {
        let space = space_path(args.space.as_deref());
        let actor = args.actor.as_deref().unwrap_or("mcp_actor");
        let folder_path = std::path::Path::new(&args.folder);
        self.with_service_mut(|svc| match SyncUseCase::sync_push(svc, actor, &space, folder_path) {
            Ok(report) => text_ok(json!({
                "pushed_files": report.pushed_files,
                "pushed_objects": report.pushed_objects,
            })),
            Err(e) => struct_err(&e),
        })
    }

    #[tool(description = "Pull changes from shared folder into space")]
    fn sync_pull(
        &self,
        Parameters(args): Parameters<SyncArgs>,
    ) -> Result<CallToolResult, McpError> {
        let space = space_path(args.space.as_deref());
        let actor = args.actor.as_deref().unwrap_or("mcp_actor");
        let folder_path = std::path::Path::new(&args.folder);
        self.with_service_mut(|svc| match SyncUseCase::sync_pull(svc, actor, &space, folder_path) {
            Ok(report) => text_ok(json!({
                "pulled_files": report.pulled_files,
                "merged_files": report.merged_files,
                "conflicts_count": report.conflicts.len(),
            })),
            Err(e) => struct_err(&e),
        })
    }

    #[tool(description = "List active sync conflicts in space")]
    fn sync_conflicts(&self) -> Result<CallToolResult, McpError> {
        self.with_service(|svc| match SyncUseCase::list_conflicts(svc) {
            Ok(conflicts) => match serde_json::to_value(&conflicts) {
                Ok(v) => text_ok(v),
                Err(e) => {
                    text_err(format!("Internal sync_conflicts serialization error: {e}"))
                }
            },
            Err(e) => struct_err(&e),
        })
    }

    #[tool(description = "Run space integrity diagnostics")]
    fn space_doctor(
        &self,
        Parameters(args): Parameters<SpaceArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.with_service(|svc| {
            let space = space_path(args.space.as_deref());
            match InspectUseCase::space_doctor(svc, &space) {
                Ok(report) => match serde_json::to_value(&report) {
                    Ok(v) => text_ok(v),
                    Err(e) => {
                        text_err(format!("Internal space_doctor serialization error: {e}"))
                    }
                },
                Err(e) => struct_err(&e),
            }
        })
    }

    #[tool(description = "List background jobs and tasks")]
    fn job_list(&self) -> Result<CallToolResult, McpError> {
        self.with_service(|svc| match InspectUseCase::list_jobs(svc) {
            Ok(jobs) => match serde_json::to_value(&jobs) {
                Ok(v) => text_ok(v),
                Err(e) => text_err(format!("Internal job_list serialization error: {e}")),
            },
            Err(e) => struct_err(&e),
        })
    }

    #[tool(description = "Check artifact freshness against source files")]
    fn artifact_stale(
        &self,
        Parameters(args): Parameters<SpaceArgs>,
    ) -> Result<CallToolResult, McpError> {
        self.with_service(|svc| {
            let space = space_path(args.space.as_deref());
            match InspectUseCase::check_artifact_freshness(svc, &space) {
                Ok(report) => match serde_json::to_value(&report) {
                    Ok(v) => text_ok(v),
                    Err(e) => {
                        text_err(format!("Internal artifact_stale serialization error: {e}"))
                    }
                },
                Err(e) => struct_err(&e),
            }
        })
    }

    /// List the capabilities the current build exposes. The payload is
    /// byte-equal to the JSON printed by the `notez list-capabilities`
    /// CLI subcommand; both call into the same `ApplicationFacade`.
    #[tool(description = "List the capabilities the current build exposes")]
    fn list_capabilities(&self) -> Result<CallToolResult, McpError> {
        self.with_service(|svc| text_ok(svc.capabilities_json()))
    }

}
#[tool_handler]
impl ServerHandler for NotezMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "notez-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_protocol_version(ProtocolVersion::LATEST)
            .with_instructions(
                "notez MCP server. Tools are callable immediately after `initialize`; \
                 `notifications/initialized` is not required.",
            )
    }
}

// Silence unused warnings for the `Json` re-export carried over from the
// legacy server. Available for any future structured-output tools.
#[allow(dead_code)]
fn _unused_reexports() {
    let _ = std::any::type_name::<Json<()>>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Serve MCP over stdin/stdout. Blocks until the peer disconnects.
pub async fn serve(service: ApplicationService<SqliteProjection>) -> anyhow::Result<()> {
    let server = NotezMcpServer::new(service);
    let (stdin, stdout) = rmcp::transport::io::stdio();
    let running: rmcp::service::RunningService<rmcp::RoleServer, NotezMcpServer> =
        server.serve((stdin, stdout)).await?;
    let _ = running.waiting().await?;
    Ok(())
}