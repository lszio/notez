//! Protocol dispatcher — the single interpreter for [`Request`].
//!
//! Every surface (CLI grammar, MCP tool call, HTTP body) converts its
//! native syntax into a `notez_protocol::Request` and hands it here.
//! The dispatcher is the only place that:
//!
//! * parses stringly identifiers into domain types,
//! * rejects malformed input with `ApplicationError::InvalidRequest`,
//! * funnels writes through one choke point (M3 adds journal + audit),
//! * maps use-case results onto the [`Response`] vocabulary.

use std::path::PathBuf;
use crate::domain::{QueryPage, ResolutionStatus, Resource, ResourceKind, Selector, TextSpan};
use crate::domain::{Community, ResolutionStatus as _, ResourceRef};
use crate::application::service::{ApplicationError, StorageErrorKind};
use crate::domain::query::{ProjectionReader, ProjectionStore, ProjectionWrite};
use crate::application::ApplicationFacade;
use crate::application::use_cases::{
    ArtifactUseCase, AttachmentUseCase, CommunityUseCase, InspectUseCase, LinkUseCase,
    ResourceUseCase, ScanUseCase, SyncUseCase, TaskUseCase,
};
use notez_protocol::request::Request;
use crate::storage::SqliteProjection;
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "result_of", rename_all = "snake_case")]
pub enum Response {
    Scan(crate::application::ScanReport),
    ResourcePage(crate::domain::QueryPage),
    /// `read`: present-or-absent single resource.
    Resource(Option<Resource>),
    Resources(Vec<Resource>),
    Resolve(crate::application::ResolveResult),
    Occurrences(Vec<crate::domain::LinkOccurrence>),
    Relations(Vec<crate::domain::ResolvedRelation>),
    Diagnostics(Vec<crate::domain::LinkDiagnostic>),
    Reindex(crate::application::LinkReindexReport),
    Agenda(crate::application::task_para::AgendaView),
    Para(crate::application::task_para::ParaOverview),
    Transition(crate::document::StateTransition),
    AttachmentRef(ResourceRef),
    Segments(Vec<crate::domain::SegmentRecord>),
    Communities(Vec<Community>),
    Derived(crate::artifact::DerivedArtifact),
    Skill(crate::artifact::SkillPackage),
    InspectRules(Option<crate::domain::InspectResult>),
    Doctor(crate::application::DoctorReport),
    Jobs(Vec<crate::application::JobRecord>),
    Freshness(crate::application::ArtifactStaleReport),
    Pushed(crate::sync::PushReport),
    Pulled(crate::sync::PullReport),
    Relay(crate::application::RelaySyncReport),
    Conflicts(Vec<crate::sync::ConflictRecord>),
    Writeback(crate::application::WritebackReport),
    Done,
}

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
    S: ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>
        + ProjectionStore,
{
    pub fn new(facade: &'a mut ApplicationFacade<S>) -> Self {
        Self { facade }
    }

    /// Evaluate one protocol request against the bound facade.
    pub fn dispatch(&mut self, req: Request) -> Result<Response, ApplicationError> {
        let f = &mut *self.facade;
        match req {
            Request::ScanNative(_) => Ok(Response::Scan(
                <ApplicationFacade<S> as ScanUseCase>::scan_native(f)?,
            )),
            Request::ScanFederation(_) => Ok(Response::Scan(
                <ApplicationFacade<S> as ScanUseCase>::scan_federation(f)?,
            )),
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
                let _page_limit = r.limit.unwrap_or(100);
                Ok(Response::ResourcePage(
                    <ApplicationFacade<S> as ResourceUseCase>::query(f, &selector)?,
                ))
            }
            Request::ReadResource(r) => {
                let rf = parse_ref(&r.r_ref)?;
                Ok(Response::Resource(
                    <ApplicationFacade<S> as ResourceUseCase>::read(f, &rf)?,
                ))
            }
            Request::DeleteResource(r) => {
                let rf = parse_ref(&r.r_ref)?;
                <ApplicationFacade<S> as ResourceUseCase>::delete_resource(f, &rf)?;
                Ok(Response::Done)
            }
            Request::ListRecent(r) => Ok(Response::Resources(
                <ApplicationFacade<S> as ResourceUseCase>::list_recent(
                    f,
                    r.limit.unwrap_or(20) as usize,
                )?,
            )),
            Request::ListBySource(r) => Ok(Response::Resources(
                <ApplicationFacade<S> as ResourceUseCase>::list_by_source(
                    f,
                    &r.source_id,
                    r.limit.unwrap_or(100) as usize,
                )?,
            )),
            Request::Resolve(r) => Ok(Response::Resolve(
                <ApplicationFacade<S> as ResourceUseCase>::resolve(f, &r.query)?,
            )),
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
                <ApplicationFacade<S> as ResourceUseCase>::upsert_resource(f, resource)?;
                Ok(Response::Done)
            }
            Request::LinkOccurrences(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Occurrences(
                    <ApplicationFacade<S> as LinkUseCase>::query_link_occurrences(f, &rf)?,
                ))
            }
            Request::ResolvedRelations(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Relations(
                    <ApplicationFacade<S> as LinkUseCase>::query_resolved_relations(f, &rf)?,
                ))
            }
            Request::ListLinks(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Occurrences(
                    <ApplicationFacade<S> as LinkUseCase>::list_links(f, &rf)?,
                ))
            }
            Request::ResolveLinks(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Relations(
                    <ApplicationFacade<S> as LinkUseCase>::resolve_links(f, &rf)?,
                ))
            }
            Request::DiagnoseLink(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Diagnostics(
                    <ApplicationFacade<S> as LinkUseCase>::diagnose_link(f, &rf)?,
                ))
            }
            Request::ReindexLinks(_) => Ok(Response::Reindex(
                <ApplicationFacade<S> as LinkUseCase>::reindex_links(f)?,
            )),
            Request::Agenda(_) => Ok(Response::Agenda(
                <ApplicationFacade<S> as TaskUseCase>::agenda(f)?,
            )),
            Request::ParaOverview(_) => Ok(Response::Para(
                <ApplicationFacade<S> as TaskUseCase>::para_overview(f)?,
            )),
            Request::TransitionTask(r) => {
                let rf = parse_ref(&r.r_ref)?;
                let timestamp = match r.timestamp.as_deref() {
                    Some(ts) => ts.to_owned(),
                    None => org_now(),
                };
                Ok(Response::Transition(
                    <ApplicationFacade<S> as TaskUseCase>::transition_task(
                        f,
                        &rf,
                        &r.to_state,
                        &timestamp,
                    )?,
                ))
            }
            Request::AddAttachment(r) => {
                let path = PathBuf::from(&r.file_path);
                let mime = r.mime.as_deref().unwrap_or("application/octet-stream");
                Ok(Response::AttachmentRef(
                    <ApplicationFacade<S> as AttachmentUseCase>::add_attachment(f, &path, mime)?,
                ))
            }
            Request::ExtractAttachment(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Segments(
                    <ApplicationFacade<S> as AttachmentUseCase>::run_extraction(f, &rf)?,
                ))
            }
            Request::QuerySegments(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Segments(
                    <ApplicationFacade<S> as AttachmentUseCase>::query_segments(f, &rf)?,
                ))
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
            Request::ListCommunities(_) => Ok(Response::Communities(
                <ApplicationFacade<S> as CommunityUseCase>::list_communities(f)?,
            )),
            Request::DeriveArtifact(r) => Ok(Response::Derived(
                <ApplicationFacade<S> as ArtifactUseCase>::derive_artifact(
                    f,
                    &r.community_id,
                    &r.recipe_name,
                )?,
            )),
            Request::ExportSkill(r) => {
                let out = PathBuf::from(&r.out_path);
                Ok(Response::Skill(<ApplicationFacade<S> as ArtifactUseCase>::export_skill(
                    f,
                    &r.community_id,
                    r.description.as_deref().unwrap_or("Notez exported skill"),
                    &out,
                )?))
            }
            Request::InspectRules(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::InspectRules(
                    <ApplicationFacade<S> as InspectUseCase>::inspect_rules(f, &rf)?,
                ))
            }
            Request::SourceDoctor(_) => Ok(Response::Doctor(
                <ApplicationFacade<S> as InspectUseCase>::source_doctor(f)?,
            )),
            Request::ListJobs(_) => Ok(Response::Jobs(
                <ApplicationFacade<S> as InspectUseCase>::list_jobs(f)?,
            )),
            Request::ArtifactFreshness(_) => Ok(Response::Freshness(
                <ApplicationFacade<S> as InspectUseCase>::check_artifact_freshness(f)?,
            )),
            Request::SyncPush(r) => Ok(Response::Pushed(
                <ApplicationFacade<S> as SyncUseCase>::sync_push(
                    f,
                    &r.actor_id,
                    &PathBuf::from(&r.folder),
                )?,
            )),
            Request::SyncPull(r) => Ok(Response::Pulled(
                <ApplicationFacade<S> as SyncUseCase>::sync_pull(
                    f,
                    &r.actor_id,
                    &PathBuf::from(&r.folder),
                )?,
            )),
            Request::RelaySync(_) => Ok(Response::Relay(
                <ApplicationFacade<S> as SyncUseCase>::relay_sync(f)?,
            )),
            Request::ListConflicts(_) => Ok(Response::Conflicts(
                <ApplicationFacade<S> as SyncUseCase>::list_conflicts(f)?,
            )),
            Request::WritebackResource(r) => {
                // Inherent writeback (source-aware); not part of the
                // nine traits because it needs the bound SourceContext.
                Ok(Response::Writeback(
                    f.writeback_resource(&r.source_id, &r.r_ref, &r.payload)?,
                ))
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
}
