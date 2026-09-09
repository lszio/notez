//! Projector — journal + audit-backed wrapper around a [`ProjectionWrite`].
//!
//! Every mutation records a [`Change`] in the journal before mutating
//! the projection, then appends an [`AuditRecord`] after. Call sites
//! pass disjoint field borrows (`&mut facade.store`,
//! `facade.journal.as_ref()`, …) so the borrow checker sees them as
//! independent:
//!
//! ```ignore
//! let journaling = Journaling::from_parts(
//!     self.journal.as_ref(),
//!     self.audit.as_ref(),
//!     self.actor_principal(),
//!     self.now_unix_millis(),
//! );
//! let mut p = Projector::new(&mut self.store, &journaling);
//! p.delete_resource(r_ref)?;
//! ```
//!
//! Failure semantics: if the journal refuses the Change, nothing was
//! persisted anywhere ([`ProjectorError::Journal`]). If the store
//! rejects the mutation after the Change was recorded, the journal
//! row stands and replay semantics apply
//! ([`ProjectorError::Store`]).

use crate::domain::audit::{AuditOutcome, AuditRecord};
use crate::domain::change::{Actor, Change, ChangeOp};
use crate::domain::journal::EventJournal;
use crate::domain::query::ProjectionWrite;
use crate::domain::resource::{ResourceRef, SegmentRecord};
use crate::domain::{
    ConflictRecord, LinkOccurrence, ResolvedRelation, ResolutionStatus, Resource,
};

/// Error from a projected write.
#[derive(Debug)]
pub enum ProjectorError<E> {
    /// The journal refused the Change; no projection mutation ran.
    Journal(JournalBuildError),
    /// The Change was recorded but the store rejected the mutation.
    Store(E),
}

impl<E: std::fmt::Display> std::fmt::Display for ProjectorError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Journal(e) => write!(f, "journal: {e}"),
            Self::Store(e) => write!(f, "store: {e}"),
        }
    }
}

impl<E: std::fmt::Debug + std::fmt::Display> std::error::Error for ProjectorError<E> {}

impl<E> From<JournalBuildError> for ProjectorError<E> {
    fn from(e: JournalBuildError) -> Self {
        Self::Journal(e)
    }
}

/// Journal + audit bundle handed to every [`Projector`] call.
pub struct Journaling<'j> {
    journal: &'j dyn EventJournal,
    audit: &'j dyn crate::domain::audit::AuditLog,
    principal: String,
    now_unix_millis: i64,
}

impl<'j> Journaling<'j> {
    pub fn from_parts(
        journal: &'j dyn EventJournal,
        audit: &'j dyn crate::domain::audit::AuditLog,
        principal: String,
        now_unix_millis: i64,
    ) -> Self {
        Self {
            journal,
            audit,
            principal,
            now_unix_millis,
        }
    }

    fn record(
        &self,
        op: ChangeOp,
        source_id: String,
        targets: Vec<ResourceRef>,
        expected_revision: Option<String>,
        payload: serde_json::Value,
    ) -> Result<(Change, u64), JournalBuildError> {
        let change = Change {
            id: Change::now_id(),
            actor: Actor::new(self.principal.clone()),
            at_unix_millis: self.now_unix_millis,
            source_id,
            op,
            targets,
            expected_revision,
            payload,
        };
        let seq = self.journal.append(&change)?;
        Ok((change, seq))
    }

    /// Record a [`ChangeOp::Writeback`] Change (plus its audit row)
    /// for a mutation that hits the authoritative source *outside*
    /// the projection store — raw document writes, adapter
    /// write-backs. The projection itself is refreshed by the caller
    /// afterwards.
    pub fn record_writeback(
        &self,
        source_id: &str,
        target: ResourceRef,
        expected_revision: Option<String>,
        payload: serde_json::Value,
    ) -> Result<u64, JournalBuildError> {
        let (change, seq) = self.record(
            ChangeOp::Writeback,
            source_id.to_string(),
            vec![target],
            expected_revision,
            payload,
        )?;
        self.audit_success(&change, change.targets.first().cloned().unwrap_or(
            ResourceRef::new(crate::domain::ResourceKind::Document, ulid::Ulid::nil()),
        ));
        Ok(seq)
    }

