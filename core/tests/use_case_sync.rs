//! Contract tests for `SyncUseCase`.

use notez_core::application::use_cases::SyncUseCase;
use notez_core::application::ApplicationFacade;
use notez_core::storage::SqliteProjection;

fn make_facade() -> (tempfile::TempDir, ApplicationFacade<SqliteProjection>) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    (dir, ApplicationFacade::new(store))
}

#[test]
fn list_conflicts_returns_unsupported_capability() {
    let (_dir, facade) = make_facade();
    let err = <ApplicationFacade<_> as SyncUseCase>::list_conflicts(&facade)
        .expect_err("list_conflicts is not yet implemented");
    assert!(err.to_string().contains("unsupported capability"));
}

#[test]
fn relay_sync_returns_unsupported_capability() {
    let (_dir, facade) = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let err = <ApplicationFacade<_> as SyncUseCase>::relay_sync(&facade, "src", dir.path())
        .expect_err("relay_sync is not yet implemented");
    assert!(err.to_string().contains("unsupported capability"));
}