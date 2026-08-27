//! Protocol dispatcher — the single interpreter for [`Request`].
//!
//! Every surface (CLI grammar, MCP tool call, HTTP body) converts its
//! native syntax into a `notez_protocol::Request` and hands it here.
//! The dispatcher is the only place that:
//!
//! * parses stringly identifiers into domain types,
//! * rejects malformed input with `ApplicationError::InvalidRequest`,
//! * enforces the expected-revision guard on every mutation arm,
//! * funnels writes through one choke point (journal + audit via the
//!   Projector at the use-case layer),
//! * maps use-case results onto the protocol [`Response`] vocabulary.

use std::path::PathBuf;
use crate::domain::{ProjectionStore, QueryPage, Resource, ResourceKind, Selector};
use crate::domain::{Community, ResourceRef};
use crate::application::service::ApplicationError;
use crate::application::{wire, ApplicationFacade};
use crate::application::use_cases::{
    ArtifactUseCase, AttachmentUseCase, CommunityUseCase, InspectUseCase, LinkUseCase,
    ResourceUseCase, ScanUseCase, SyncUseCase, TaskUseCase,
};
use notez_protocol::request::Request;
pub use notez_protocol::response::Response;

fn invalid(msg: impl Into<String>) -> ApplicationError {
    ApplicationError::InvalidRequest { message: msg.into() }
}

fn parse_ref(s: &str) -> Result<ResourceRef, ApplicationError> {
    ResourceRef::parse(s).map_err(|e| invalid(format!("malformed resource ref `{s}`: {e}")))
}

fn parse_kind(s: &str) -> Result<ResourceKind, ApplicationError> {
    match s {
        "document" => Ok(ResourceKind::Document),
        "heading" => Ok(ResourceKind::Heading),
        "block" => Ok(ResourceKind::Block),
        "attachment" => Ok(ResourceKind::Attachment),
        other => Err(invalid(format!(
            "unknown resource kind `{other}` (expected document|heading|block|attachment)"
        ))),
    }
}

/// Effective revision precondition for a request carrying both the
/// uniform guard and an operation-specific alias (`base_revision`).
/// The uniform field wins; the alias is a fallback for web parity.
fn effective_expected<'a>(
    expected: &'a Option<String>,
    base: &'a Option<String>,
) -> Option<&'a str> {
    expected
        .as_deref()
        .filter(|s| !s.is_empty())
        .or_else(|| base.as_deref().filter(|s| !s.is_empty()))
}

/// Org-format timestamp for "now", e.g. `2026-08-25 Mon 14:30`.
/// Dependency-free civil-date conversion (Hinnant's algorithm);
/// 1970-01-01 was a Thursday.
pub fn org_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (secs / 86_400) as i64;
    let secs_of_day = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let weekday = (((days % 7) + 7 + 3) % 7) as usize; // Thu=3 in Mon-first
    format!(
        "{y:04}-{m:02}-{d:02} {} {:02}:{:02}",
        WEEKDAYS[weekday],
        secs_of_day / 3_600,
        (secs_of_day % 3_600) / 60
    )
}

