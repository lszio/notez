//! Core → protocol DTO mapping.
//!
//! The dispatcher returns [`notez_protocol::response::Response`];
//! these helpers convert engine values into their wire DTOs with
//! identical JSON shapes (see `notez_protocol::response` docs).

use crate::application::link_resolution::LinkReindexReport;
use crate::application::service::{ResolveResult, ScanReport};
use crate::application::{
    task_para::{AgendaItem, AgendaView, ParaNode, ParaOverview},
    ArtifactStaleReport, DoctorIssue, DoctorReport, JobRecord, RelaySyncReport, WritebackReport,
};
use crate::domain::resource::{ObjectIdentity, ResourceKind};
use crate::domain::{
    ConflictRecord, InspectResult, LinkDiagnostic, LinkOccurrence, LinkTarget, QueryPage,
    ResolutionStatus, ResolvedRelation, RuleTrace, SegmentRecord, TextSpan,
};
use notez_protocol::response as proto;

pub fn kind(k: ResourceKind) -> proto::ResourceKind {
    match k {
        ResourceKind::Document => proto::ResourceKind::Document,
        ResourceKind::Heading => proto::ResourceKind::Heading,
        ResourceKind::Attachment => proto::ResourceKind::Attachment,
        ResourceKind::Block => proto::ResourceKind::Block,
    }
}

fn object_id(o: &ObjectIdentity) -> String {
    o.to_string()
}

pub fn resource(r: &crate::domain::Resource) -> proto::Resource {
    proto::Resource {
        ref_: r.r#ref.to_string(),
        kind: kind(r.kind),
        title: r.title.clone(),
        revision: r.revision.clone(),
        source_id: r.source_id.clone(),
        locator: r.locator.clone(),
        properties: r.properties.clone(),
        object_id: object_id(&r.object_id),
        primary_source_id: r.primary_source_id.clone(),
    }
}

pub fn page(p: &QueryPage) -> proto::QueryPage {
    proto::QueryPage {
        items: p.items.iter().map(resource).collect(),
        next_cursor: p.next_cursor.clone(),
    }
}

pub fn scan_report(r: &ScanReport) -> proto::ScanReport {
    proto::ScanReport {
        scanned_files: r.scanned_files,
        scanned_resources: r.scanned_resources,
        scanned_relations: r.scanned_relations,
        ignored: r.ignored,
        ignored_hidden: r.ignored_hidden,
        ignored_excluded: r.ignored_excluded,
        ignored_not_included: r.ignored_not_included,
        ignored_too_large: r.ignored_too_large,
        ignored_symlink: r.ignored_symlink,
    }
}
pub fn resources(v: &[crate::domain::Resource]) -> Vec<proto::Resource> {
    v.iter().map(resource).collect()
}


pub fn resolve_result(r: &ResolveResult) -> proto::ResolveResult {
    match r {
        ResolveResult::Found(r_ref) => proto::ResolveResult::Found(r_ref.to_string()),
        ResolveResult::NotFound => proto::ResolveResult::NotFound,
        ResolveResult::Ambiguous(refs) => {
            proto::ResolveResult::Ambiguous(refs.iter().map(|r| r.to_string()).collect())
        }
    }
}

pub fn link_target(t: &LinkTarget) -> proto::LinkTarget {
    match t {
        LinkTarget::Id { value, kind_hint } => proto::LinkTarget::Id {
            value: value.clone(),
            kind_hint: kind_hint.map(kind),
        },
        LinkTarget::File { path, fragment } => proto::LinkTarget::File {
            path: path.clone(),
            fragment: fragment.clone(),
        },
        LinkTarget::Title { title, fragment } => proto::LinkTarget::Title {
            title: title.clone(),
            fragment: fragment.clone(),
        },
        LinkTarget::Url { url } => proto::LinkTarget::Url { url: url.clone() },
        LinkTarget::Custom {
            scheme,
            value,
            fragment,
        } => proto::LinkTarget::Custom {
            scheme: scheme.clone(),
            value: value.clone(),
            fragment: fragment.clone(),
        },
    }
}

