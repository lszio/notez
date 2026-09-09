//! Request vocabulary — one operation, one struct.
//!
//! Every surface (CLI grammar, MCP tool input, HTTP body) translates
//! into these values. The engine (`core::application::dispatcher`)
//! is the sole interpreter: it parses stringly identifiers into
//! domain types, applies write checks, and returns typed results.
//!
//! Field policy:
//!
//! * identifiers (`r_ref`, `source_id`, paths) are strings;
//! * optional fields model surface optionality exactly once — a
//!   filter that exists here is available to *every* surface;
//! * `JsonSchema` is derived so MCP `tools/list` schemas are
//!   generated, never hand-written.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ---- scan -----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ScanNativeRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ScanFederationRequest {}

// ---- resource --------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct QueryResourcesRequest {
    /// Filter by resource kind (`document|heading|block|attachment`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_contains: Option<String>,
    /// Exact `ResourceRef` match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exact_ref: Option<String>,
    /// Restrict to one source adapter id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReadResourceRequest {
    pub r_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeleteResourceRequest {
    pub r_ref: String,
    /// Optimistic-concurrency guard on the row being deleted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ListRecentRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ListBySourceRequest {
    pub source_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Free-form address resolution: ULID, exact ref, path locator, or
/// title query — the engine picks the resolution rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResolveRequest {
    pub query: String,
}

/// Wire shape mirroring `core::domain::Resource`. Field names match
/// the domain serde names so existing JSON fixtures round-trip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ResourcePayload {
    /// `ResourceRef` string, e.g. `heading:01J0…`. Required: upsert
    /// addresses an existing projection row. Serialized as `ref` to
    /// match domain fixtures.
    #[serde(rename = "ref")]
    pub ref_: String,
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub revision: String,
    pub source_id: String,
    #[serde(default)]
    pub locator: String,
    #[serde(default)]
    pub properties: std::collections::BTreeMap<String, String>,
    /// Stable cross-space identity, e.g. `local::01J0…`. When
    /// omitted the engine stores the domain default identity,
    /// matching `Resource`'s serde contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    #[serde(default)]
    pub primary_source_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpsertResourceRequest {
    pub resource: ResourcePayload,
    /// Optimistic-concurrency guard for this upsert.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<String>,
}
/// Overwrite a whole document's content at `source_id` + `locator`.
/// Field names follow the existing web `update_document` entry-point
/// semantics: the target is addressed by source + relative locator
/// (not by ref), the write is guarded by a revision precondition, and
/// `format` optionally pins the parser (`markdown` | `org`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateDocumentRequest {
    pub source_id: String,
    /// Source-relative POSIX path of the document file.
    pub locator: String,
    pub content: String,
    /// Revision the caller loaded (content hash of the raw bytes).
    /// Alias of `expected_revision` kept for web parity; the engine
    /// falls back to it when `expected_revision` is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<String>,
}