/// Interpreter over a facade. Cheap to construct per dispatch.
pub struct ApplicationDispatcher<'a, S: ProjectionStore> {
    facade: &'a mut ApplicationFacade<S>,
}
impl<'a, S> ApplicationDispatcher<'a, S>
where
    S: crate::domain::ProjectionReader<Error = crate::storage::StorageError>
        + crate::domain::ProjectionWrite<Error = crate::storage::StorageError>
        + ProjectionStore,
{
    pub fn new(facade: &'a mut ApplicationFacade<S>) -> Self {
        Self { facade }
    }

    /// Evaluate one protocol request against the bound facade.
    pub fn dispatch(&mut self, req: Request) -> Result<Response, ApplicationError> {
        use crate::application::write_check;
        let f = &mut *self.facade;
        match req {
            Request::ScanNative(_) => Ok(Response::Scan(wire::scan_report(
                &<ApplicationFacade<S> as ScanUseCase>::scan_native(f)?,
            ))),
            Request::ScanFederation(_) => Ok(Response::Scan(wire::scan_report(
                &<ApplicationFacade<S> as ScanUseCase>::scan_federation(f)?,
            ))),
            Request::QueryResources(r) => {
                let mut selector = Selector::new();
                if let Some(kind) = &r.kind {
                    selector.kind = Some(parse_kind(kind)?);
                }
                if let Some(t) = &r.title_contains {
                    selector = selector.with_title_contains(t.clone());
                }
                if let Some(x) = &r.exact_ref {
                    selector = selector.with_exact_ref(parse_ref(x)?);
                }
                if let Some(src) = &r.source_id {
                    selector = selector.with_source(src.clone());
                }
                // Push the page limit all the way into the store query.
                selector.limit = r.limit.map(|l| l as usize).or(selector.limit);
                let page: QueryPage =
                    <ApplicationFacade<S> as ResourceUseCase>::query(f, &selector)?;
                Ok(Response::ResourcePage(wire::page(&page)))
            }
            Request::ReadResource(r) => {
                let rf = parse_ref(&r.r_ref)?;
                Ok(Response::Resource(
                    <ApplicationFacade<S> as ResourceUseCase>::read(f, &rf)?
                        .as_ref()
                        .map(wire::resource),
                ))
            }
            Request::DeleteResource(r) => {
                let rf = parse_ref(&r.r_ref)?;
                if let Some(expected) = effective_expected(&r.expected_revision, &None) {
                    write_check::check_revision(f, &rf, expected)?;
                }
                <ApplicationFacade<S> as ResourceUseCase>::delete_resource(f, &rf)?;
                Ok(Response::Done)
            }
            Request::ListRecent(r) => Ok(Response::Resources(
                wire::resources(
                    &<ApplicationFacade<S> as ResourceUseCase>::list_recent(
                        f,
                        r.limit.unwrap_or(20) as usize,
                    )?,
                ),
            )),
            Request::ListBySource(r) => Ok(Response::Resources(
                wire::resources(
                    &<ApplicationFacade<S> as ResourceUseCase>::list_by_source(
                        f,
                        &r.source_id,
                        r.limit.unwrap_or(100) as usize,
                    )?,
                ),
            )),
            Request::Resolve(r) => Ok(Response::Resolve(wire::resolve_result(
                &<ApplicationFacade<S> as ResourceUseCase>::resolve(f, &r.query)?,
            ))),
            Request::UpsertResource(r) => {
                let p = r.resource;
                let kind = parse_kind(&p.kind)?;
                if p.ref_.is_empty() {
                    return Err(invalid("resource.ref_ is required"));
                }
                let object_id = match p.object_id.as_deref() {
                    Some(s) if !s.is_empty() => crate::domain::ObjectIdentity::parse(s)
                        .map_err(|e| invalid(format!("malformed object_id: {e}")))?,
                    _ => crate::domain::ObjectIdentity::default(),
                };
                let resource = Resource {
                    r#ref: parse_ref(&p.ref_)?,
                    kind,
                    title: p.title,
                    revision: p.revision,
                    source_id: p.source_id,
                    locator: p.locator,
                    properties: p.properties,
                    object_id,
                    primary_source_id: p.primary_source_id,
                };
                if let Some(expected) = effective_expected(&r.expected_revision, &None) {
                    write_check::check_revision(f, &resource.r#ref, expected)?;
                }
                <ApplicationFacade<S> as ResourceUseCase>::upsert_resource(f, resource)?;
                Ok(Response::Done)
            }
            Request::LinkOccurrences(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Occurrences(wire::occurrences(
                    &<ApplicationFacade<S> as LinkUseCase>::query_link_occurrences(f, &rf)?,
                )))
            }
            Request::ResolvedRelations(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Relations(wire::relations(
                    &<ApplicationFacade<S> as LinkUseCase>::query_resolved_relations(f, &rf)?,
                )))
            }
            Request::ListLinks(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Occurrences(wire::occurrences(
                    &<ApplicationFacade<S> as LinkUseCase>::list_links(f, &rf)?,
                )))
            }
            Request::ResolveLinks(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Relations(wire::relations(
                    &<ApplicationFacade<S> as LinkUseCase>::resolve_links(f, &rf)?,
                )))
            }
            Request::DiagnoseLink(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Diagnostics(wire::diagnostics(
                    &<ApplicationFacade<S> as LinkUseCase>::diagnose_link(f, &rf)?,
                )))
            }
            Request::ReindexLinks(_) => Ok(Response::Reindex(wire::reindex_report(
                &<ApplicationFacade<S> as LinkUseCase>::reindex_links(f)?,
            ))),
            Request::Agenda(_) => Ok(Response::Agenda(wire::agenda(
                &<ApplicationFacade<S> as TaskUseCase>::agenda(f)?,
            ))),
            Request::ParaOverview(_) => Ok(Response::Para(wire::para_overview(
                &<ApplicationFacade<S> as TaskUseCase>::para_overview(f)?,
            ))),
            Request::TransitionTask(r) => {
                let rf = parse_ref(&r.r_ref)?;
                if let Some(expected) = effective_expected(&r.expected_revision, &None) {
                    write_check::check_revision(f, &rf, expected)?;
                }
                let timestamp = match r.timestamp.as_deref() {
                    Some(ts) => ts.to_owned(),
                    None => org_now(),
                };
                Ok(Response::Transition(wire::state_transition(
                    &<ApplicationFacade<S> as TaskUseCase>::transition_task(
                        f,
                        &rf,
                        &r.to_state,
                        &timestamp,
                    )?,
                )))
            }
            Request::AddAttachment(r) => {
                let path = PathBuf::from(&r.file_path);
                let mime = r.mime.as_deref().unwrap_or("application/octet-stream");
                Ok(Response::AttachmentRef(
                    <ApplicationFacade<S> as AttachmentUseCase>::add_attachment(f, &path, mime)?
                        .to_string(),
                ))
            }
            Request::ExtractAttachment(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Segments(wire::segments(
                    &<ApplicationFacade<S> as AttachmentUseCase>::run_extraction(f, &rf)?,
                )))
            }
            Request::QuerySegments(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Segments(wire::segments(
                    &<ApplicationFacade<S> as AttachmentUseCase>::query_segments(f, &rf)?,
                )))
            }
            Request::CreateCommunity(r) => {
                let mut selector = Selector::new();
                if let Some(kind) = &r.kind {
                    selector.kind = Some(parse_kind(kind)?);
                }
                if let Some(t) = &r.title_contains {
                    selector = selector.with_title_contains(t.clone());
                }
                let community = Community {
                    id: r.id,
                    name: r.name,
                    selector,
                    pinned_members: Vec::new(),
                    excluded_members: Vec::new(),
                };
                <ApplicationFacade<S> as CommunityUseCase>::create_community(f, community)?;
                Ok(Response::Done)
            }
            Request::ListCommunities(_) => Ok(Response::Communities(wire::communities(
                &<ApplicationFacade<S> as CommunityUseCase>::list_communities(f)?,
            ))),
            Request::DeriveArtifact(r) => Ok(Response::Derived(wire::derived_artifact(
                &<ApplicationFacade<S> as ArtifactUseCase>::derive_artifact(
                    f,
                    &r.community_id,
                    &r.recipe_name,
                )?,
            ))),
            Request::ExportSkill(r) => {
                let out = PathBuf::from(&r.out_path);
                Ok(Response::Skill(wire::skill_package(
                    &<ApplicationFacade<S> as ArtifactUseCase>::export_skill(
                        f,
                        &r.community_id,
                        r.description.as_deref().unwrap_or("Notez exported skill"),
                        &out,
                    )?,
                )))
            }
            Request::InspectRules(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::InspectRules(
                    <ApplicationFacade<S> as InspectUseCase>::inspect_rules(f, &rf)?
                        .as_ref()
                        .map(wire::inspect_result),
                ))
            }
            Request::SourceDoctor(_) => Ok(Response::Doctor(wire::doctor_report(
                &<ApplicationFacade<S> as InspectUseCase>::source_doctor(f)?,
            ))),
            Request::ListJobs(_) => Ok(Response::Jobs(wire::job_records(
                &<ApplicationFacade<S> as InspectUseCase>::list_jobs(f)?,
            ))),
            Request::ArtifactFreshness(_) => Ok(Response::Freshness(wire::stale_report(
                &<ApplicationFacade<S> as InspectUseCase>::check_artifact_freshness(f)?,
            ))),
            Request::SyncPush(r) => Ok(Response::Pushed(wire::push_report(
                &<ApplicationFacade<S> as SyncUseCase>::sync_push(
                    f,
                    &r.actor_id,
                    &PathBuf::from(&r.folder),
                )?,
            ))),
            Request::SyncPull(r) => Ok(Response::Pulled(wire::pull_report(
                &<ApplicationFacade<S> as SyncUseCase>::sync_pull(
                    f,
                    &r.actor_id,
                    &PathBuf::from(&r.folder),
                )?,
            ))),
            Request::RelaySync(_) => Ok(Response::Relay(wire::relay_sync_report(
                &<ApplicationFacade<S> as SyncUseCase>::relay_sync(f)?,
            ))),
            Request::ListConflicts(_) => Ok(Response::Conflicts(wire::conflicts(
                &<ApplicationFacade<S> as SyncUseCase>::list_conflicts(f)?,
            ))),
            Request::WritebackResource(r) => {
                let rf = parse_ref(&r.r_ref)?;
                if let Some(expected) = effective_expected(&r.expected_revision, &None) {
                    write_check::check_revision(f, &rf, expected)?;
                }
                // Inherent writeback (source-aware); not part of the
                // nine traits because it needs the bound SourceContext.
                Ok(Response::Writeback(wire::writeback_report(
                    &f.writeback_resource(&r.source_id, &r.r_ref, &r.payload)?,
                )))
            }
            Request::UpdateDocument(r) => {
                let expected =
                    effective_expected(&r.expected_revision, &r.base_revision).map(str::to_string);
                let report = f.update_document(
                    &r.source_id,
                    &r.locator,
                    &r.content,
                    expected.as_deref(),
                )?;
                Ok(Response::DocumentUpdated(wire::document_update_report(&report)))
            }
            Request::ExecuteJanet(r) => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let source_scope = r.source_id.clone();
                    let mut selector = Selector::new();
                    selector.source_id = source_scope.clone();
                    let resources = <ApplicationFacade<S> as ResourceUseCase>::query(f, &selector)?.items;
                    if let Some(doc) = &r.document_ref {
                        let target = parse_ref(doc)?;
                        if !resources.iter().any(|x| x.r#ref == target) { return Err(invalid("document scope is outside source scope")); }
                    }
                    let mut relations = Vec::new();
                    for resource in &resources { relations.extend(<ApplicationFacade<S> as LinkUseCase>::query_resolved_relations(f, &resource.r#ref)?); }
                    let sources = serde_json::to_value(f.list_sources()?).unwrap_or_default();
                    let reads = resources.iter().filter(|x| r.document_ref.as_ref().map_or(true, |d| d == &x.r#ref.to_string())).map(|x| (x.r#ref.to_string(), serde_json::to_value(x).unwrap_or_default())).collect();
                    let snapshot = crate::application::service::JanetQuerySnapshot { sources, search: serde_json::to_value(&resources).unwrap_or_default(), objects: serde_json::to_value(&resources).unwrap_or_default(), relations: serde_json::to_value(&relations).unwrap_or_default(), reads, render_list: serde_json::to_value(&resources).unwrap_or_default() };
                    let executor = f.janet_executor.as_mut().ok_or(ApplicationError::UnsupportedCapability { capability: "execute_janet" })?;
                    let value = executor.execute(&r, &snapshot).map_err(|(kind, message)| ApplicationError::Janet { kind, message })?;
                    return Ok(Response::Janet(notez_protocol::response::JanetResult { value }));
                }
                #[cfg(target_arch = "wasm32")]
                { let _ = r; Err(ApplicationError::UnsupportedCapability { capability: "execute_janet" }) }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn org_now_formats_like_org_timestamp() {
        let ts = org_now();
        // "YYYY-MM-DD Www HH:MM" — 20 chars total.
        assert_eq!(ts.len(), 20, "{ts}");
        let bytes = ts.as_bytes();
        assert_eq!(bytes[4], b'-');
        assert_eq!(bytes[7], b'-');
        assert_eq!(bytes[10], b' ');
        assert_eq!(bytes[14], b' ');
        assert_eq!(bytes[17], b':');
        assert!(ts[11..14].chars().all(|c| c.is_ascii_alphabetic()));
    }

    #[test]
    fn weekday_algorithm_matches_known_dates() {
        // 1970-01-01 → Thursday; verify via org_now-style math on a
        // fixed epoch day count rather than wall clock.
        let days: i64 = 0;
        const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        let weekday = ((days % 7) + 7 + 3) % 7;
        assert_eq!(WEEKDAYS[weekday as usize], "Thu");
        // 2026-08-25 was a Tuesday (verified against the system clock
        // date at authoring time).
        let days_aug25_2026: i64 = 20_690;
        let weekday = ((days_aug25_2026 % 7) + 7 + 3).rem_euclid(7);
        assert_eq!(WEEKDAYS[weekday as usize], "Tue");
    }

    #[test]
    fn parse_kind_rejects_unknown() {
        assert!(parse_kind("heading").is_ok());
        assert!(matches!(
            parse_kind("bogus"),
            Err(ApplicationError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn parse_ref_rejects_garbage() {
        assert!(matches!(
            parse_ref("not-a-ref"),
            Err(ApplicationError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn effective_expected_prefers_uniform_field_and_skips_empty() {
        let exp = Some("e1".to_string());
        let base = Some("b1".to_string());
        assert_eq!(effective_expected(&exp, &base), Some("e1"));
        let none: Option<String> = None;
        assert_eq!(effective_expected(&none, &base), Some("b1"));
        assert_eq!(effective_expected(&exp, &none), Some("e1"));
        assert_eq!(effective_expected(&none, &none), None);
        let empty = Some(String::new());
        assert_eq!(effective_expected(&empty, &base), Some("b1"));
        assert_eq!(effective_expected(&empty, &none), None);
    }
}
