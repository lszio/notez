//! Write-spine acceptance tests (docs/refactoring-v1.org §5/§8/§14).
//!
//! Covers:
//! * stale `expected_revision` rejection on upsert / delete /
//!   transition / update_document, with structured
//!   `RevisionConflict` errors;
//! * every migrated write path producing `event_journal` entries,
//!   asserted through an in-memory journal fake's `since()`;
//! * `query_resources.limit` pushed down into the store query.

use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_core::application::use_cases::{ResourceUseCase, ScanUseCase, SyncUseCase};
use notez_core::application::{ApplicationError, Engine};
use notez_core::domain::change::ChangeOp;
use notez_core::domain::journal::{EventJournal, JournalEntry, JournalError};
use notez_core::domain::{ResourceRef, Selector};
use notez_core::storage::SqliteProjection;
use notez_protocol::request::{
    DeleteResourceRequest, ExtractAttachmentRequest, AddAttachmentRequest, QueryResourcesRequest,
    Request, ResolveLinksRequest, ScanFederationRequest, ScanNativeRequest,
    TransitionTaskRequest, UpdateDocumentRequest, UpsertResourceRequest,
};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;

// ---- journal fake -----------------------------------------------------------

#[derive(Default)]
struct MemJournalInner {
    entries: Vec<(u64, notez_core::domain::change::Change)>,
}

/// Shareable in-memory journal fake. Clones observe the same log.
#[derive(Clone, Default)]
struct MemJournal {
    inner: Arc<parking_lot::Mutex<MemJournalInner>>,
}

impl EventJournal for MemJournal {
    fn append(&self, change: &notez_core::domain::change::Change) -> Result<u64, JournalError> {
        let mut guard = self.inner.lock();
        let seq = guard.entries.len() as u64 + 1;
        guard.entries.push((seq, change.clone()));
        Ok(seq)
    }

    fn since(&self, cursor: u64) -> Result<Vec<JournalEntry>, JournalError> {
        let guard = self.inner.lock();
        Ok(guard
            .entries
            .iter()
            .filter(|(seq, _)| *seq >= cursor)
            .map(|(seq, change)| JournalEntry {
                sequence: *seq,
                change: change.clone(),
            })
            .collect())
    }

    fn len(&self) -> Result<u64, JournalError> {
        Ok(self.inner.lock().entries.len() as u64)
    }

    fn activity(&self, limit: usize) -> Result<Vec<notez_core::domain::journal::ActivityRecord>, JournalError> {
        Ok(self.since(0)?.into_iter().rev().take(limit).map(|entry| notez_core::domain::journal::ActivityRecord {
            sequence: entry.sequence, change: entry.change, audited: true,
        }).collect())
    }
}

impl MemJournal {
    fn clear(&self) {
        self.inner.lock().entries.clear();
    }

    fn ops(&self) -> Vec<ChangeOp> {
        self.inner.lock().entries.iter().map(|(_, change)| change.op.clone()).collect()
    }

    fn changes(&self) -> Vec<notez_core::domain::change::Change> {
        self.inner.lock().entries.iter().map(|(_, change)| change.clone()).collect()
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}


// ---- fixture ----------------------------------------------------------------

struct Space {
    dir: tempfile::TempDir,
    service: Engine<SqliteProjection>,
    journal: MemJournal,
}

fn build_service(root: &Path, db_path: &Path, journal: MemJournal) -> Engine<SqliteProjection> {
    let store = SqliteProjection::open(db_path).unwrap();
    let config = notez_core::config::model::SourceConfig {
        version: 2,
        source: notez_core::config::model::SourceIdentity {
            name: "test".into(),
            database: std::path::PathBuf::from(".notez/index.sqlite"),

        },
        workflow: Default::default(),
        sources: vec![],
        link_overrides: serde_json::Value::Null,
    };
    let ctx = notez_core::application::context::SourceContext::new("test", root.to_path_buf(), config);
    let mut service = Engine::with_source(store, ctx);
    service.register_format_parser(Box::new(orgmode::OrgParser::new()));
    service.register_format_parser(Box::new(markdown::MarkdownParser::new()));
    service.attach_journal(journal.clone());
    service
}

fn space_with(files: &[(&str, &str)]) -> Space {
    let dir = tempfile::tempdir().unwrap();
    for (path, content) in files {
        let full = dir.path().join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, content).unwrap();
    }
    std::fs::create_dir_all(dir.path().join(".notez")).unwrap();
    let journal = MemJournal::default();
    let service = build_service(dir.path(), &dir.path().join(".notez/index.sqlite"), journal.clone());
    Space { dir, service, journal }
}