    fn audit_success(&self, change: &Change, target: ResourceRef) {
        let record = AuditRecord {
            change_id: change.id,
            principal: change.actor.principal.clone(),
            action: format!("{:?}", change.op),
            target,
            outcome: AuditOutcome::Success,
            recorded_at_unix_millis: change.at_unix_millis,
        };
        let _ = self.audit.append(record);
    }
}

/// Failure to build/append the journal row.
#[derive(Debug)]
pub enum JournalBuildError {
    Storage(String),
}

impl std::fmt::Display for JournalBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(m) => write!(f, "storage unavailable: {m}"),
        }
    }
}

impl std::error::Error for JournalBuildError {}

impl From<crate::domain::journal::JournalError> for JournalBuildError {
    fn from(e: crate::domain::journal::JournalError) -> Self {
        Self::Storage(e.to_string())
    }
}

/// Per-write outcome: journal sequence + whether an audit row was
/// appended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteOutcome {
    pub sequence: Option<u64>,
    pub audited: bool,
}

/// Wrapper around a projection-write implementation that journals
/// before writing and audits after.
pub struct Projector<'a, 'j, S: ProjectionWrite> {
    store: &'a mut S,
    j: &'j Journaling<'j>,
}

impl<'a, 'j, S: ProjectionWrite> Projector<'a, 'j, S> {
    pub fn new(store: &'a mut S, j: &'j Journaling<'j>) -> Self {
        Self { store, j }
    }

    /// Bulk replace one source's slice; one Change for the whole
    /// transaction, one audit row per affected target. The journal op
    /// defaults to [`ChangeOp::Rebuilt`].
    pub fn replace_source(
        &mut self,
        source_id: &str,
        resources: Vec<Resource>,
        relations: Vec<crate::domain::ResourceRelation>,
        occurrences: Vec<LinkOccurrence>,
    ) -> Result<WriteOutcome, ProjectorError<S::Error>> {
        self.replace_source_op(
            ChangeOp::Rebuilt,
            source_id,
            resources,
            relations,
            occurrences,
            serde_json::Value::Null,
        )
    }

