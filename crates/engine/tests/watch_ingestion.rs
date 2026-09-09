use notez_engine::{Action, AllowList, ApplicationError, Command, Dispatcher, EnginePorts, Scope};
use notez_ports::{SourceReader, SourceSnapshot, SourceWriteOutcome, SourceWriter, SourcePatch, WatchBatch, WatchEvent, WatchEventKind};
use std::sync::{Arc, Mutex};
use notez_protocol::request::WatchChange;

struct Source { revision: String }
impl SourceReader for Source { fn read(&self, source_id: &str) -> Result<SourceSnapshot, notez_ports::SourceError> { Ok(SourceSnapshot { source_id: source_id.into(), revision: self.revision.clone(), documents: vec![] }) } }
impl SourceWriter for Source { fn apply(&self, _: &SourceSnapshot, _: &[SourcePatch]) -> Result<SourceWriteOutcome, notez_ports::SourceError> { Ok(SourceWriteOutcome::Applied { new_revision: "r2".into() }) } }

fn dispatcher() -> Dispatcher {
    let auth = Arc::new(AllowList::default());
    auth.grant("watcher", "space", "src", Action::Ingest, Scope::Source("src".into()));
    Dispatcher::new(EnginePorts::with_ports(notez_engine::PortBundle { source: Some(Arc::new(Source { revision: "r1".into() })), journal: None, audit: None, projection: None }, auth))
}

fn command(batch_id: &str, kind: &str) -> Command {
    Command::IngestWatch(notez_protocol::IngestWatchBatch { principal: "watcher".into(), space_id: "space".into(), source_id: "src".into(), batch_id: batch_id.into(), expected_revision: "r1".into(), events: vec![WatchChange { kind: kind.into(), locator: "note.md".into(), observed_revision: "r1".into(), observed_at: "now".into() }] })
}

#[test]
fn accepted_watch_batch_is_idempotent() {
    let dispatcher = dispatcher();
    assert!(matches!(dispatcher.execute(command("b1", "modified")), Ok(notez_protocol::response::CommandResult::Applied { new_revision }) if new_revision == "accepted"));
    assert!(matches!(dispatcher.execute(command("b1", "modified")), Ok(notez_protocol::response::CommandResult::Applied { new_revision }) if new_revision == "duplicate"));
    assert_eq!(dispatcher.change_count(), 1);
}

#[test]
fn unknown_watch_event_kind_is_rejected() {
    assert!(matches!(dispatcher().execute(command("b1", "renamed")), Err(ApplicationError::InvalidRequest { .. })));
}

#[test]
fn direct_ingestion_types_remain_value_objects() {
    let batch = WatchBatch { batch_id: "b1".into(), source_id: "src".into(), events: vec![WatchEvent { locator: "note.md".into(), kind: WatchEventKind::Modified, observed_revision: "r1".into(), observed_at: "now".into() }] };
    assert_eq!(batch.events.len(), 1);
    let _ = Mutex::new(batch);
}
