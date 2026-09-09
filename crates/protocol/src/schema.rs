//! Schema utilities.
//!
//! Surfaces that advertise their operations (MCP `tools/list`, future
//! HTTP `/schema`) pull JSON Schemas from here instead of maintaining
//! hand-written copies. One struct = one schema = one source of truth.

use schemars::Schema;
use std::collections::BTreeMap;

use crate::request::{self, Request};
use crate::response;

/// Human-readable tool name for a request variant. Matches the
/// historical MCP tool naming so the surface does not churn while the
/// engine unifies.
pub fn tool_name(req: &Request) -> &'static str {
    match req {
        Request::ScanNative(_) => "scan_native",
        Request::ScanFederation(_) => "scan_federation",
        Request::CaptureInboxItem(_) => "capture_inbox_item",
        Request::QueryResources(_) => "query",
        Request::ReadResource(_) => "read",
        Request::DeleteResource(_) => "resource_delete",
        Request::ListRecent(_) => "recent",
        Request::ListBySource(_) => "list_by_source",
        Request::Resolve(_) => "resolve",
        Request::UpsertResource(_) => "resource_upsert",
        Request::LinkOccurrences(_) => "link_occurrences",
        Request::ResolvedRelations(_) => "link_resolved",
        Request::ListLinks(_) => "link_list",
        Request::ResolveLinks(_) => "link_resolve",
        Request::DiagnoseLink(_) => "link_diagnose",
        Request::ReindexLinks(_) => "link_reindex",
        Request::Agenda(_) => "agenda",
        Request::ParaOverview(_) => "para",
        Request::TransitionTask(_) => "task_transition",
        Request::AddAttachment(_) => "attachment_add",
        Request::ExtractAttachment(_) => "attachment_extract",
        Request::QuerySegments(_) => "query_segments",
        Request::CreateCommunity(_) => "community_create",
        Request::ListCommunities(_) => "community_list",
        Request::DeriveArtifact(_) => "derive_artifact",
        Request::ExportSkill(_) => "export_skill",
        Request::InspectRules(_) => "inspect_rules",
        Request::SourceDoctor(_) => "source_doctor",
        Request::ListJobs(_) => "job_list",
        Request::ArtifactFreshness(_) => "artifact_stale",
        Request::SyncPush(_) => "sync_push",
        Request::SyncPull(_) => "sync_pull",
        Request::RelaySync(_) => "sync_relay",
        Request::ListConflicts(_) => "sync_conflicts",
        Request::WritebackResource(_) => "source_writeback",
        Request::UpdateDocument(_) => "update_document",
        Request::ExecuteJanet(_) => "execute_janet",
        Request::ListCards(_) => "list_cards",
        Request::ReadCard(_) => "read_card",
        Request::ExecuteCard(_) => "execute_card",
        Request::ListDashboard(_) => "list_dashboard",
        Request::UpdateDashboard(_) => "update_dashboard",
    }
}