impl Space {
    fn dispatch(&mut self, req: Request) -> Result<Response, ApplicationError> {
        ApplicationDispatcher::new(&mut self.service).dispatch(req)
    }

    fn root(&self) -> std::path::PathBuf {
        self.dir.path().to_path_buf()
    }
}

fn resource_payload(ref_str: &str, kind: &str, revision: &str) -> notez_protocol::request::ResourcePayload {
    notez_protocol::request::ResourcePayload {
        ref_: ref_str.to_string(),
        kind: kind.to_string(),
        title: "t".to_string(),
        revision: revision.to_string(),
        source_id: "native".to_string(),
        locator: "/x.org".to_string(),
        properties: Default::default(),
        object_id: None,
        primary_source_id: String::new(),
    }
}

fn err_json(err: &ApplicationError) -> serde_json::Value {
    serde_json::to_value(err).expect("ApplicationError must serialize")
}

fn expect_page(resp: Response) -> notez_protocol::response::QueryPage {
    match resp {
        Response::ResourcePage(page) => page,
        other => panic!("expected ResourcePage, got {other:?}"),
    }
}

// ---- (a) stale revision rejection --------------------------------------------

#[test]
fn stale_revision_rejects_upsert_delete_transition_update_document() {
    let mut sp = space_with(&[]);

    // Seed one row at revision r1.
    let r_ref = ResourceRef::parse("heading:01J000000000000000000000F1").unwrap();
    sp.dispatch(Request::UpsertResource(UpsertResourceRequest {
        resource: resource_payload(&r_ref.to_string(), "heading", "r1"),
        expected_revision: None,
    }))
    .unwrap();

    // Upsert with a stale guard is rejected with structured fields.
    let err = sp
        .dispatch(Request::UpsertResource(UpsertResourceRequest {
            resource: resource_payload(&r_ref.to_string(), "heading", "r1"),
            expected_revision: Some("stale2".to_string()),
        }))
        .err()
        .expect("stale upsert must fail");
    assert!(matches!(
        err,
        ApplicationError::RevisionConflict { ref expected, ref actual }
            if expected == "stale2" && actual == "r1"
    ));
    let json = err_json(&err);
    assert_eq!(json["kind"], "revision_conflict");
    assert_eq!(json["expected"], "stale2");
    assert_eq!(json["actual"], "r1");

    // Delete with a stale guard is rejected; without a guard it succeeds.
    assert!(matches!(
        sp.dispatch(Request::DeleteResource(DeleteResourceRequest {
            r_ref: r_ref.to_string(),
            expected_revision: Some("nope".to_string()),
        })),
        Err(ApplicationError::RevisionConflict { .. })
    ));
    sp.dispatch(Request::DeleteResource(DeleteResourceRequest {
        r_ref: r_ref.to_string(),
        expected_revision: None,
    }))
    .unwrap();

    // Transition with a stale guard is rejected before any write.
    std::fs::write(sp.root().join("tasks.org"), "#+title: Tasks\n\n* NEXT spine task\n").unwrap();
    <Engine<SqliteProjection> as ScanUseCase>::scan_native(&mut sp.service).unwrap();
    let page = <Engine<SqliteProjection> as ResourceUseCase>::query(
        &sp.service,
        &Selector::new().with_title_contains("spine task"),
    )
    .unwrap();
    let task_row = page.items.iter().find(|r| r.properties.contains_key("TODO")).expect("task heading");
    let task_rev = task_row.revision.clone();
    let task_ref = task_row.r#ref.to_string();
    assert!(matches!(
        sp.dispatch(Request::TransitionTask(TransitionTaskRequest {
            r_ref: task_ref.clone(),
            to_state: "DONE".to_string(),
            timestamp: Some("[2026-08-26 Wed 10:00]".to_string()),
            expected_revision: Some("not-the-revision".to_string()),
        })),
        Err(ApplicationError::RevisionConflict { ref expected, ref actual })
            if expected == "not-the-revision" && *actual == task_rev
    ));

    // update_document with a stale base_revision is rejected; the actual
    // side is the content hash of the raw file bytes.
    std::fs::write(sp.root().join("hello.md"), "# Hello\n\nfirst\n").unwrap();
    <Engine<SqliteProjection> as ScanUseCase>::scan_native(&mut sp.service).unwrap();
    let err = sp
        .dispatch(Request::UpdateDocument(UpdateDocumentRequest {
            source_id: "native".to_string(),
            locator: "hello.md".to_string(),
            content: "# Hello\n\nsecond\n".to_string(),
            base_revision: Some("deadbeef".to_string()),
            format: None,
            expected_revision: None,
        }))
        .err()
        .expect("stale update must fail");
    assert!(matches!(
        err,
        ApplicationError::RevisionConflict { ref expected, ref actual }
            if expected == "deadbeef"
                && *actual == sha256_hex(b"# Hello\n\nfirst\n")
    ));
}
#[test]
fn markdown_update_through_dispatcher_commits_revision_and_journal_metadata() {
    let mut sp = space_with(&[("acceptance.md", "# Acceptance\n\noriginal bytes\n")]);
    sp.dispatch(Request::ScanNative(ScanNativeRequest {})).unwrap();
    let page = expect_page(sp.dispatch(Request::QueryResources(QueryResourcesRequest {
        kind: None,
        title_contains: None,
        exact_ref: None,
        source_id: Some("native".to_string()),
        limit: None,
    })).unwrap());
    let original = page.items.iter().find(|row| row.locator == "acceptance.md").expect("scanned Markdown document");
    let old_revision = original.revision.clone();
    let new_content = "# Acceptance\n\nupdated bytes\n";
    sp.journal.clear();

    let response = sp.dispatch(Request::UpdateDocument(UpdateDocumentRequest {
        source_id: "native".to_string(),
        locator: "acceptance.md".to_string(),
        content: new_content.to_string(),
        base_revision: None,
        format: None,
        expected_revision: Some(old_revision.clone()),
    })).expect("matching revision must permit writeback");
    let Response::DocumentUpdated(report) = response else { panic!("expected DocumentUpdated response"); };
    assert_eq!(std::fs::read_to_string(sp.root().join("acceptance.md")).unwrap(), new_content);
    assert_eq!(report.revision, sha256_hex(new_content.as_bytes()));
    assert_ne!(report.revision, old_revision);

    let writeback = sp.journal.changes().into_iter().find(|change| matches!(change.op, ChangeOp::Writeback)).expect("writeback journal entry");
    assert_eq!(writeback.expected_revision.as_deref(), Some(old_revision.as_str()));
    assert_eq!(writeback.payload["locator"], "acceptance.md");
}

