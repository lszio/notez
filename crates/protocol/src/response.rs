//! Typed response vocabulary — one operation, one payload struct.
//!
//! Mirrors the payload shapes the engine (`core::application`
//! use-cases) produces today. Wire compatibility rule: every field
//! name and serde representation matches what the previous
//! `serde_json` serialization of the corresponding core/domain type
//! emitted, so existing JSON consumers keep working across the
//! migration.
//!
//! Representation notes:
//!
//! * `ResourceRef`, `ObjectIdentity`, and resource `kind`s are
//!   plain strings here — the domain serializes them as strings;
//! * enums mirror the domain serde naming (`snake_case`, or the
//!   externally tagged default where the domain does not rename);
//! * every payload derives `JsonSchema` so surfaces can advertise
//!   response shapes without hand-written copies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---- shared primitives -----------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Document,
    Heading,
    Attachment,
    Block,
}

/// Domain `Selector` mirror (also nested inside [`Community`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Selector {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ResourceKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_contains: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exact_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    /// Row cap pushed down into the store query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

/// Domain `Resource` mirror. Serialized as `{"ref": "<kind>:<ulid>", ...}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Resource {
    #[serde(rename = "ref")]
    pub ref_: String,
    pub kind: ResourceKind,
    pub title: String,
    pub revision: String,
    pub source_id: String,
    pub locator: String,
    pub properties: BTreeMap<String, String>,
    /// `authority::native_id` string form; empty when unknown.
    #[serde(default)]
    pub object_id: String,
    #[serde(default)]
    pub primary_source_id: String,
}

/// Domain `QueryPage` mirror.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QueryPage {
    pub items: Vec<Resource>,
    pub next_cursor: Option<String>,
}

// ---- payloads per operation --------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ScanReport {
    pub scanned_files: usize,
    pub scanned_resources: usize,
    pub scanned_relations: usize,
    /// Files rejected by the source scan policy, by reason. Zero
    /// total means no rejection, not that no file exists.
    #[serde(default)]
    pub ignored: u32,
    #[serde(default)]
    pub ignored_hidden: u32,
    #[serde(default)]
    pub ignored_excluded: u32,
    #[serde(default)]
    pub ignored_not_included: u32,
    #[serde(default)]
    pub ignored_too_large: u32,
    #[serde(default)]
    pub ignored_symlink: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SegmentRecord {
    pub id: String,
    pub attachment_ref: String,
    pub text: String,
    pub offset_start: usize,
    pub offset_end: usize,
}

// ---- links -------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    References,
    Embeds,
    Backlink,
    Custom(String),
}

