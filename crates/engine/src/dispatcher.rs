use std::sync::Arc;
use crate::{authorization::{Action, AllowList, Authorization, Principal, Scope, Space, Source}, commands::Command, ingestion::IngestionService, IngestionReceipt, PortBundle, Query, Response};
use notez_ports::{SourcePatch, SourceWriteOutcome, WatchBatch};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationError {
    StaleRevision { expected: String, actual: String },
    Forbidden { message: String },
    InvalidRequest { message: String },
    Source { message: String },
    Journal { message: String },
    Audit { message: String },
    Unsupported { capability: String },
}

pub struct EnginePorts { pub bundle: Arc<PortBundle>, pub authorization: Arc<dyn Authorization> }
impl EnginePorts {
    pub fn empty() -> Self { Self { bundle: Arc::new(PortBundle::default()), authorization: Arc::new(AllowList::default()) } }
    pub fn with_ports(bundle: PortBundle, authorization: Arc<dyn Authorization>) -> Self {
        Self { bundle: Arc::new(bundle), authorization }
    }
    pub fn from_source<S>(source: Arc<S>) -> Self where S: crate::SourcePort + 'static {
        Self::with_ports(PortBundle { source: Some(source), ..PortBundle::default() }, Arc::new(AllowList::default()))
    }
}

pub struct Dispatcher { ports: EnginePorts, ingestion: IngestionService }
impl Dispatcher {
    pub fn new(ports: EnginePorts) -> Self {
        let ingestion = IngestionService::new(ports.bundle.clone());
        Self { ports, ingestion }
    }

    pub fn execute(&self, command: Command) -> Result<notez_protocol::response::CommandResult, ApplicationError> {
        if let Some(revision) = command.expected_revision() {
            if revision.is_empty() { return Err(ApplicationError::InvalidRequest { message: "expected_revision must not be empty".into() }); }
        }
        match command {
            Command::UpdateDocument { principal, space_id, source_id, document_path, content, expected_revision } => {
                self.authorize(&principal, &space_id, &source_id, Action::Write, Scope::Object(document_path.clone()))?;
                let source = self.ports.bundle.source.as_ref().ok_or_else(|| ApplicationError::Unsupported { capability: "source_writer".into() })?;
                let snapshot = source.read(&source_id).map_err(|e| ApplicationError::Source { message: format!("{e:?}") })?;
                if snapshot.revision != expected_revision { return Err(ApplicationError::StaleRevision { expected: expected_revision, actual: snapshot.revision }); }
                self.append_journal(b"update_document")?;
                let outcome = source.apply(&snapshot, &[SourcePatch { locator: document_path, content }]).map_err(|e| ApplicationError::Source { message: format!("{e:?}") })?;
                match outcome {
                    SourceWriteOutcome::Applied { new_revision } => {
                        self.record_audit("dispatcher", "update_document", notez_ports::AuditOutcome::Applied, None)?;
                        Ok(notez_protocol::response::CommandResult::Applied { new_revision })
                    }
                    SourceWriteOutcome::Stale { current_revision } => Err(ApplicationError::StaleRevision { expected: snapshot.revision, actual: current_revision }),
                    SourceWriteOutcome::Unsupported => Err(ApplicationError::Unsupported { capability: "source_writer".into() }),
                }
            }
            Command::IngestWatch(batch) => {
                self.authorize(&batch.principal, &batch.space_id, &batch.source_id, Action::Ingest, Scope::Source(batch.source_id.clone()))?;
                let expected_revision = batch.expected_revision;
                let mut events = Vec::with_capacity(batch.events.len());
                for event in batch.events {
                    let kind = match event.kind.as_str() {
                        "created" => notez_ports::WatchEventKind::Created,
                        "modified" => notez_ports::WatchEventKind::Modified,
                        "removed" => notez_ports::WatchEventKind::Removed,
                        _ => return Err(ApplicationError::InvalidRequest { message: "unknown watch event kind".into() }),
                    };
                    events.push(notez_ports::WatchEvent { locator: event.locator, kind, observed_revision: event.observed_revision, observed_at: event.observed_at });
                }
                let batch = WatchBatch { batch_id: batch.batch_id, source_id: batch.source_id, events };
                match self.ingestion.accept_with_revision(batch, Some(&expected_revision)) {
                    IngestionReceipt::Accepted => Ok(notez_protocol::response::CommandResult::Applied { new_revision: "accepted".into() }),
                    IngestionReceipt::Duplicate => Ok(notez_protocol::response::CommandResult::Applied { new_revision: "duplicate".into() }),
                    IngestionReceipt::Rejected => Err(ApplicationError::InvalidRequest { message: "invalid watch batch".into() }),
                    IngestionReceipt::Retryable => Err(ApplicationError::Source { message: "retryable ingestion failure".into() }),
                }
            }
            _ => Err(ApplicationError::Unsupported { capability: "command".into() }),
        }
    }

    pub fn query(&self, query: Query) -> Result<Response, ApplicationError> {
        let capability = match query {
            Query::ListSpaces { .. } => "list_spaces", Query::InspectSpace { .. } => "inspect_space", Query::ListDocuments { .. } => "list_documents",
            Query::GetDocument { .. } => "get_document", Query::ResolveAddress { .. } => "resolve_address", Query::GetObject { .. } => "get_object",
            Query::Backlinks { .. } => "backlinks", Query::Graph { .. } => "graph", Query::GetSummary { .. } => "get_summary", Query::WatchStatus { .. } => "watch_status",
        };
        Err(ApplicationError::Unsupported { capability: capability.into() })
    }

    pub fn change_count(&self) -> usize { self.ingestion.change_count() }

    fn authorize(&self, principal: &str, space: &str, source: &str, action: Action, scope: Scope) -> Result<(), ApplicationError> {
        self.ports.authorization.check(&Principal(principal.into()), &Space(space.into()), &Source(source.into()), &action, &scope)
            .map_err(|e| ApplicationError::Forbidden { message: format!("{e:?}") })
    }
    fn append_journal(&self, bytes: &[u8]) -> Result<(), ApplicationError> {
        if let Some(journal) = &self.ports.bundle.journal { journal.append(bytes).map_err(|e| ApplicationError::Journal { message: format!("{e:?}") })?; }
        Ok(())
    }
    fn record_audit(&self, principal: &str, operation: &str, outcome: notez_ports::AuditOutcome, diagnostic: Option<String>) -> Result<(), ApplicationError> {
        if let Some(audit) = &self.ports.bundle.audit {
            audit.record(&notez_ports::AuditRecord { principal: principal.into(), operation: operation.into(), outcome, diagnostic })
                .map_err(|e| ApplicationError::Audit { message: format!("{e:?}") })?;
        }
        Ok(())
    }
}
