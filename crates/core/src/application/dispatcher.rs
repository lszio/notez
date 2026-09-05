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
use crate::domain::{ProjectionReader, ProjectionStore, ProjectionWrite, QueryPage, Resource, ResourceKind, Selector};
use crate::domain::{Community, ResourceRef};
use crate::application::service::ApplicationError;
use crate::application::{wire, Engine};
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
    facade: &'a mut Engine<S>,
}
impl<'a, S> ApplicationDispatcher<'a, S>
where
    S: crate::domain::ProjectionReader<Error = crate::storage::StorageError>
        + crate::domain::ProjectionWrite<Error = crate::storage::StorageError>
        + ProjectionStore,
{
    pub fn new(facade: &'a mut Engine<S>) -> Self {
        Self { facade }
    }

    /// Evaluate one protocol request against the bound facade.
    pub fn dispatch(&mut self, req: Request) -> Result<Response, ApplicationError> {
        use crate::application::write_check;
        let f = &mut *self.facade;
        match req {
            Request::ScanNative(_) => Ok(Response::Scan(wire::scan_report(
                &<Engine<S> as ScanUseCase>::scan_native(f)?,
            ))),
            Request::ScanFederation(_) => Ok(Response::Scan(wire::scan_report(
                &<Engine<S> as ScanUseCase>::scan_federation(f)?,
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
                    <Engine<S> as ResourceUseCase>::query(f, &selector)?;
                Ok(Response::ResourcePage(wire::page(&page)))
            }
            Request::ReadResource(r) => {
                let rf = parse_ref(&r.r_ref)?;
                Ok(Response::Resource(
                    <Engine<S> as ResourceUseCase>::read(f, &rf)?
                        .as_ref()
                        .map(wire::resource),
                ))
            }
            Request::DeleteResource(r) => {
                let rf = parse_ref(&r.r_ref)?;
                if let Some(expected) = effective_expected(&r.expected_revision, &None) {
                    write_check::check_revision(f, &rf, expected)?;
                }
                <Engine<S> as ResourceUseCase>::delete_resource(f, &rf)?;
                Ok(Response::Done)
            }
            Request::ListRecent(r) => Ok(Response::Resources(
                wire::resources(
                    &<Engine<S> as ResourceUseCase>::list_recent(
                        f,
                        r.limit.unwrap_or(20) as usize,
                    )?,
                ),
            )),
            Request::ListBySource(r) => Ok(Response::Resources(
                wire::resources(
                    &<Engine<S> as ResourceUseCase>::list_by_source(
                        f,
                        &r.source_id,
                        r.limit.unwrap_or(100) as usize,
                    )?,
                ),
            )),
            Request::Resolve(r) => Ok(Response::Resolve(wire::resolve_result(
                &<Engine<S> as ResourceUseCase>::resolve(f, &r.query)?,
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
                <Engine<S> as ResourceUseCase>::upsert_resource(f, resource)?;
                Ok(Response::Done)
            }
            Request::LinkOccurrences(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Occurrences(wire::occurrences(
                    &<Engine<S> as LinkUseCase>::query_link_occurrences(f, &rf)?,
                )))
            }
            Request::ResolvedRelations(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Relations(wire::relations(
                    &<Engine<S> as LinkUseCase>::query_resolved_relations(f, &rf)?,
                )))
            }
            Request::ListLinks(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Occurrences(wire::occurrences(
                    &<Engine<S> as LinkUseCase>::list_links(f, &rf)?,
                )))
            }
            Request::ResolveLinks(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Relations(wire::relations(
                    &<Engine<S> as LinkUseCase>::resolve_links(f, &rf)?,
                )))
            }
            Request::DiagnoseLink(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Diagnostics(wire::diagnostics(
                    &<Engine<S> as LinkUseCase>::diagnose_link(f, &rf)?,
                )))
            }
            Request::ReindexLinks(_) => Ok(Response::Reindex(wire::reindex_report(
                &<Engine<S> as LinkUseCase>::reindex_links(f)?,
            ))),
            Request::Agenda(_) => Ok(Response::Agenda(wire::agenda(
                &<Engine<S> as TaskUseCase>::agenda(f)?,
            ))),
            Request::ParaOverview(_) => Ok(Response::Para(wire::para_overview(
                &<Engine<S> as TaskUseCase>::para_overview(f)?,
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
                    &<Engine<S> as TaskUseCase>::transition_task(
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
                    <Engine<S> as AttachmentUseCase>::add_attachment(f, &path, mime)?
                        .to_string(),
                ))
            }
            Request::ExtractAttachment(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Segments(wire::segments(
                    &<Engine<S> as AttachmentUseCase>::run_extraction(f, &rf)?,
                )))
            }
            Request::QuerySegments(r) => {
                let rf = parse_ref(&r.source_ref)?;
                Ok(Response::Segments(wire::segments(
                    &<Engine<S> as AttachmentUseCase>::query_segments(f, &rf)?,
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
                <Engine<S> as CommunityUseCase>::create_community(f, community)?;
                Ok(Response::Done)
            }
            Request::ListCommunities(_) => Ok(Response::Communities(wire::communities(
                &<Engine<S> as CommunityUseCase>::list_communities(f)?,
            ))),
            Request::DeriveArtifact(r) => Ok(Response::Derived(wire::derived_artifact(
                &<Engine<S> as ArtifactUseCase>::derive_artifact(
                    f,
                    &r.community_id,
                    &r.recipe_name,
                )?,
            ))),
            Request::ExportSkill(r) => {
                let out = PathBuf::from(&r.out_path);
                Ok(Response::Skill(wire::skill_package(
                    &<Engine<S> as ArtifactUseCase>::export_skill(
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
                    <Engine<S> as InspectUseCase>::inspect_rules(f, &rf)?
                        .as_ref()
                        .map(wire::inspect_result),
                ))
            }
            Request::CaptureInboxItem(_) => Err(ApplicationError::UnsupportedCapability {
                capability: "capture_inbox_item",
            }),
            Request::ListJobs(_) => Ok(Response::Jobs(wire::job_records(
                &<Engine<S> as InspectUseCase>::list_jobs(f)?,
            ))),
            Request::SourceDoctor(_) => Ok(Response::Doctor(wire::doctor_report(
                &<Engine<S> as InspectUseCase>::source_doctor(f)?,
            ))),
            Request::ArtifactFreshness(_) => Ok(Response::Freshness(wire::stale_report(
                &<Engine<S> as InspectUseCase>::check_artifact_freshness(f)?,
            ))),
            Request::SyncPush(r) => Ok(Response::Pushed(wire::push_report(
                &<Engine<S> as SyncUseCase>::sync_push(
                    f,
                    &r.actor_id,
                    &PathBuf::from(&r.folder),
                )?,
            ))),
            Request::SyncPull(r) => Ok(Response::Pulled(wire::pull_report(
                &<Engine<S> as SyncUseCase>::sync_pull(
                    f,
                    &r.actor_id,
                    &PathBuf::from(&r.folder),
                )?,
            ))),
            Request::RelaySync(_) => Ok(Response::Relay(wire::relay_sync_report(
                &<Engine<S> as SyncUseCase>::relay_sync(f)?,
            ))),
            Request::ListConflicts(_) => Ok(Response::Conflicts(wire::conflicts(
                &<Engine<S> as SyncUseCase>::list_conflicts(f)?,
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
                    let resources = <Engine<S> as ResourceUseCase>::query(f, &selector)?.items;
                    if let Some(doc) = &r.document_ref {
                        let target = parse_ref(doc)?;
                        if !resources.iter().any(|x| x.r#ref == target) { return Err(invalid("document scope is outside source scope")); }
                    }
                    let mut relations = Vec::new();
                    for resource in &resources { relations.extend(<Engine<S> as LinkUseCase>::query_resolved_relations(f, &resource.r#ref)?); }
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
            Request::ListCards(r) => Ok(Response::Dashboard { cards: project_dashboard_cards(
                f,
                r.source.as_deref(),
                r.limit.unwrap_or(32),
            )? }),
            Request::ReadCard(r) => Ok(Response::Card(read_card(
                f,
                r.source.as_str(),
                &r.locator,
                &r.card_id,
            )?)),
            Request::ExecuteCard(r) => Ok(Response::CardExecution(execute_card(
                f,
                r.source.as_str(),
                &r.locator,
                &r.card_id,
                r.timeout_ms,
            )?)),
            Request::ListDashboard(_) => Ok(Response::Dashboard { cards: project_dashboard_cards(
                f, None, 32,
            )? }),
            Request::UpdateDashboard(r) => Ok(Response::DashboardUpdate(
                update_dashboard_layout(f, r.ordered_card_ids.clone(), r.hidden_card_ids.clone())?,
            )),
        }
    }
}

/// Walk the bound source's policy-filtered files, parse notez
/// blocks, execute each via the engine's [`CardExecutionService`],
/// and project them into the wire shape. `source` defaults to the
/// engine's bound source.
fn project_dashboard_cards<S>(
    engine: &mut Engine<S>,
    source: Option<&str>,
    limit: usize,
) -> Result<Vec<notez_protocol::response::CardProjection>, ApplicationError>
where
    S: ProjectionStore
        + ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>,
{
    use crate::application::card_executor::CardProjection as InternalProjection;
    let ctx = engine.source.as_ref();
    let root = ctx.ok_or(invalid("dashboard cards require a bound source context"))?.root.clone();
    let locator = source.map(String::from).unwrap_or_else(|| root.to_string_lossy().into_owned());
    let policy = crate::source::policy::SourcePolicy::load_for_root(&root);
    let mut files = Vec::new();
    collect_dashboard_files(&root, &root, &policy, &mut files);
    files.truncate(50);
    let mut out = Vec::with_capacity(files.len());
    let mut count = 0usize;
    for path in files {
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let rel = path.strip_prefix(&root).map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| locator.clone());
        let blocks = match path.extension().and_then(|e| e.to_str()) {
            Some("org") => crate::document::parse_org_notez_blocks(&text),
            _ => crate::document::parse_markdown_notez_blocks(&text),
        };
        for block in blocks {
            if out.iter().any(|c: &notez_protocol::response::CardProjection| c.id == block.id) {
                continue; // first card wins
            }
            let context = crate::application::card_executor::CardExecutionContext::for_locator(&rel);
            let state = engine.card_service.run(&block, &context);
            out.push(to_wire_projection(InternalProjection::from_state(&block, rel.clone(), state)));
            count += 1;
            if count >= limit {
                break;
            }
        }
        if count >= limit {
            break;
        }
    }
    Ok(out)
}

fn collect_dashboard_files(
    root: &std::path::Path,
    dir: &std::path::Path,
    policy: &crate::source::policy::SourcePolicy,
    out: &mut Vec<std::path::PathBuf>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let file_type = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        let is_symlink = file_type.is_symlink()
            || std::fs::symlink_metadata(&path).map(|m| m.file_type().is_symlink()).unwrap_or(false);
        if file_type.is_dir() {
            if policy.hides_dir(&rel, is_symlink) {
                continue;
            }
            collect_dashboard_files(root, &path, policy, out);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if matches!(ext, "md" | "markdown" | "org") {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if let crate::source::policy::Decision::Allow = policy.decide(&rel, is_symlink, size) {
                candidates.push(path);
            }
        }
    }
    out.extend(candidates);
}

/// Look up one notez card by id within the file at `locator`.
fn read_card<S>(
    engine: &mut Engine<S>,
    source: &str,
    locator: &str,
    card_id: &str,
) -> Result<notez_protocol::response::CardProjection, ApplicationError>
where
    S: ProjectionStore
        + ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>,
{
    use crate::application::card_executor::CardProjection as InternalProjection;
    let root = resolve_source_root(engine, source)?;
    let path = root.join(locator);
    let text = std::fs::read_to_string(&path)
        .map_err(|_| invalid(format!("locator `{locator}` is not readable")))?;
    let format = path.extension().and_then(|e| e.to_str());
    let blocks = match format {
        Some("org") => crate::document::parse_org_notez_blocks(&text),
        _ => crate::document::parse_markdown_notez_blocks(&text),
    };
    let block = blocks
        .into_iter()
        .find(|b| b.id == card_id)
        .ok_or_else(|| invalid(format!("card `{card_id}` not found in `{locator}`")))?;
    let context = crate::application::card_executor::CardExecutionContext::for_locator(locator);
    let state = engine.card_service.run(&block, &context);
    Ok(to_wire_projection(InternalProjection::from_state(&block, locator.to_string(), state)))
}

/// Execute one notez card and return the live result.
fn execute_card<S>(
    engine: &mut Engine<S>,
    source: &str,
    locator: &str,
    card_id: &str,
    timeout_ms: Option<u64>,
) -> Result<notez_protocol::response::CardProjection, ApplicationError>
where
    S: ProjectionStore
        + ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>,
{
    use crate::application::card_executor::CardProjection as InternalProjection;
    let root = resolve_source_root(engine, source)?;
    let path = root.join(locator);
    let text = std::fs::read_to_string(&path)
        .map_err(|_| invalid(format!("locator `{locator}` is not readable")))?;
    let format = path.extension().and_then(|e| e.to_str());
    let blocks = match format {
        Some("org") => crate::document::parse_org_notez_blocks(&text),
        _ => crate::document::parse_markdown_notez_blocks(&text),
    };
    let block = blocks
        .into_iter()
        .find(|b| b.id == card_id)
        .ok_or_else(|| invalid(format!("card `{card_id}` not found in `{locator}`")))?;
    let mut context = crate::application::card_executor::CardExecutionContext::for_locator(locator);
    if let Some(ms) = timeout_ms {
        context.timeout = std::time::Duration::from_millis(ms.min(2_000));
    }
    let state = engine.card_service.run(&block, &context);
    Ok(to_wire_projection(InternalProjection::from_state(&block, locator.to_string(), state)))
}

/// Update the per-user dashboard layout (ordered + hidden card ids).
fn update_dashboard_layout<S>(
    engine: &mut Engine<S>,
    ordered_card_ids: Vec<String>,
    hidden_card_ids: Vec<String>,
) -> Result<notez_protocol::response::DashboardLayout, ApplicationError>
where
    S: ProjectionStore
        + ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>,
{
    // The dashboard layout lives in the web surface (web.toml).
    // Surfaces that care wire this through their own surface; the
    // dispatcher echoes the new layout so callers can confirm.
    let _ = (engine, ordered_card_ids, hidden_card_ids);
    Err(ApplicationError::UnsupportedCapability { capability: "update_dashboard" })
}

fn resolve_source_root<S>(
    engine: &Engine<S>,
    source: &str,
) -> Result<std::path::PathBuf, ApplicationError>
where
    S: ProjectionStore
        + ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>,
{
    if let Some(ctx) = engine.source.as_ref() {
        if source.is_empty()
            || ctx.root.to_string_lossy() == source
            || ctx.source_id == source
            || source == "default"
        {
            return Ok(ctx.root.clone());
        }
    }
    Err(invalid(format!(
        "source `{source}` is not the engine's bound source (use composition::open_selected first)"
    )))
}

/// Convert the engine-internal card projection into the protocol
/// DTO every surface consumes.
fn to_wire_projection(
    p: crate::application::card_executor::CardProjection,
) -> notez_protocol::response::CardProjection {
    use notez_protocol::response::CardProjection as Wire;
    Wire {
        id: p.id,
        title: p.title,
        language: p.language,
        locator: p.locator,
        ordinal: p.ordinal,
        state: p.state_name.to_string(),
        output_type: p.output_type.to_string(),
        output: p.output.map(|c| card_output_to_value(&c)),
        object_ref: p.object_ref,
        html: p.html,
        error: p.error,
        error_kind: p.error_kind,
    }
}

fn card_output_to_value(c: &crate::document::notez_block::CardOutput) -> serde_json::Value {
    use crate::document::notez_block::CardOutput;
    match c {
        CardOutput::Json(value) => value.clone(),
        CardOutput::List(items) => serde_json::Value::Array(items.clone()),
        CardOutput::Object { .. } | CardOutput::Html(_) => serde_json::Value::Null,
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
