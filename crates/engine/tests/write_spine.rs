use notez_engine::{AllowList, ApplicationError, Command, Dispatcher, EnginePorts};
use notez_ports::{AuditError, AuditLog, AuditRecord, ChangeJournal, JournalEntry, JournalError, SourcePatch, SourceReader, SourceSnapshot, SourceWriteOutcome, SourceWriter};
use std::sync::{Arc, Mutex};

struct Writer { calls: Mutex<usize>, revision: String }
impl SourceReader for Writer {
    fn read(&self, source_id: &str) -> Result<SourceSnapshot, notez_ports::SourceError> { Ok(SourceSnapshot { source_id: source_id.into(), revision: self.revision.clone(), documents: vec![] }) }
}
impl SourceWriter for Writer {
    fn apply(&self, _: &SourceSnapshot, _: &[SourcePatch]) -> Result<SourceWriteOutcome, notez_ports::SourceError> { *self.calls.lock().unwrap() += 1; Ok(SourceWriteOutcome::Applied { new_revision: "r3".into() }) }
}

struct Journal { calls: Mutex<Vec<&'static str>>, fail: bool }
impl ChangeJournal for Journal {
    fn append(&self, _: &[u8]) -> Result<JournalEntry, JournalError> { self.calls.lock().unwrap().push("journal"); if self.fail { Err(JournalError::Unavailable { message: "down".into() }) } else { Ok(JournalEntry { sequence: 1, change_id: "c1".into(), payload: vec![] }) } }
    fn since(&self, _: u64) -> Result<Vec<JournalEntry>, JournalError> { Ok(vec![]) }
}
struct Audit { calls: Mutex<Vec<&'static str>> }
impl AuditLog for Audit { fn record(&self, _: &AuditRecord) -> Result<(), AuditError> { self.calls.lock().unwrap().push("audit"); Ok(()) } }

fn authorized_engine(writer: Arc<Writer>, journal: Arc<Journal>, audit: Arc<Audit>) -> Dispatcher {
    let auth = Arc::new(AllowList::default());
    auth.grant("alice", "space", "src", notez_engine::Action::Write, notez_engine::Scope::Object("note.md".into()));
    let bundle = notez_engine::PortBundle { source: Some(writer), journal: Some(journal), audit: Some(audit), projection: None };
    Dispatcher::new(EnginePorts::with_ports(bundle, auth))
}

#[test]
fn stale_revision_never_reaches_source_writer() {
    let writer = Arc::new(Writer { calls: Mutex::new(0), revision: "r2".into() });
    let auth = Arc::new(AllowList::default());
    auth.grant("alice", "space", "src", notez_engine::Action::Write, notez_engine::Scope::Object("note.md".into()));
    let ports = EnginePorts::with_ports(notez_engine::PortBundle { source: Some(writer.clone()), journal: None, audit: None, projection: None }, auth);
    let dispatcher = Dispatcher::new(ports);
    let result = dispatcher.execute(Command::UpdateDocument { principal: "alice".into(), space_id: "space".into(), source_id: "src".into(), document_path: "note.md".into(), content: "new".into(), expected_revision: "r1".into() });
    assert!(matches!(result, Err(ApplicationError::StaleRevision { .. })));
    assert_eq!(*writer.calls.lock().unwrap(), 0);
}

#[test]
fn update_document_rejects_empty_revision_even_when_constructed_in_rust() {
    let writer = Arc::new(Writer { calls: Mutex::new(0), revision: "r1".into() });
    let dispatcher = Dispatcher::new(EnginePorts::from_source(writer.clone()));
    let result = dispatcher.execute(Command::UpdateDocument { principal: "alice".into(), space_id: "space".into(), source_id: "src".into(), document_path: "note.md".into(), content: "new".into(), expected_revision: String::new() });
    assert!(matches!(result, Err(ApplicationError::InvalidRequest { .. })));
    assert_eq!(*writer.calls.lock().unwrap(), 0);
}

#[test]
fn journal_failure_prevents_source_write_and_audit() {
    let writer = Arc::new(Writer { calls: Mutex::new(0), revision: "r1".into() });
    let journal = Arc::new(Journal { calls: Mutex::new(vec![]), fail: true });
    let audit = Arc::new(Audit { calls: Mutex::new(vec![]) });
    let dispatcher = authorized_engine(writer.clone(), journal, audit.clone());
    let result = dispatcher.execute(Command::UpdateDocument { principal: "alice".into(), space_id: "space".into(), source_id: "src".into(), document_path: "note.md".into(), content: "new".into(), expected_revision: "r1".into() });
    assert!(matches!(result, Err(ApplicationError::Journal { .. })));
    assert_eq!(*writer.calls.lock().unwrap(), 0);
    assert!(audit.calls.lock().unwrap().is_empty());
}

#[test]
fn journal_precedes_source_and_audit() {
    let writer = Arc::new(Writer { calls: Mutex::new(0), revision: "r1".into() });
    let journal = Arc::new(Journal { calls: Mutex::new(vec![]), fail: false });
    let audit = Arc::new(Audit { calls: Mutex::new(vec![]) });
    let dispatcher = authorized_engine(writer.clone(), journal.clone(), audit.clone());
    dispatcher.execute(Command::UpdateDocument { principal: "alice".into(), space_id: "space".into(), source_id: "src".into(), document_path: "note.md".into(), content: "new".into(), expected_revision: "r1".into() }).unwrap();
    assert_eq!(journal.calls.lock().unwrap().as_slice(), &["journal"]);
    assert_eq!(audit.calls.lock().unwrap().as_slice(), &["audit"]);
}