/// Run a Janet script in the restricted read-only query runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExecuteJanetRequest {
    pub script: String,
    /// Optional source scope; empty means the engine's active source.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Optional document/resource scope (a protocol resource ref).
    #[serde(default)]
    pub document_ref: Option<String>,
    #[serde(default = "default_actor")]
    pub actor_id: String,
    #[serde(default)]
    pub expected_revision: Option<String>,
    #[serde(default)]
    pub trace_id: Option<String>,
    /// Wall-clock budget in milliseconds. The adapter may apply a tighter cap.
    #[serde(default = "default_janet_timeout_ms")]
    pub timeout_ms: u64,
    /// Maximum serialized result size in bytes.
    #[serde(default = "default_janet_result_limit")]
    pub result_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListCardsRequest {
    /// Source selector (`name` from the global config or absolute path).
    #[serde(default)]
    pub source: Option<String>,
    /// Page size; bounded by the dispatcher (default 32).
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadCardRequest {
    pub card_id: String,
    /// Source selector (`name` or path). Required so the dispatcher
    /// knows where to find the card definition.
    pub source: String,
    /// Document locator inside the source (file path relative to
    /// the source root).
    pub locator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecuteCardRequest {
    pub card_id: String,
    /// Source selector (`name` or path).
    pub source: String,
    /// Document locator inside the source.
    pub locator: String,
    /// Optional wall-clock budget override (ms).
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListDashboardRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateDashboardRequest {
    /// Ordered card ids, optionally including hidden ids so the
    /// ordering survives hide/show cycles.
    pub ordered_card_ids: Vec<String>,
    /// Hidden card ids; surfaces.
    #[serde(default)]
    pub hidden_card_ids: Vec<String>,
}

fn default_janet_timeout_ms() -> u64 {
    2_000
}
fn default_janet_result_limit() -> usize {
    256 * 1024
}

macro_rules! single_ref_request {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
        pub struct $name {
            pub source_ref: String,
        }
    };
}

single_ref_request!(
    LinkOccurrencesRequest,
    "Outgoing link occurrences of a resource."
);
single_ref_request!(
    ResolvedRelationsRequest,
    "Persisted resolved relations of a resource."
);
single_ref_request!(ListLinksRequest, "Raw link listing of a resource.");
single_ref_request!(
    ResolveLinksRequest,
    "Actively resolve and persist a resource's links."
);
single_ref_request!(
    DiagnoseLinkRequest,
    "Per-occurrence resolution diagnostics."
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReindexLinksRequest {}

// ---- tasks -------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AgendaRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ParaOverviewRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CaptureInboxItemRequest {
    pub source_id: String,
    pub content: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TransitionTaskRequest {
    pub r_ref: String,
    pub to_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<String>,
}

// ---- attachments ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AddAttachmentRequest {
    pub file_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
}

single_ref_request!(
    ExtractAttachmentRequest,
    "Run extraction jobs for an attachment."
);
single_ref_request!(
    QuerySegmentsRequest,
    "List extracted segments of an attachment."
);

// ---- communities -----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CreateCommunityRequest {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_contains: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ListCommunitiesRequest {}

// ---- artifacts --------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeriveArtifactRequest {
    pub community_id: String,
    pub recipe_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExportSkillRequest {
    pub community_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub out_path: String,
}

// ---- inspect -----------------------------------------------------------------------

single_ref_request!(
    InspectRulesRequest,
    "Evaluate stored rules against a resource."
);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SourceDoctorRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ListJobsRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactFreshnessRequest {}

// ---- sync ----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SyncPushRequest {
    #[serde(default = "default_actor")]
    pub actor_id: String,
    pub folder: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SyncPullRequest {
    #[serde(default = "default_actor")]
    pub actor_id: String,
    pub folder: String,
}

fn default_actor() -> String {
    "default_actor".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RelaySyncRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ListConflictsRequest {}

// ---- sources ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WritebackResourceRequest {
    pub source_id: String,
    pub r_ref: String,
    pub payload: String,
    /// Optimistic-concurrency guard on the resource being written back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<String>,
}
/// can POST `{"op": "query_resources", ...}` directly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    ScanNative(ScanNativeRequest),
    ScanFederation(ScanFederationRequest),
    QueryResources(QueryResourcesRequest),
    ReadResource(ReadResourceRequest),
    DeleteResource(DeleteResourceRequest),
    ListRecent(ListRecentRequest),
    ListBySource(ListBySourceRequest),
    Resolve(ResolveRequest),
    UpsertResource(UpsertResourceRequest),
    LinkOccurrences(LinkOccurrencesRequest),
    ResolvedRelations(ResolvedRelationsRequest),
    ListLinks(ListLinksRequest),
    ResolveLinks(ResolveLinksRequest),
    DiagnoseLink(DiagnoseLinkRequest),
    ReindexLinks(ReindexLinksRequest),
    Agenda(AgendaRequest),
    ParaOverview(ParaOverviewRequest),
    CaptureInboxItem(CaptureInboxItemRequest),
    TransitionTask(TransitionTaskRequest),
    AddAttachment(AddAttachmentRequest),
    ExtractAttachment(ExtractAttachmentRequest),
    QuerySegments(QuerySegmentsRequest),
    CreateCommunity(CreateCommunityRequest),
    ListCommunities(ListCommunitiesRequest),
    DeriveArtifact(DeriveArtifactRequest),
    ExportSkill(ExportSkillRequest),
    InspectRules(InspectRulesRequest),
    SourceDoctor(SourceDoctorRequest),
    ListJobs(ListJobsRequest),
    ArtifactFreshness(ArtifactFreshnessRequest),
    SyncPush(SyncPushRequest),
    SyncPull(SyncPullRequest),
    RelaySync(RelaySyncRequest),
    ListConflicts(ListConflictsRequest),
    WritebackResource(WritebackResourceRequest),
    UpdateDocument(UpdateDocumentRequest),
    ExecuteJanet(ExecuteJanetRequest),
    ListCards(ListCardsRequest),
    ReadCard(ReadCardRequest),
    ExecuteCard(ExecuteCardRequest),
    ListDashboard(ListDashboardRequest),
    UpdateDashboard(UpdateDashboardRequest),
}

// ---- unified command/query contracts ---------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObjectAddress {
    Stable {
        object_id: String,
    },
    Positioned {
        space_id: String,
        source_id: String,
        document_path: String,
        locator: String,
        /// Content fingerprint guards positioned identity against drift.
        fingerprint: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "query", rename_all = "snake_case")]