    /// [`Self::replace_source`] with an explicit journal op and a
    /// free-form detail object merged into the Change payload. Scan
    /// paths record [`ChangeOp::Scan`], task transitions record
    /// [`ChangeOp::TransitionTask`], and so on.
    pub fn replace_source_op(
        &mut self,
        op: ChangeOp,
        source_id: &str,
        resources: Vec<Resource>,
        relations: Vec<crate::domain::ResourceRelation>,
        occurrences: Vec<LinkOccurrence>,
        detail: serde_json::Value,
    ) -> Result<WriteOutcome, ProjectorError<S::Error>> {
        let targets: Vec<ResourceRef> = resources
            .iter()
            .map(|r| r.r#ref.clone())
            .chain(relations.iter().map(|r| r.target_ref.clone()))
            .collect();
        let mut payload = serde_json::json!({
            "source_id": source_id,
            "resource_count": resources.len(),
            "relation_count": relations.len(),
            "occurrence_count": occurrences.len(),
        });
        if !detail.is_null() {
            payload["detail"] = detail;
        }
        let (change, seq) =
            self.j.record(op, source_id.to_string(), targets.clone(), None, payload)?;
        self.store
            .replace_source(source_id, resources, relations, occurrences)
            .map_err(ProjectorError::Store)?;
        for t in &targets {
            self.j.audit_success(&change, t.clone());
        }
        Ok(WriteOutcome {
            sequence: Some(seq),
            audited: !targets.is_empty(),
        })
    }

    pub fn upsert_resource(
        &mut self,
        resource: &Resource,
    ) -> Result<WriteOutcome, ProjectorError<S::Error>> {
        let (change, seq) = self.j.record(
            ChangeOp::UpsertResource,
            resource.source_id.clone(),
            vec![resource.r#ref.clone()],
            Some(resource.revision.clone()),
            serde_json::to_value(resource).unwrap_or(serde_json::Value::Null),
        )?;
        self.store
            .upsert_resource(resource)
            .map_err(ProjectorError::Store)?;
        self.j.audit_success(&change, resource.r#ref.clone());
        Ok(WriteOutcome {
            sequence: Some(seq),
            audited: true,
        })
    }

    pub fn delete_resource(
        &mut self,
        r_ref: &ResourceRef,
    ) -> Result<WriteOutcome, ProjectorError<S::Error>> {
        let (change, seq) = self.j.record(
            ChangeOp::DeleteResource,
            String::new(),
            vec![r_ref.clone()],
            None,
            serde_json::Value::Null,
        )?;
        self.store
            .delete_resource(r_ref)
            .map_err(ProjectorError::Store)?;
        self.j.audit_success(&change, r_ref.clone());
        Ok(WriteOutcome {
            sequence: Some(seq),
            audited: true,
        })
    }

    pub fn insert_segments(
        &mut self,
        segments: &[SegmentRecord],
    ) -> Result<WriteOutcome, ProjectorError<S::Error>> {
        let (_change, seq) = self.j.record(
            ChangeOp::InsertSegments,
            String::new(),
            vec![],
            None,
            serde_json::json!(segments.len()),
        )?;
        self.store
            .insert_segments(segments)
            .map_err(ProjectorError::Store)?;
        Ok(WriteOutcome {
            sequence: Some(seq),
            audited: false,
        })
    }

    pub fn replace_link_occurrences(
        &mut self,
        source_id: &str,
        occurrences: Vec<LinkOccurrence>,
    ) -> Result<WriteOutcome, ProjectorError<S::Error>> {
        let (_change, seq) = self.j.record(
            ChangeOp::ReplaceLinkOccurrences,
            source_id.to_string(),
            vec![],
            None,
            serde_json::json!(occurrences.len()),
        )?;
        self.store
            .replace_link_occurrences(source_id, occurrences)
            .map_err(ProjectorError::Store)?;
        Ok(WriteOutcome {
            sequence: Some(seq),
            audited: false,
        })
    }

    pub fn replace_resolved_relations(
        &mut self,
        source_id: &str,
        relations: Vec<ResolvedRelation>,
    ) -> Result<WriteOutcome, ProjectorError<S::Error>> {
        let (_change, seq) = self.j.record(
            ChangeOp::ReplaceResolvedRelations,
            source_id.to_string(),
            vec![],
            None,
            serde_json::json!(relations.len()),
        )?;
        self.store
            .replace_resolved_relations(source_id, relations)
            .map_err(ProjectorError::Store)?;
        Ok(WriteOutcome {
            sequence: Some(seq),
            audited: false,
        })
    }

    pub fn write_link_diagnostics(
        &mut self,
        source_id: &str,
        diagnostics: &[(LinkOccurrence, ResolutionStatus, Vec<ResourceRef>)],
    ) -> Result<WriteOutcome, ProjectorError<S::Error>> {
        let (_change, seq) = self.j.record(
            ChangeOp::WriteLinkDiagnostics,
            source_id.to_string(),
            vec![],
            None,
            serde_json::json!(diagnostics.len()),
        )?;
        self.store
            .write_link_diagnostics(source_id, diagnostics)
            .map_err(ProjectorError::Store)?;
        Ok(WriteOutcome {
            sequence: Some(seq),
            audited: false,
        })
    }

    pub fn replace_conflicts(
        &mut self,
        records: &[ConflictRecord],
    ) -> Result<WriteOutcome, ProjectorError<S::Error>> {
        let (_change, seq) = self.j.record(
            ChangeOp::ReplaceConflicts,
            String::new(),
            vec![],
            None,
            serde_json::json!(records.len()),
        )?;
        self.store
            .replace_conflicts(records)
            .map_err(ProjectorError::Store)?;
        Ok(WriteOutcome {
            sequence: Some(seq),
            audited: false,
        })
    }
    /// Remove adjudicated conflict records while preserving the journal spine.
    pub fn remove_conflicts(
        &mut self,
        logical_paths: &[String],
    ) -> Result<WriteOutcome, ProjectorError<S::Error>> {
        let (_change, seq) = self.j.record(
            ChangeOp::ReplaceConflicts,
            String::new(),
            vec![],
            None,
            serde_json::json!({
                "operation": "remove",
                "logical_paths": logical_paths,
            }),
        )?;
        self.store
            .remove_conflicts(logical_paths)
            .map_err(ProjectorError::Store)?;
        Ok(WriteOutcome {
            sequence: Some(seq),
            audited: false,
        })
    }
}