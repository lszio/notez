//! notez MCP stdio server.
//!
//! Built on the `rmcp` SDK. Speaks MCP protocol version `2025-11-25`
//! (rmcp's `LATEST`). All tools are callable immediately after `initialize`
//! returns; the server does NOT track `notifications/initialized` and does
//! NOT gate tool calls on any client handshake.
//!
//! Tool execution flow (M1 protocol unification): each tool deserializes its
//! raw JSON arguments into the matching [`notez_protocol`] request struct —
//! accepting both the historical MCP argument names (`ref`, `to`,
//! `community`, …) and the canonical protocol names — wraps it in a
//! [`Request`] and hands it to the engine's [`ApplicationDispatcher`], the
//! sole interpreter of the protocol (ref/kind parsing, write checks). On
//! success the tool serializes the *unwrapped* payload from the [`Response`]
//! envelope, so every tool's text output stays wire-compatible with earlier
//! A few service-level tools (`source_list`, `source_add`, `watch_*`,
//! `list_capabilities`) have no protocol request yet and keep calling the
//! application service directly. `source_doctor` is protocol-backed.
//!
//! Input schemas advertised via `tools/list` are generated from the protocol
//! request types with `schemars::schema_for!`; there are no hand-written
//! `JsonSchema` impls left in this module.

use std::collections::BTreeMap;
use std::sync::{Arc, LazyLock, Mutex};

use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_core::application::ApplicationService;
use notez_core::storage::SqliteProjection;
use notez_protocol::response::ResolveResult;
use notez_protocol::request::{
    AddAttachmentRequest, AgendaRequest, ArtifactFreshnessRequest, CreateCommunityRequest,
    DeriveArtifactRequest, DiagnoseLinkRequest, ExecuteJanetRequest, ExportSkillRequest,
    ExtractAttachmentRequest, InspectRulesRequest, ListConflictsRequest, ListJobsRequest,
    ListLinksRequest, QueryResourcesRequest, QuerySegmentsRequest, ReadResourceRequest,
    ReindexLinksRequest, Request, ResolveLinksRequest, ResolveRequest, SourceDoctorRequest,
    SyncPullRequest, SyncPushRequest, TransitionTaskRequest,
};
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};

// ─────────────────────────────────────────────────────────────────────────────
// Service-level argument DTOs
//
// Tools that have no protocol request yet keep small local argument structs.
// They are plain `#[derive(schemars::JsonSchema)]`: the workspace no longer
// shadows `::core`, so derived schemas work everywhere.
// ─────────────────────────────────────────────────────────────────────────────

/// Arguments for tools that address a space by filesystem path.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct SpaceArgs {
    /// Filesystem path to the space root. Defaults to `.` when omitted.
    pub space: Option<String>,
}

/// Arguments for `watch_status` — a space path and an event limit.
#[derive(Debug, Deserialize, Default, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct WatchStatusArgs {
    /// Optional space name or absolute path. Defaults to the active space.
    pub space: Option<String>,
    /// Maximum number of recent events to return. Default 50.
    pub limit: Option<usize>,
}

/// Arguments for `source_add`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
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

// ─────────────────────────────────────────────────────────────────────────────
// tools/list input schemas
//
// Generated once from the protocol request types — one schema, one source of
// truth. Tools whose handler takes raw `Value` arguments (to accept legacy
// argument names) get their real schemas patched in here; without this they
// would advertise a catch-all object.
// ─────────────────────────────────────────────────────────────────────────────