pub fn span(s: &TextSpan) -> proto::TextSpan {
    proto::TextSpan {
        line: s.line,
        col_start: s.col_start,
        col_end: s.col_end,
    }
}

pub fn status(s: &ResolutionStatus) -> proto::ResolutionStatus {
    match s {
        ResolutionStatus::Resolved => proto::ResolutionStatus::Resolved,
        ResolutionStatus::Ambiguous => proto::ResolutionStatus::Ambiguous,
        ResolutionStatus::Unresolved => proto::ResolutionStatus::Unresolved,
        ResolutionStatus::External => proto::ResolutionStatus::External,
        ResolutionStatus::Invalid => proto::ResolutionStatus::Invalid,
    }
}

pub fn occurrence(o: &LinkOccurrence) -> proto::LinkOccurrence {
    proto::LinkOccurrence {
        source_ref: o.source_ref.to_string(),
        target: link_target(&o.target),
        raw: o.raw.clone(),
        display_text: o.display_text.clone(),
        span: span(&o.span),
    }
}

pub fn occurrences(v: &[LinkOccurrence]) -> Vec<proto::LinkOccurrence> {
    v.iter().map(occurrence).collect()
}

pub fn relation(r: &ResolvedRelation) -> proto::ResolvedRelation {
    proto::ResolvedRelation {
        source_ref: r.source_ref.to_string(),
        target_ref: r.target_ref.to_string(),
        target: link_target(&r.target),
        status: status(&r.status),
        candidates: r.candidates.iter().map(|c| c.to_string()).collect(),
        relation_type: relation_type(&r.relation_type),
        direction: relation_direction(&r.direction),
        evidence_json: r.evidence_json.clone(),
        created_at: r.created_at.clone(),
        creator: r.creator.clone(),
    }
}

pub fn relations(v: &[ResolvedRelation]) -> Vec<proto::ResolvedRelation> {
    v.iter().map(relation).collect()
}

fn relation_type(t: &crate::domain::RelationType) -> proto::RelationType {
    match t {
        crate::domain::RelationType::References => proto::RelationType::References,
        crate::domain::RelationType::Embeds => proto::RelationType::Embeds,
        crate::domain::RelationType::Backlink => proto::RelationType::Backlink,
        crate::domain::RelationType::Custom(s) => proto::RelationType::Custom(s.clone()),
    }
}

fn relation_direction(d: &crate::domain::RelationDirection) -> proto::RelationDirection {
    match d {
        crate::domain::RelationDirection::Forward => proto::RelationDirection::Forward,
        crate::domain::RelationDirection::Backward => proto::RelationDirection::Backward,
        crate::domain::RelationDirection::Unknown => proto::RelationDirection::Unknown,
    }
}

pub fn diagnostic(d: &LinkDiagnostic) -> proto::LinkDiagnostic {
    proto::LinkDiagnostic {
        occurrence: occurrence(&d.occurrence),
        status: status(&d.status),
        candidates: d.candidates.iter().map(|c| c.to_string()).collect(),
    }
}

pub fn diagnostics(v: &[LinkDiagnostic]) -> Vec<proto::LinkDiagnostic> {
    v.iter().map(diagnostic).collect()
}

pub fn reindex_report(r: &LinkReindexReport) -> proto::LinkReindexReport {
    proto::LinkReindexReport {
        scanned: r.scanned,
        resolved: r.resolved,
        unresolved: r.unresolved,
        ambiguous: r.ambiguous,
        external: r.external,
        invalid: r.invalid,
    }
}

