//! Contract tests for `InspectUseCase`.

use notez_core::application::use_cases::InspectUseCase;
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
fn inspect_rules_for_missing_resource_returns_none() {
    let (_dir, facade) = make_facade();
    let r_ref = ResourceRef::parse("document:01J0000000000000000000111A").unwrap();
    let out = <ApplicationFacade<_> as InspectUseCase>::inspect_rules(&facade, &r_ref).unwrap();
    assert!(out.is_none());
}

#[test]
fn list_jobs_returns_unsupported_capability() {
    let (_dir, facade) = make_facade();
    let err = <ApplicationFacade<_> as InspectUseCase>::list_jobs(&facade)
        .expect_err("list_jobs is not yet implemented");
    assert!(err.to_string().contains("unsupported capability"));
}

#[test]
fn check_artifact_freshness_returns_unsupported_capability() {
    let (_dir, facade) = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let err = <ApplicationFacade<_> as InspectUseCase>::check_artifact_freshness(&facade, dir.path())
        .expect_err("check_artifact_freshness is not yet implemented");
    assert!(err.to_string().contains("unsupported capability"));
}

#[test]
fn space_doctor_runs_against_a_missing_space_root() {
    let (_dir, facade) = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let result = <ApplicationFacade<_> as InspectUseCase>::space_doctor(&facade, dir.path());
    let _ = result;
}