#[test]
fn markdown_external_change_rejects_stale_dispatcher_write_without_overwrite() {
    let mut sp = space_with(&[("external.md", "# External\n\nindexed bytes\n")]);
    sp.dispatch(Request::ScanNative(ScanNativeRequest {})).unwrap();
    let page = expect_page(sp.dispatch(Request::QueryResources(QueryResourcesRequest {
        kind: None,
        title_contains: None,
        exact_ref: None,
        source_id: Some("native".to_string()),
        limit: None,
    })).unwrap());
    let row = page.items.iter().find(|row| row.locator == "external.md").expect("scanned Markdown document");
    let indexed_revision = row.revision.clone();
    let external = "# External\n\nchanged outside Notez\n";
    std::fs::write(sp.root().join("external.md"), external).unwrap();

    let err = sp.dispatch(Request::UpdateDocument(UpdateDocumentRequest {
        source_id: "native".to_string(),
        locator: "external.md".to_string(),
        content: "# External\n\nNotez attempted overwrite\n".to_string(),
        base_revision: None,
        format: None,
        expected_revision: Some(indexed_revision),
    })).expect_err("stale revision must reject external-change overwrite");
    assert!(matches!(err, ApplicationError::RevisionConflict { .. }));
    assert_eq!(std::fs::read_to_string(sp.root().join("external.md")).unwrap(), external);
}

#[test]
fn failed_markdown_patch_leaves_original_bytes_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("patch.md");
    let original = "# Patch\n\nkeep these bytes\n";
    std::fs::write(&path, original).unwrap();
    let bad = notez_core::source::writer::TextPatch::replace_line(
        1,
        "# Different snapshot",
        "# Mutated",
    );
    assert!(notez_core::source::writer::FsSpanWriter::apply(&path, &[bad]).is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), original);
}

// ---- (b) journal entries per path ---------------------------------------------

#[test]
fn scan_federation_journals_scan_changes() {
    let mut sp = space_with(&[("a.md", "# A\n\nbody a\n")]);
    sp.journal.clear();

    sp.dispatch(Request::ScanFederation(ScanFederationRequest {}))
        .unwrap();

    assert!(
        sp.journal.ops().iter().any(|op| matches!(op, ChangeOp::Scan)),
        "scan_federation must journal Scan changes"
    );
}