pub enum GraphQuery {
    Neighbors {
        principal: String,
        object: ObjectAddress,
        scope: String,
        #[serde(default = "default_graph_depth")]
        depth: u32,
    },
}

fn default_graph_depth() -> u32 {
    1
}

fn deserialize_non_empty<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.is_empty() {
        return Err(serde::de::Error::custom(
            "expected_revision must not be empty",
        ));
    }
    Ok(value)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WatchChange {
    pub kind: String,
    pub locator: String,
    pub observed_revision: String,
    pub observed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IngestWatchBatch {
    pub principal: String,
    pub space_id: String,
    pub source_id: String,
    pub batch_id: String,
    #[serde(deserialize_with = "deserialize_non_empty")]
    pub expected_revision: String,
    #[serde(rename = "events")]
    pub events: Vec<WatchChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    RegisterSpace {
        principal: String,
        space_id: String,
        name: String,
        #[serde(deserialize_with = "deserialize_non_empty")]
        expected_revision: String,
    },
    RemoveSpace {
        principal: String,
        space_id: String,
        #[serde(deserialize_with = "deserialize_non_empty")]
        expected_revision: String,
    },
    RegisterSource {
        principal: String,
        space_id: String,
        source_id: String,
        kind: String,
        #[serde(deserialize_with = "deserialize_non_empty")]
        expected_revision: String,
    },
    AttachSource {
        principal: String,
        space_id: String,
        source_id: String,
        #[serde(deserialize_with = "deserialize_non_empty")]
        expected_revision: String,
    },
    ScanSpace {
        principal: String,
        space_id: String,
        #[serde(deserialize_with = "deserialize_non_empty")]
        expected_revision: String,
    },
    IngestWatch(IngestWatchBatch),
    CreateDocument {
        principal: String,
        space_id: String,
        source_id: String,
        document_path: String,
        content: String,
        #[serde(deserialize_with = "deserialize_non_empty")]
        expected_revision: String,
    },
    UpdateDocument {
        principal: String,
        space_id: String,
        source_id: String,
        document_path: String,
        content: String,
        #[serde(deserialize_with = "deserialize_non_empty")]
        expected_revision: String,
    },
    CreateObject {
        principal: String,
        space_id: String,
        address: ObjectAddress,
        content: String,
        #[serde(deserialize_with = "deserialize_non_empty")]
        expected_revision: String,
    },
    WriteObject {
        principal: String,
        space_id: String,
        address: ObjectAddress,
        content: String,
        #[serde(deserialize_with = "deserialize_non_empty")]
        expected_revision: String,
    },
    TransitionTask {
        principal: String,
        space_id: String,
        object_id: String,
        state: String,
        #[serde(deserialize_with = "deserialize_non_empty")]
        expected_revision: String,
    },
    ResolveConflict {
        principal: String,
        space_id: String,
        conflict_id: String,
        resolution: String,
        #[serde(deserialize_with = "deserialize_non_empty")]
        expected_revision: String,
    },
}

impl Command {
    pub fn expected_revision(&self) -> Option<&str> {
        let revision = match self {
            Self::RegisterSpace {
                expected_revision, ..
            }
            | Self::RemoveSpace {
                expected_revision, ..
            }
            | Self::RegisterSource {
                expected_revision, ..
            }
            | Self::AttachSource {
                expected_revision, ..
            }
            | Self::ScanSpace {
                expected_revision, ..
            }
            | Self::CreateDocument {
                expected_revision, ..
            }
            | Self::UpdateDocument {
                expected_revision, ..
            }
            | Self::CreateObject {
                expected_revision, ..
            }
            | Self::WriteObject {
                expected_revision, ..
            }
            | Self::TransitionTask {
                expected_revision, ..
            }
            | Self::ResolveConflict {
                expected_revision, ..
            } => expected_revision,
            Self::IngestWatch(batch) => &batch.expected_revision,
        };
        Some(revision)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "query", rename_all = "snake_case")]
pub enum Query {
    ListSpaces {
        principal: String,
    },
    InspectSpace {
        principal: String,
        space_id: String,
    },
    ListDocuments {
        principal: String,
        space_id: String,
    },
    GetDocument {
        principal: String,
        space_id: String,
        document_path: String,
    },
    ResolveAddress {
        principal: String,
        space_id: String,
        address: ObjectAddress,
    },
    GetObject {
        principal: String,
        address: ObjectAddress,
    },
    Backlinks {
        principal: String,
        address: ObjectAddress,
    },
    Graph {
        principal: String,
        object: ObjectAddress,
        scope: String,
        #[serde(default = "default_graph_depth")]
        depth: u32,
    },
    GetSummary {
        principal: String,
        address: ObjectAddress,
    },
    WatchStatus {
        principal: String,
        source_id: String,
    },
}
