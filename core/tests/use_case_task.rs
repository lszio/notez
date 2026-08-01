//! Contract tests for `TaskUseCase`.

use notez_core::application::use_cases::TaskUseCase;
use notez_core::application::ApplicationFacade;
use notez_core::domain::ResourceRef;
use notez_core::storage::SqliteProjection;

fn make_facade() -> (tempfile::TempDir, ApplicationFacade<SqliteProjection>) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    (dir, ApplicationFacade::new(store))
}

#[test]
fn agenda_on_empty_projection_is_empty() {
    let (_dir, facade) = make_facade();
    let view = <ApplicationFacade<_> as TaskUseCase>::agenda(&facade).unwrap();
    assert!(view.items.is_empty());
}

#[test]
fn para_overview_on_empty_projection_has_four_empty_buckets() {
    let (_dir, facade) = make_facade();
    let view = <ApplicationFacade<_> as TaskUseCase>::para_overview(&facade).unwrap();
    assert!(view.projects.is_empty());
    assert!(view.areas.is_empty());
    assert!(view.resources.is_empty());
    assert!(view.archives.is_empty());
}

#[test]
fn transition_task_on_missing_resource_returns_not_found() {
    let (_dir, mut facade) = make_facade();
    let r_ref = ResourceRef::parse("heading:01J000000000000000000000F1").unwrap();
    let err = <ApplicationFacade<_> as TaskUseCase>::transition_task(&mut facade, &r_ref, "DONE", "2026-08-02")
        .expect_err("transition on missing resource must fail");
    assert!(err.to_string().contains("not found") || err.to_string().contains("01J000000000000000000000F1"));
}