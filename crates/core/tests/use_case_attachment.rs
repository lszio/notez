//! Contract tests for `AttachmentUseCase`.

use notez_core::application::Engine;
use notez_core::application::use_cases::AttachmentUseCase;
use notez_core::domain::ResourceRef;
use notez_core::storage::SqliteProjection;

fn make_facade() -> (tempfile::TempDir, Engine<SqliteProjection>) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    (dir, Engine::new(store))
}

#[test]
fn query_segments_on_unknown_ref_returns_empty_vec() {
    let (_dir, facade) = make_facade();
    let r_ref = ResourceRef::parse("attachment:01J0000000000000000000010A").unwrap();
    let segs =
        <Engine<_> as AttachmentUseCase>::query_segments(&facade, &r_ref).unwrap();
    assert!(segs.is_empty());
}