#[test]
fn update_document_journals_writeback_and_rescan() {
    let mut sp = space_with(&[("a.md", "# A\n\nbody a\n")]);
    <Engine<SqliteProjection> as ScanUseCase>::scan_native(&mut sp.service).unwrap();

    let resp = sp
        .dispatch(Request::UpdateDocument(UpdateDocumentRequest {
            source_id: "native".to_string(),
            locator: "a.md".to_string(),
            content: "# A\n\nbody a edited\n".to_string(),
            base_revision: None,
            format: None,
            expected_revision: None,
        }))
        .unwrap();
    let Response::DocumentUpdated(report) = resp else {
        panic!("expected DocumentUpdated");
    };

    let ops = sp.journal.ops();
    assert!(
        ops.iter().any(|op| matches!(op, ChangeOp::Writeback)),
        "update_document must journal a Writeback change"
    );
    assert!(
        ops.iter().any(|op| matches!(op, ChangeOp::Scan)),
        "update_document rescan must journal Scan changes"
    );

    // File updated atomically; report revision matches the new content hash.
    let on_disk = std::fs::read_to_string(sp.root().join("a.md")).unwrap();
    assert_eq!(on_disk, "# A\n\nbody a edited\n");
    assert_eq!(
        report.revision,
        sha256_hex(b"# A\n\nbody a edited\n"),
        "report revision must be the content hash of the new bytes"
    );
    assert_eq!(report.locator, "a.md");
}

#[test]
fn transition_task_journals_transition_op() {
    let mut sp = space_with(&[(
        "tasks.org",
        "#+title: Tasks\n\n* NEXT spine transition\n",
    )]);
    <Engine<SqliteProjection> as ScanUseCase>::scan_native(&mut sp.service).unwrap();
    let page = <Engine<SqliteProjection> as ResourceUseCase>::query(
        &sp.service,
        &Selector::new().with_title_contains("spine transition"),
    )
    .unwrap();
    let task_ref = page
        .items
        .iter()
        .find(|r| r.properties.contains_key("TODO"))
        .expect("task heading")
        .r#ref
        .to_string();

    sp.journal.clear();
    let resp = sp
        .dispatch(Request::TransitionTask(TransitionTaskRequest {
            r_ref: task_ref,
            to_state: "DONE".to_string(),
            timestamp: Some("[2026-08-26 Wed 10:00]".to_string()),
            expected_revision: None,
        }))
        .expect("transition must succeed");
    assert!(matches!(resp, Response::Transition(_)));

    let ops = sp.journal.ops();
    assert!(
        ops.iter().any(
            |op| matches!(op, ChangeOp::TransitionTask { to_state, .. } if to_state == "DONE")
        ),
        "transition_task must journal a TransitionTask change; got {ops:?}"
    );
}

#[test]
fn resolve_links_journals_relations_and_diagnostics() {
    let mut sp = space_with(&[("notes/source.md", "[[target]]\n"), ("notes/target.md", "# target\n")]);
    sp.dispatch(Request::ScanNative(ScanNativeRequest {})).unwrap();

    sp.journal.clear();
    let page = expect_page(
        sp.dispatch(Request::QueryResources(QueryResourcesRequest {
            kind: None,
            title_contains: Some("source".to_string()),
            exact_ref: None,
            source_id: None,
            limit: None,
        }))
        .unwrap(),
    );
    let src_ref = page.items[0].ref_.clone();

    sp.dispatch(Request::ResolveLinks(ResolveLinksRequest { source_ref: src_ref }))
        .expect("resolve_links must succeed");

    let ops = sp.journal.ops();
    assert!(
        ops.iter().any(|op| matches!(op, ChangeOp::ReplaceResolvedRelations)),
        "resolve_links must journal resolved relations"
    );
    assert!(
        ops.iter().any(|op| matches!(op, ChangeOp::WriteLinkDiagnostics)),
        "resolve_links must journal link diagnostics"
    );
}

#[test]
fn attachment_add_and_extraction_journal_changes() {
    let mut sp = space_with(&[("attach.txt", "some extractable text payload\n")]);
    sp.journal.clear();

    let resp = sp
        .dispatch(Request::AddAttachment(AddAttachmentRequest {
            file_path: sp.root().join("attach.txt").display().to_string(),
            mime: Some("text/plain".to_string()),
        }))
        .unwrap();
    let Response::AttachmentRef(att_ref) = resp else {
        panic!("expected AttachmentRef");
    };
    assert!(att_ref.starts_with("attachment:"));

    sp.dispatch(Request::ExtractAttachment(ExtractAttachmentRequest {
        source_ref: att_ref,
    }))
    .expect("extraction must succeed");

    let ops = sp.journal.ops();
    assert!(
        ops.iter().any(|op| matches!(op, ChangeOp::InsertSegments)),
        "run_extraction must journal InsertSegments; got {ops:?}"
    );
    assert!(
        ops.iter().any(|op| matches!(op, ChangeOp::Rebuilt)),
        "add_attachment must journal its native-slice replace"
    );
}