pub fn agenda_item(i: &AgendaItem) -> proto::AgendaItem {
    proto::AgendaItem {
        r_ref: i.r_ref.clone(),
        title: i.title.clone(),
        todo: i.todo.clone(),
        scheduled: i.scheduled.clone(),
        deadline: i.deadline.clone(),
        closed: i.closed.clone(),
        locator: i.locator.clone(),
    }
}

pub fn agenda(v: &AgendaView) -> proto::AgendaView {
    proto::AgendaView {
        items: v.items.iter().map(agenda_item).collect(),
    }
}

fn para_node(n: &ParaNode) -> proto::ParaNode {
    proto::ParaNode {
        resource: resource(&n.resource),
        tasks: n.tasks.iter().map(agenda_item).collect(),
    }
}

pub fn para_overview(v: &ParaOverview) -> proto::ParaOverview {
    proto::ParaOverview {
        projects: v.projects.iter().map(para_node).collect(),
        areas: v.areas.iter().map(para_node).collect(),
        resources: v.resources.iter().map(para_node).collect(),
        archives: v.archives.iter().map(para_node).collect(),
    }
}

pub fn state_transition(t: &crate::document::StateTransition) -> proto::StateTransition {
    proto::StateTransition {
        from_state: t.from_state.clone(),
        to_state: t.to_state.clone(),
        closed_timestamp: t.closed_timestamp.clone(),
        logbook_entry: t.logbook_entry.clone(),
    }
}

pub fn segment(s: &SegmentRecord) -> proto::SegmentRecord {
    proto::SegmentRecord {
        id: s.id.clone(),
        attachment_ref: s.attachment_ref.clone(),
        text: s.text.clone(),
        offset_start: s.offset_start,
        offset_end: s.offset_end,
    }
}

pub fn segments(v: &[SegmentRecord]) -> Vec<proto::SegmentRecord> {
    v.iter().map(segment).collect()
}

pub fn selector(s: &crate::domain::Selector) -> proto::Selector {
    proto::Selector {
        kind: s.kind.map(kind),
        title_contains: s.title_contains.clone(),
        exact_refs: s.exact_refs.iter().map(|r| r.to_string()).collect(),
        source_id: s.source_id.clone(),
        limit: s.limit,
    }
}

pub fn community(c: &crate::domain::Community) -> proto::Community {
    proto::Community {
        id: c.id.clone(),
        name: c.name.clone(),
        selector: selector(&c.selector),
        pinned_members: c.pinned_members.iter().map(|r| r.to_string()).collect(),
        excluded_members: c.excluded_members.iter().map(|r| r.to_string()).collect(),
    }
}

pub fn communities(v: &[crate::domain::Community]) -> Vec<proto::Community> {
    v.iter().map(community).collect()
}

pub fn derived_artifact(
    a: &crate::artifact::DerivedArtifact,
) -> proto::DerivedArtifact {
    proto::DerivedArtifact {
        recipe_name: a.recipe_name.clone(),
        kind: recipe_kind(a.kind),
        content: a.content.clone(),
    }
}

fn recipe_kind(k: crate::artifact::RecipeKind) -> proto::RecipeKind {
    match k {
        crate::artifact::RecipeKind::Summary => proto::RecipeKind::Summary,
        crate::artifact::RecipeKind::LlmsTxt => proto::RecipeKind::LlmsTxt,
        crate::artifact::RecipeKind::ContextPack => proto::RecipeKind::ContextPack,
        crate::artifact::RecipeKind::SkillIr => proto::RecipeKind::SkillIr,
    }
}

pub fn skill_package(p: &crate::artifact::SkillPackage) -> proto::SkillPackage {
    proto::SkillPackage {
        name: p.name.clone(),
        package_path: p.package_path.display().to_string(),
    }
}

fn rule_kind(k: crate::domain::RuleKind) -> proto::RuleKind {
    match k {
        crate::domain::RuleKind::Classify => proto::RuleKind::Classify,
        crate::domain::RuleKind::Validate => proto::RuleKind::Validate,
        crate::domain::RuleKind::Derive => proto::RuleKind::Derive,
        crate::domain::RuleKind::React => proto::RuleKind::React,
    }
}