static TOOL_SCHEMAS: LazyLock<BTreeMap<&'static str, JsonObject>> = LazyLock::new(|| {
    fn schema_of<T: schemars::JsonSchema>() -> JsonObject {
        match serde_json::to_value(schemars::schema_for!(T)) {
            Ok(Value::Object(map)) => map,
            _ => unreachable!("struct schemas serialize to JSON objects"),
        }
    }
    fn object_map(value: Value) -> JsonObject {
        match value {
            Value::Object(map) => map,
            _ => unreachable!("literal schemas are JSON objects"),
        }
    }
    BTreeMap::from([
        ("agenda", schema_of::<AgendaRequest>()),
        ("artifact_stale", schema_of::<ArtifactFreshnessRequest>()),
        ("attachment_add", schema_of::<AddAttachmentRequest>()),
        ("attachment_extract", schema_of::<ExtractAttachmentRequest>()),
        ("community_create", schema_of::<CreateCommunityRequest>()),
        (
            "derive_artifact",
            schema_of::<DeriveArtifactRequest>(),
        ),
        ("export_skill", schema_of::<ExportSkillRequest>()),
        ("inspect_rules", schema_of::<InspectRulesRequest>()),
        ("job_list", schema_of::<ListJobsRequest>()),
        ("link_diagnose", schema_of::<DiagnoseLinkRequest>()),
        ("link_list", schema_of::<ListLinksRequest>()),
        ("link_reindex", schema_of::<ReindexLinksRequest>()),
        ("link_resolve", schema_of::<ResolveLinksRequest>()),
        (
            "list_capabilities",
            object_map(json!({ "type": "object" })),
        ),
        (
            // Composite snapshot: an optional ref plus the query filters.
            "inspect",
            object_map(json!({
                "type": "object",
                "properties": {
                    "ref": {
                        "type": "string",
                        "description": "Optional resource ref. When present, returns occurrences, resolved relations, and diagnostics for that resource."
                    },
                    "source_ref": {
                        "type": "string",
                        "description": "Canonical spelling of `ref`."
                    },
                    "kind": { "type": "string" },
                    "title_contains": { "type": "string" },
                    "exact_ref": { "type": "string" },
                    "source_id": { "type": "string" },
                    "limit": { "type": "integer" }
                }
            })),
        ),
        ("query", schema_of::<QueryResourcesRequest>()),
        ("query_segments", schema_of::<QuerySegmentsRequest>()),
        ("execute_janet", schema_of::<ExecuteJanetRequest>()),
        ("read", schema_of::<ReadResourceRequest>()),
        ("resolve", schema_of::<ResolveRequest>()),
        ("source_doctor", schema_of::<SourceDoctorRequest>()),
        ("source_add", schema_of::<SourceAddArgs>()),
        ("source_list", schema_of::<SpaceArgs>()),
        ("sync_conflicts", schema_of::<ListConflictsRequest>()),
        ("sync_pull", schema_of::<SyncPullRequest>()),
        ("sync_push", schema_of::<SyncPushRequest>()),
        ("task_transition", schema_of::<TransitionTaskRequest>()),
        ("watch_start", schema_of::<SpaceArgs>()),
        ("watch_status", schema_of::<WatchStatusArgs>()),
        ("watch_stop", schema_of::<SpaceArgs>()),
    ])
});

// ─────────────────────────────────────────────────────────────────────────────
// Result helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a JSON value into a text content block wrapped in a successful
/// tool result.
fn text_ok(value: Value) -> Result<CallToolResult, McpError> {
    let text =
        serde_json::to_string(&value).map_err(|e| McpError::internal_error(e.to_string(), None))?;
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
        notez_core::application::ApplicationError::AddressUniqueness { .. } => "address_uniqueness",
        notez_core::application::ApplicationError::InvalidRequest { .. } => "invalid_request",
        notez_core::application::ApplicationError::Janet { .. } => "janet",
    };
    text_err(serde_json::json!({ "kind": kind, "message": err.to_string() }).to_string())
}

/// Serialize a successful dispatcher payload as the tool's text content.
fn payload_ok<T: serde::Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    match serde_json::to_value(value) {
        Ok(v) => text_ok(v),
        Err(e) => text_err(format!("Internal serialization error: {e}")),
    }
}

/// Message for the impossible case of a response variant not matching the
/// dispatched request.
fn unexpected_response(resp: &Response) -> String {
    format!("Internal dispatcher error: unexpected response variant {resp:?}")
}

/// Deserialize raw tool arguments into a protocol request struct, first
/// translating legacy MCP argument names onto canonical protocol field
/// names. Clients depend on the historical spellings (`ref`, `to`,
/// `community`, …), so both are accepted; when both are present the
/// Raw tool arguments. rmcp requires the handler parameter type to
/// carry an object-typed JSON Schema; `serde_json::Value` alone does
/// not, so tools accepting free-form maps use this newtype.
#[derive(Debug, serde::Deserialize)]
struct RawArgs(Value);

impl schemars::JsonSchema for RawArgs {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("RawArgs")
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        serde_json::from_value(json!({
            "type": "object",
            "additionalProperties": true
        }))
        .expect("static object schema")
    }
}