/// Stage a genuine three-way conflict in the shared folder: one incoming
/// manifest whose base differs from both the local text and the incoming
/// text, forcing `ThreeWayMerger` into the conflict branch.
#[test]
fn sync_pull_and_resolve_conflict_journal_changes() {
    const MINE: &str = "mine-line\n";
    const BASE: &str = "base-line\n";
    const THEIRS: &str = "theirs-line\n";

    let shared = tempfile::tempdir().unwrap();
    let objects = shared.path().join("objects");
    std::fs::create_dir_all(&objects).unwrap();
    for text in [MINE, BASE, THEIRS] {
        std::fs::write(objects.join(sha256_hex(text.as_bytes())), text).unwrap();
    }
    std::fs::create_dir_all(shared.path().join("manifests")).unwrap();
    let manifest = serde_json::json!({
        "source_id": "default_source",
        "actor_id": "b",
        "parent_snapshots": [sha256_hex(BASE.as_bytes())],
        "logical_path": "note.md",
        "content_hash": sha256_hex(THEIRS.as_bytes()),
        "properties": {},
    });
    std::fs::write(
        shared.path().join("manifests/note_md.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let mut sp = space_with(&[("note.md", MINE)]);
    sp.journal.clear();

    let resp = sp
        .dispatch(Request::SyncPull(notez_protocol::request::SyncPullRequest {
            actor_id: "a".to_string(),
            folder: shared.path().display().to_string(),
        }))
        .expect("pull must succeed");
    let Response::Pulled(report) = resp else {
        panic!("expected Pulled");
    };
    assert_eq!(report.conflicts.len(), 1, "staged divergence must conflict");

    let ops = sp.journal.ops();
    assert!(
        ops.iter().any(|op| matches!(op, ChangeOp::ReplaceConflicts)),
        "sync_pull with conflicts must journal ReplaceConflicts; got {ops:?}"
    );
    assert!(
        ops.iter().any(|op| matches!(op, ChangeOp::Scan)),
        "sync_pull rescans the projection, journaling Scan changes"
    );

    // resolve_conflict clears the adjudicated record through the Projector.
    sp.journal.clear();
    let pending =
        <Engine<SqliteProjection> as SyncUseCase>::list_conflicts(&mut sp.service)
            .unwrap();
    assert_eq!(pending.len(), 1);
    let report = <Engine<SqliteProjection> as SyncUseCase>::resolve_conflict(
        &mut sp.service,
        shared.path(),
        "note.md",
        true,
    )
    .expect("resolve_conflict must succeed");
    assert!(report.committed);

    let ops = sp.journal.ops();
    assert!(
        ops.iter().any(|op| matches!(op, ChangeOp::ReplaceConflicts)),
        "resolve_conflict must journal ReplaceConflicts; got {ops:?}"
    );
    assert!(
        <Engine<SqliteProjection> as SyncUseCase>::list_conflicts(&mut sp.service)
            .unwrap()
            .is_empty(),
        "adjudicated conflict must be cleared"
    );
}

// ---- (c) limit push-down --------------------------------------------------------

#[test]
fn query_resources_limit_is_pushed_down() {
    let mut sp = space_with(&[]);
    for i in 0..5 {
        let r_ref = ResourceRef::parse(&format!("heading:01J000000000000000000000F{i}")).unwrap();
        sp.dispatch(Request::UpsertResource(UpsertResourceRequest {
            resource: resource_payload(&r_ref.to_string(), "heading", &format!("r{i}")),
            expected_revision: None,
        }))
        .unwrap();
    }

    let page = expect_page(
        sp.dispatch(Request::QueryResources(QueryResourcesRequest {
            kind: None,
            title_contains: None,
            exact_ref: None,
            source_id: None,
            limit: Some(2),
        }))
        .unwrap(),
    );
    assert_eq!(page.items.len(), 2, "limit must reach the SQL LIMIT clause");
    assert!(page.next_cursor.is_none());

    let page = expect_page(
        sp.dispatch(Request::QueryResources(QueryResourcesRequest {
            kind: None,
            title_contains: None,
            exact_ref: None,
            source_id: None,
            limit: None,
        }))
        .unwrap(),
    );
    assert_eq!(page.items.len(), 5);
}