impl Default for RelationType {
    fn default() -> Self {
        Self::References
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RelationDirection {
    Forward,
    Backward,
    Unknown,
}

impl Default for RelationDirection {
    fn default() -> Self {
        Self::Unknown
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LinkTarget {
    Id {
        value: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind_hint: Option<ResourceKind>,
    },
    File {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fragment: Option<String>,
    },
    Title {
        title: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fragment: Option<String>,
    },
    Url {
        url: String,
    },
    Custom {
        scheme: String,
        value: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fragment: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct TextSpan {
    pub line: usize,
    pub col_start: usize,
    pub col_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LinkOccurrence {
    pub source_ref: String,
    pub target: LinkTarget,
    pub raw: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_text: Option<String>,
    pub span: TextSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStatus {
    Resolved,
    Ambiguous,
    Unresolved,
    External,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolvedRelation {
    pub source_ref: String,
    pub target_ref: String,
    pub target: LinkTarget,
    pub status: ResolutionStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<String>,
    #[serde(default)]
    pub relation_type: RelationType,
    #[serde(default)]
    pub direction: RelationDirection,
    #[serde(default = "default_evidence_json")]
    pub evidence_json: serde_json::Value,
    #[serde(default)]
    pub created_at: String,
    #[serde(default = "default_creator")]
    pub creator: String,
}

fn default_evidence_json() -> serde_json::Value {
    serde_json::json!({})
}
fn default_creator() -> String {
    "legacy".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LinkDiagnostic {
    pub occurrence: LinkOccurrence,
    pub status: ResolutionStatus,
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LinkReindexReport {
    pub scanned: usize,
    pub resolved: usize,
    pub unresolved: usize,
    pub ambiguous: usize,
    pub external: usize,
    pub invalid: usize,
}

// ---- tasks -------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgendaItem {
    pub r_ref: String,
    pub title: String,
    pub todo: Option<String>,
    pub scheduled: Option<String>,
    pub deadline: Option<String>,
    pub closed: Option<String>,
    pub locator: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgendaView {
    pub items: Vec<AgendaItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParaNode {
    pub resource: Resource,
    pub tasks: Vec<AgendaItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParaOverview {
    pub projects: Vec<ParaNode>,
    pub areas: Vec<ParaNode>,
    pub resources: Vec<ParaNode>,
    pub archives: Vec<ParaNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StateTransition {
    pub from_state: String,
    pub to_state: String,
    pub closed_timestamp: Option<String>,
    pub logbook_entry: String,
}

// ---- communities / artifacts -------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Community {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub selector: Selector,
    #[serde(default)]
    pub pinned_members: Vec<String>,
    #[serde(default)]
    pub excluded_members: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RecipeKind {
    Summary,
    LlmsTxt,
    ContextPack,
    SkillIr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DerivedArtifact {
    pub recipe_name: String,
    pub kind: RecipeKind,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SkillPackage {
    pub name: String,
    /// Absolute export directory path (domain `PathBuf` string form).
    pub package_path: String,
}

// ---- inspect -----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    Classify,
    Validate,
    Derive,
    React,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RuleTrace {
    pub rule_id: String,
    pub kind: RuleKind,
    pub matched: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InspectResult {
    pub r_ref: String,
    pub classified_type: Option<String>,
    pub derived_properties: BTreeMap<String, String>,
    pub traces: Vec<RuleTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DoctorIssue {
    pub severity: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DoctorReport {
    pub status: String,
    pub issues: Vec<DoctorIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct JobRecord {
    pub job_id: String,
    pub job_type: String,
    pub target_ref: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactStaleReport {
    pub status: String,
    pub stale_artifacts: Vec<String>,
}

// ---- sync --------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PushReport {
    pub pushed_files: usize,
    pub pushed_manifests: usize,
    pub pushed_objects: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PullReport {
    pub pulled_files: usize,
    pub merged_files: usize,
    pub conflicts: Vec<ConflictRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConflictRecord {
    pub logical_path: String,
    pub mine_hash: String,
    pub theirs_hash: String,
    pub conflict_text: String,
    pub status: String,
    pub detected_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RelaySyncReport {
    pub source_id: String,
    pub synced_via_relay: bool,
}

/// Combined push+pull snapshot used by the `sync_reports` response
/// variant. Defined here because the variant lives in [`Response`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SyncReportPair {
    pub push: PushReport,
    pub pull: PullReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WritebackReport {
    pub target_ref: String,
    pub committed: bool,
}

/// Result of a successful `update_document`: the re-read row's identity
/// plus its post-write revision (content hash).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentUpdateReport {
    pub r_ref: String,
    pub locator: String,
    pub revision: String,
}

/// Successful restricted Janet evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct JanetResult {
    pub value: serde_json::Value,
}

/// One notez card projection — the stable wire shape every surface
/// consumes (Web SSR, CLI, future remote clients). Mirrors the engine
/// projection so renderers never need to peek at engine internals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CardProjection {
    pub id: String,
    pub title: String,
    pub language: String,
    pub locator: String,
    pub ordinal: usize,
    /// `ready` | `failed` | `timeout` | `stale` | `denied`.
    pub state: String,
    /// `json` | `list` | `object` | `html`; empty for failed states.
    pub output_type: String,
    /// Json/List bodies (typed). `None` for object/html/failed.
    pub output: Option<serde_json::Value>,
    /// Remote/local object reference (`object` output only).
    pub object_ref: Option<String>,
    /// Sanitized HTML (`html` output only).
    pub html: Option<String>,
    /// Failure detail (non-ready states only).
    pub error: Option<String>,
    pub error_kind: Option<String>,
}

/// Ordered + visible card layout persisted per dashboard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct DashboardLayout {
    /// All known card ids in display order (visible + hidden).
    pub ordered_card_ids: Vec<String>,
    /// Card ids currently hidden from the dashboard.
    pub hidden_card_ids: Vec<String>,
}

// ---- resolution ---------------------------------------------------------------

/// Externally tagged exactly like the domain `ResolveResult` (no
/// rename): `{"Found": "..."} | "NotFound" | {"Ambiguous": [...]}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ResolveResult {
    Found(String),
    NotFound,
    Ambiguous(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SpaceSummary {
    pub space_id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DocumentSummary {
    pub source_id: String,
    pub document_path: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObjectSummary {
    pub address: crate::request::ObjectAddress,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GraphResult {
    pub object: crate::request::ObjectAddress,
    pub neighbors: Vec<ObjectSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WatchStatus {
    pub source_id: String,
    pub active: bool,
    pub last_batch_id: Option<String>,
}

// ---- envelope -------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CommandResult {
    Applied { new_revision: String },
    StaleRevision { expected: String, current: String },
    Conflict { message: String },
    Forbidden { message: String },
    Unsupported { capability: String },
}

/// The complete result set. Tagged with `result_of` so clients can
/// discriminate without guessing by shape; variant/payload pairing is
/// one-to-one with the request `op`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "result_of", rename_all = "snake_case")]
pub enum Response {
    CommandResult(CommandResult),
    Space(SpaceSummary),
    Document(DocumentSummary),
    Object(ObjectSummary),
    Graph(GraphResult),
    Summary(SpaceSummary),
    Watch(WatchStatus),
    Scan(ScanReport),
    ResourcePage(QueryPage),
    /// `read`: present-or-absent single resource.
    Resource(Option<Resource>),
    Resources(Vec<Resource>),
    Resolve(ResolveResult),
    Occurrences(Vec<LinkOccurrence>),
    Relations(Vec<ResolvedRelation>),
    Diagnostics(Vec<LinkDiagnostic>),
    Reindex(LinkReindexReport),
    Agenda(AgendaView),
    Para(ParaOverview),
    Transition(StateTransition),
    /// Attachment ref string (`attachment:<ulid>`).
    AttachmentRef(String),
    Segments(Vec<SegmentRecord>),
    Communities(Vec<Community>),
    Derived(DerivedArtifact),
    Skill(SkillPackage),
    InspectRules(Option<InspectResult>),
    Doctor(DoctorReport),
    Jobs(Vec<JobRecord>),
    Freshness(ArtifactStaleReport),
    Pushed(PushReport),
    Pulled(PullReport),
    Relay(RelaySyncReport),
    Conflicts(Vec<ConflictRecord>),
    Writeback(WritebackReport),
    /// `update_document`: saved, rescanned, revision refreshed.
    DocumentUpdated(DocumentUpdateReport),
    /// One card projection (per the engine's stable wire shape).
    Card(CardProjection),
    /// Result of executing one card live: id, state, and typed body.
    CardExecution(CardProjection),
    /// List of card projections for the current dashboard layout.
    Dashboard {
        cards: Vec<CardProjection>,
    },
    /// Updated dashboard layout (echoed after `update_dashboard`).
    DashboardUpdate(DashboardLayout),
    Janet(JanetResult),
    InboxCaptured {
        r_ref: String,
        revision: String,
    },
    Done,
}