/// canonical name wins. `fill` inserts defaults for fields the MCP surface
/// has always defaulted differently from the protocol.
fn parse_args<T: serde::de::DeserializeOwned>(
    args: &Value,
    renames: &[(&str, &str)],
    fill: &[(&str, &str)],
) -> Result<T, McpError> {
    let mut value = args.clone();
    if let Some(obj) = value.as_object_mut() {
        for &(from, to) in renames {
            if let Some(v) = obj.remove(from) {
                obj.entry(to.to_string()).or_insert(v);
            }
        }
        for &(key, default) in fill {
            obj.entry(key.to_string())
                .or_insert_with(|| Value::String(default.to_string()));
        }
    }
    serde_json::from_value(value)
        .map_err(|e| McpError::invalid_params(format!("Invalid tool arguments: {e}"), None))
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
    watch: Arc<notez_core::application::WatchService>,
    tool_router: ToolRouter<Self>,
}

impl NotezMcpServer {
    pub fn new(service: ApplicationService<SqliteProjection>) -> Self {
        Self {
            service: Arc::new(Mutex::new(service)),
            watch: notez_core::application::WatchService::new(),
            tool_router: Self::tool_router(),
        }
    }

    /// Run one protocol request through the engine dispatcher while holding
    /// the service mutex mutably (dispatch interprets writes too).
    fn dispatch(
        &self,
        req: Request,
    ) -> Result<Response, notez_core::application::ApplicationError> {
        self.with_service_mut(|svc| {
            let mut dispatcher = ApplicationDispatcher::new(svc);
            dispatcher.dispatch(req)
        })
    }