/// Build `{ op_name: JsonSchema }` for every request struct, plus the
/// tagged [`Request`] enum itself under `"request"` so generic
/// clients can construct any operation.
pub fn request_schemas() -> BTreeMap<String, Schema> {
    let mut map: BTreeMap<String, Schema> = BTreeMap::new();
    macro_rules! add {
        ($($t:ty),* $(,)?) => {
            $(map.insert(to_snake_case(stringify!($t)), schemars::schema_for!($t));)*
        };
    }
    add!(
        request::ScanNativeRequest,
        request::ScanFederationRequest,
        request::QueryResourcesRequest,
        request::ReadResourceRequest,
        request::DeleteResourceRequest,
        request::ListRecentRequest,
        request::ListBySourceRequest,
        request::ResolveRequest,
        request::UpsertResourceRequest,
        request::LinkOccurrencesRequest,
        request::ResolvedRelationsRequest,
        request::ListLinksRequest,
        request::ResolveLinksRequest,
        request::DiagnoseLinkRequest,
        request::ReindexLinksRequest,
        request::AgendaRequest,
        request::ParaOverviewRequest,
        request::TransitionTaskRequest,
        request::AddAttachmentRequest,
        request::ExtractAttachmentRequest,
        request::QuerySegmentsRequest,
        request::CreateCommunityRequest,
        request::ListCommunitiesRequest,
        request::DeriveArtifactRequest,
        request::ExportSkillRequest,
        request::InspectRulesRequest,
        request::SourceDoctorRequest,
        request::ListJobsRequest,
        request::ArtifactFreshnessRequest,
        request::CaptureInboxItemRequest,
        request::WritebackResourceRequest,
        request::UpdateDocumentRequest,
        request::ExecuteJanetRequest,
        request::ListCardsRequest,
        request::ReadCardRequest,
        request::ExecuteCardRequest,
        request::ListDashboardRequest,
        request::UpdateDashboardRequest,
        request::SyncPushRequest,
        request::SyncPullRequest,
        request::RelaySyncRequest,
    );
    map.insert("request".to_string(), schemars::schema_for!(Request));
    map.insert(
        "command".to_string(),
        schemars::schema_for!(request::Command),
    );
    map.insert("query".to_string(), schemars::schema_for!(request::Query));
    map.insert(
        "object_address".to_string(),
        schemars::schema_for!(request::ObjectAddress),
    );
    map.insert(
        "graph_query".to_string(),
        schemars::schema_for!(request::GraphQuery),
    );
    map.insert(
        "ingest_watch_batch".to_string(),
        schemars::schema_for!(request::IngestWatchBatch),
    );
    map.insert(
        "response".to_string(),
        schemars::schema_for!(response::Response),
    );
    map.insert("error".to_string(), schemars::schema_for!(crate::Error));
    map
}

/// `stringify!(FooBarRequest)` can expand to a path like
/// `request :: FooBarRequest`; keep the final non-empty segment and
/// convert it to snake_case.
fn to_snake_case(name: &str) -> String {
    let name = name
        .rsplit(':')
        .map(str::trim)
        .find(|s| !s.is_empty())
        .unwrap_or(name);
    let mut out = String::with_capacity(name.len() + 4);
    for (i, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemas_cover_every_operation() {
        let map = request_schemas();
        assert_eq!(map.len(), 49); // 41 ops + request + unified command/query contracts
        assert!(map.contains_key("query_resources_request"));
        assert!(map.contains_key("transition_task_request"));
        assert!(map.contains_key("request"));
    }

    #[test]
    fn snake_case_handles_paths_and_acronyms() {
        assert_eq!(
            to_snake_case("QueryResourcesRequest"),
            "query_resources_request"
        );
        assert_eq!(to_snake_case("foo :: Bar"), "bar");
    }

    #[test]
    fn tool_names_are_unique() {
        use crate::request::*;
        let all = [
            Request::ScanNative(ScanNativeRequest {}),
            Request::ScanFederation(ScanFederationRequest {}),
            Request::QueryResources(QueryResourcesRequest {
                kind: None,
                title_contains: None,
                exact_ref: None,
                source_id: None,
                limit: None,
            }),
            Request::ReadResource(ReadResourceRequest {
                r_ref: String::new(),
            }),
            Request::DeleteResource(DeleteResourceRequest {
                r_ref: String::new(),
                expected_revision: None,
            }),
            Request::ListBySource(ListBySourceRequest {
                source_id: String::new(),
                limit: None,
            }),
            Request::Resolve(ResolveRequest {
                query: String::new(),
            }),
            Request::Agenda(AgendaRequest {}),
            Request::ListConflicts(ListConflictsRequest {}),
        ];
        let mut names: Vec<_> = all.iter().map(tool_name).collect();
        names.sort_unstable();
        let len = names.len();
        names.dedup();
        assert_eq!(names.len(), len);
    }
}