fn rule_trace(t: &RuleTrace) -> proto::RuleTrace {
    proto::RuleTrace {
        rule_id: t.rule_id.clone(),
        kind: rule_kind(t.kind),
        matched: t.matched,
        message: t.message.clone(),
    }
}

pub fn inspect_result(i: &InspectResult) -> proto::InspectResult {
    proto::InspectResult {
        r_ref: i.r_ref.clone(),
        classified_type: i.classified_type.clone(),
        derived_properties: i.derived_properties.clone(),
        traces: i.traces.iter().map(rule_trace).collect(),
    }
}

pub fn doctor_report(d: &DoctorReport) -> proto::DoctorReport {
    proto::DoctorReport {
        status: d.status.clone(),
        issues: d
            .issues
            .iter()
            .map(|i| proto::DoctorIssue {
                severity: i.severity.clone(),
                code: i.code.clone(),
                message: i.message.clone(),
            })
            .collect(),
    }
}

// Keep DoctorIssue import used for signature clarity downstream.
#[allow(unused)]
fn doctor_issue(i: &DoctorIssue) -> proto::DoctorIssue {
    proto::DoctorIssue {
        severity: i.severity.clone(),
        code: i.code.clone(),
        message: i.message.clone(),
    }
}

pub fn job_record(j: &JobRecord) -> proto::JobRecord {
    proto::JobRecord {
        job_id: j.job_id.clone(),
        job_type: j.job_type.clone(),
        target_ref: j.target_ref.clone(),
        status: j.status.clone(),
        message: j.message.clone(),
    }
}

pub fn job_records(v: &[JobRecord]) -> Vec<proto::JobRecord> {
    v.iter().map(job_record).collect()
}

pub fn stale_report(r: &ArtifactStaleReport) -> proto::ArtifactStaleReport {
    proto::ArtifactStaleReport {
        status: r.status.clone(),
        stale_artifacts: r.stale_artifacts.clone(),
    }
}

pub fn push_report(r: &crate::sync::PushReport) -> proto::PushReport {
    proto::PushReport {
        pushed_files: r.pushed_files,
        pushed_manifests: r.pushed_manifests,
        pushed_objects: r.pushed_objects,
    }
}

pub fn pull_report(r: &crate::sync::PullReport) -> proto::PullReport {
    proto::PullReport {
        pulled_files: r.pulled_files,
        merged_files: r.merged_files,
        conflicts: r.conflicts.iter().map(conflict_record).collect(),
    }
}

pub fn conflict_record(c: &ConflictRecord) -> proto::ConflictRecord {
    proto::ConflictRecord {
        logical_path: c.logical_path.clone(),
        mine_hash: c.mine_hash.clone(),
        theirs_hash: c.theirs_hash.clone(),
        conflict_text: c.conflict_text.clone(),
        status: c.status.clone(),
        detected_at: c.detected_at,
    }
}

pub fn conflicts(v: &[ConflictRecord]) -> Vec<proto::ConflictRecord> {
    v.iter().map(conflict_record).collect()
}

pub fn relay_sync_report(r: &RelaySyncReport) -> proto::RelaySyncReport {
    proto::RelaySyncReport {
        source_id: r.source_id.clone(),
        synced_via_relay: r.synced_via_relay,
    }
}

pub fn writeback_report(w: &WritebackReport) -> proto::WritebackReport {
    proto::WritebackReport {
        target_ref: w.target_ref.clone(),
        committed: w.committed,
    }
}

pub fn document_update_report(d: &crate::application::service::DocumentUpdateReport) -> proto::DocumentUpdateReport {
    proto::DocumentUpdateReport {
        r_ref: d.r_ref.clone(),
        locator: d.locator.clone(),
        revision: d.revision.clone(),
    }
}