    fn with_service<R>(&self, f: impl FnOnce(&ApplicationService<SqliteProjection>) -> R) -> R {
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
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let req: ResolveRequest = parse_args(&args.0, &[], &[])?;
        let query = req.query.clone();
        match self.dispatch(Request::Resolve(req)) {
            Ok(Response::Resolve(result)) => match result {
                ResolveResult::Found(r_ref) => text_ok(json!({ "ref": r_ref.to_string() })),
                ResolveResult::NotFound => text_err(format!("Resource not found: {query}")),
                ResolveResult::Ambiguous(refs) => {
                    let str_refs: Vec<String> = refs.iter().map(|r| r.to_string()).collect();
                    text_err(format!(
                        "Ambiguous resolve '{query}': matches {str_refs:?}"
                    ))
                }
            },
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Query resources in the space")]
    fn query(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let req: QueryResourcesRequest = parse_args(&args.0, &[], &[])?;
        match self.dispatch(Request::QueryResources(req)) {
            Ok(Response::ResourcePage(page)) => payload_ok(&page),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Read details of a resource by ref")]
    fn read(&self, Parameters(args): Parameters<RawArgs>) -> Result<CallToolResult, McpError> {
        let req: ReadResourceRequest = parse_args(&args.0, &[("ref", "r_ref")], &[])?;
        let ref_str = req.r_ref.clone();
        match self.dispatch(Request::ReadResource(req)) {
            Ok(Response::Resource(Some(resource))) => payload_ok(&resource),
            Ok(Response::Resource(None)) => text_err(format!("Resource not found: {ref_str}")),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(
        description = "Inspect projection status and resources. With no ref, returns a project-level snapshot."
    )]
    fn inspect(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let raw_ref = args.0
            .get("ref")
            .or_else(|| args.0.get("source_ref"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let Some(ref_str) = raw_ref else {
            let req = QueryResourcesRequest {
                kind: None,
                title_contains: None,
                exact_ref: None,
                source_id: None,
                limit: None,
            };
            return match self.dispatch(Request::QueryResources(req)) {
                Ok(Response::ResourcePage(page)) => {
                    text_ok(json!({ "status": "ok", "total_items": page.items.len() }))
                }
                Ok(other) => text_err(unexpected_response(&other)),
                Err(e) => struct_err(&e),
            };
        };
        // resolve_links persists relations, so run the whole composite
        // snapshot through the mutable service view (the dispatcher takes
        // `&mut` anyway).
        let occs = match self.dispatch(Request::ListLinks(ListLinksRequest {
            source_ref: ref_str.clone(),
        })) {
            Ok(Response::Occurrences(occs)) => occs,
            Ok(other) => return text_err(unexpected_response(&other)),
            Err(e) => return struct_err(&e),
        };
        let resolved = match self.dispatch(Request::ResolveLinks(ResolveLinksRequest {
            source_ref: ref_str.clone(),
        })) {
            Ok(Response::Relations(rels)) => rels,
            Ok(other) => return text_err(unexpected_response(&other)),
            Err(e) => return struct_err(&e),
        };
        let diags = match self.dispatch(Request::DiagnoseLink(DiagnoseLinkRequest {
            source_ref: ref_str.clone(),
        })) {
            Ok(Response::Diagnostics(diags)) => diags,
            Ok(other) => return text_err(unexpected_response(&other)),
            Err(e) => return struct_err(&e),
        };
        let mut by_status = serde_json::Map::new();
        for d in &diags {
            let key = format!("{:?}", d.status);
            let count = by_status.get(&key).and_then(Value::as_u64).unwrap_or(0);
            by_status.insert(key, json!(count + 1));
        }
        text_ok(json!({
            "ref": ref_str,
            "occurrences": occs,
            "resolved_relations": resolved,
            "diagnostics": diags,
            "occurrences_by_status": by_status,
        }))
    }

    #[tool(description = "Inspect rule traces for a resource ref")]
    fn inspect_rules(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let req: InspectRulesRequest = parse_args(&args.0, &[("ref", "source_ref")], &[])?;
        let ref_str = req.source_ref.clone();
        match self.dispatch(Request::InspectRules(req)) {
            Ok(Response::InspectRules(Some(inspect_res))) => payload_ok(&inspect_res),
            Ok(Response::InspectRules(None)) => {
                text_err(format!("Resource not found: {ref_str}"))
            }
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "List link occurrences for a resource")]
    fn link_list(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let req: ListLinksRequest = parse_args(&args.0, &[("ref", "source_ref")], &[])?;
        match self.dispatch(Request::ListLinks(req)) {
            Ok(Response::Occurrences(occs)) => payload_ok(&occs),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Re-resolve link occurrences for a resource and return relations")]
    fn link_resolve(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let req: ResolveLinksRequest = parse_args(&args.0, &[("ref", "source_ref")], &[])?;
        match self.dispatch(Request::ResolveLinks(req)) {
            Ok(Response::Relations(rels)) => payload_ok(&rels),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Diagnose link occurrences (status + candidates) for a resource")]
    fn link_diagnose(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let req: DiagnoseLinkRequest = parse_args(&args.0, &[("ref", "source_ref")], &[])?;
        match self.dispatch(Request::DiagnoseLink(req)) {
            Ok(Response::Diagnostics(diags)) => payload_ok(&diags),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Reindex link status counts for the whole space")]
    fn link_reindex(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let _ = args;
        match self.dispatch(Request::ReindexLinks(ReindexLinksRequest {})) {
            Ok(Response::Reindex(report)) => payload_ok(&report),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Show the agenda (tasks + scheduled + deadline windows)")]
    fn agenda(&self, Parameters(args): Parameters<RawArgs>) -> Result<CallToolResult, McpError> {
        let _ = args;
        match self.dispatch(Request::Agenda(AgendaRequest {})) {
            Ok(Response::Agenda(agenda)) => payload_ok(&agenda),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Transition task state")]
    fn task_transition(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let req: TransitionTaskRequest =
            parse_args(&args.0, &[("ref", "r_ref"), ("to", "to_state")], &[])?;
        match self.dispatch(Request::TransitionTask(req)) {
            Ok(Response::Transition(transition)) => payload_ok(&transition),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    fn source_list(
        &self,
        Parameters(args): Parameters<SpaceArgs>,
    ) -> Result<CallToolResult, McpError> {
        if args.space.is_some() {
            return text_err("source_list does not support selecting a different space");
        }
        self.with_service(|svc| match svc.list_sources() {
            Ok(sources) => payload_ok(&sources),
            Err(e) => struct_err(&e),
        })
    }
    #[tool(description = "Add an external source to the space")]
    fn source_add(
        &self,
        Parameters(args): Parameters<SourceAddArgs>,
    ) -> Result<CallToolResult, McpError> {
        if args.space.is_some() {
            return text_err("source_add does not support selecting a different space");
        }
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
            url: None,
            read_only: args.read_only.unwrap_or(false),
            include_paths,
            exclude_paths,
        };
        self.with_service_mut(|svc| match svc.add_source(config) {
            Ok(_) => text_ok(json!({ "added": true })),
            Err(e) => struct_err(&e),
        })
    }

    #[tool(description = "Add an attachment file to space")]
    fn attachment_add(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let req: AddAttachmentRequest = parse_args(&args.0, &[("path", "file_path")], &[])?;
        match self.dispatch(Request::AddAttachment(req)) {
            Ok(Response::AttachmentRef(att_ref)) => {
                text_ok(json!({ "ref": att_ref.to_string() }))
            }
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Run text/metadata extraction job on attachment ref")]
    fn attachment_extract(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let req: ExtractAttachmentRequest = parse_args(&args.0, &[("ref", "source_ref")], &[])?;
        let ref_str = req.source_ref.clone();
        match self.dispatch(Request::ExtractAttachment(req)) {
            Ok(Response::Segments(segments)) => text_ok(json!({
                "attachment_ref": ref_str,
                "segments_count": segments.len(),
            })),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Query extracted text segments for attachment ref")]
    fn query_segments(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let req: QuerySegmentsRequest = parse_args(&args.0, &[("ref", "source_ref")], &[])?;
        match self.dispatch(Request::QuerySegments(req)) {
            Ok(Response::Segments(segments)) => payload_ok(&segments),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Create a community in space")]
    fn community_create(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let req: CreateCommunityRequest = parse_args(&args.0, &[], &[])?;
        match self.dispatch(Request::CreateCommunity(req)) {
            Ok(Response::Done) => text_ok(json!({ "created": true })),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Derive a recipe artifact (summary, llms.txt, context-pack, skill-ir)")]
    fn derive_artifact(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let req: DeriveArtifactRequest = parse_args(
            &args.0,
            &[("community", "community_id"), ("recipe", "recipe_name")],
            &[],
        )?;
        let recipe_name = req.recipe_name.clone();
        match self.dispatch(Request::DeriveArtifact(req)) {
            Ok(Response::Derived(artifact)) => text_ok(json!({
                "recipe": recipe_name,
                "content": artifact.content,
            })),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Export a SKILL.md package for a community")]
    fn export_skill(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let req: ExportSkillRequest = parse_args(
            &args.0,
            &[("community", "community_id"), ("out", "out_path")],
            &[],
        )?;
        match self.dispatch(Request::ExportSkill(req)) {
            Ok(Response::Skill(package)) => text_ok(json!({
                "name": package.name,
                "path": package.package_path,
            })),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Push space changes to shared folder")]
    fn sync_push(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        // The MCP surface has historically defaulted to `mcp_actor`; keep
        // that instead of the protocol's `default_actor`.
        let req: SyncPushRequest =
            parse_args(&args.0, &[("actor", "actor_id")], &[("actor_id", "mcp_actor")])?;
        match self.dispatch(Request::SyncPush(req)) {
            Ok(Response::Pushed(report)) => text_ok(json!({
                "pushed_files": report.pushed_files,
                "pushed_objects": report.pushed_objects,
            })),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Pull changes from shared folder into space")]
    fn sync_pull(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        // The MCP surface has historically defaulted to `mcp_actor`; keep
        // that instead of the protocol's `default_actor`.
        let req: SyncPullRequest =
            parse_args(&args.0, &[("actor", "actor_id")], &[("actor_id", "mcp_actor")])?;
        match self.dispatch(Request::SyncPull(req)) {
            Ok(Response::Pulled(report)) => text_ok(json!({
                "pulled_files": report.pulled_files,
                "merged_files": report.merged_files,
                "conflicts_count": report.conflicts.len(),
            })),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "List active sync conflicts in space")]
    fn sync_conflicts(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let _ = args;
        match self.dispatch(Request::ListConflicts(ListConflictsRequest {})) {
            Ok(Response::Conflicts(conflicts)) => payload_ok(&conflicts),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Run space integrity diagnostics")]
    fn source_doctor(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let _ = args;
        match self.dispatch(Request::SourceDoctor(SourceDoctorRequest {})) {
            Ok(Response::Doctor(report)) => payload_ok(&report),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "List background jobs and tasks")]
    fn job_list(&self, Parameters(args): Parameters<RawArgs>) -> Result<CallToolResult, McpError> {
        let _ = args;
        match self.dispatch(Request::ListJobs(ListJobsRequest {})) {
            Ok(Response::Jobs(jobs)) => payload_ok(&jobs),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    #[tool(description = "Check artifact freshness against source files")]
    fn artifact_stale(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let _ = args;
        match self.dispatch(Request::ArtifactFreshness(ArtifactFreshnessRequest {})) {
            Ok(Response::Freshness(report)) => payload_ok(&report),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(e) => struct_err(&e),
        }
    }

    /// List the capabilities the current build exposes. The payload is
    /// byte-equal to the JSON printed by the `notez list-capabilities`
    /// CLI subcommand; both call into the same `ApplicationFacade`.
    #[tool(description = "List the capabilities the current build exposes")]
    fn list_capabilities(&self) -> Result<CallToolResult, McpError> {
        self.with_service(|svc| text_ok(svc.capabilities_json()))
    }

    /// Execute a restricted, read-only Janet query through the protocol
    /// dispatcher. The MCP layer never constructs a Janet VM.
    #[tool(description = "Execute a restricted read-only Janet query")]
    fn execute_janet(
        &self,
        Parameters(args): Parameters<RawArgs>,
    ) -> Result<CallToolResult, McpError> {
        let req: ExecuteJanetRequest = parse_args(&args.0, &[], &[])?;
        match self.dispatch(Request::ExecuteJanet(req)) {
            Ok(Response::Janet(result)) => payload_ok(&result),
            Ok(other) => text_err(unexpected_response(&other)),
            Err(error) => struct_err(&error),
        }
    }

    #[tool(description = "Start watching a space root for filesystem events")]
    fn watch_start(
        &self,
        Parameters(args): Parameters<SpaceArgs>,
    ) -> Result<CallToolResult, McpError> {
        let space = space_path(args.space.as_deref());
        match self.watch.start(&space) {
            Ok(_) => text_ok(json!({ "started": true, "space": space.to_string_lossy() })),
            Err(e) => text_err(format!("watch start failed: {e}")),
        }
    }

    #[tool(description = "Stop watching a space root")]
    fn watch_stop(
        &self,
        Parameters(args): Parameters<SpaceArgs>,
    ) -> Result<CallToolResult, McpError> {
        let space = space_path(args.space.as_deref());
        let stopped = self.watch.stop(&space);
        text_ok(json!({ "stopped": stopped }))
    }

    #[tool(description = "Return the current watch state and recent events for a space root")]
    fn watch_status(
        &self,
        Parameters(args): Parameters<WatchStatusArgs>,
    ) -> Result<CallToolResult, McpError> {
        let space = space_path(args.space.as_deref());
        let limit = args.limit.unwrap_or(50);
        let status = self.watch.status(&space);
        let events = self.watch.events(&space, limit);
        text_ok(json!({
            "status": status,
            "events": events,
        }))
    }
}

#[tool_handler]
impl ServerHandler for NotezMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("notez-mcp", env!("CARGO_PKG_VERSION")))
            .with_protocol_version(ProtocolVersion::LATEST)
            .with_instructions(
                "notez MCP server. Tools are callable immediately after `initialize`; \
                 `notifications/initialized` is not required.",
            )
    }

    /// Advertise protocol-derived input schemas. Handlers take raw JSON to
    /// accept legacy argument names, so patch each tool's catch-all schema
    /// with the schema generated from its protocol request type.
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let mut tools = self.tool_router.list_all();
        for tool_def in &mut tools {
            if let Some(schema) = TOOL_SCHEMAS.get(tool_def.name.as_ref()) {
                tool_def.input_schema = Arc::new(schema.clone());
            }
        }
        Ok(ListToolsResult {
            meta: None,
            next_cursor: None,
            tools,
        })
    }

    /// Same schema patching as [`Self::list_tools`], for single-tool lookups.
    fn get_tool(&self, name: &str) -> Option<Tool> {
        let mut tool_def = self.tool_router.get(name).cloned()?;
        if let Some(schema) = TOOL_SCHEMAS.get(name) {
            tool_def.input_schema = Arc::new(schema.clone());
        }
        Some(tool_def)
    }
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
