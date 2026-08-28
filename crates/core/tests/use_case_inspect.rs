//! Contract tests for `InspectUseCase`.

use notez_core::application::Engine;
use notez_core::application::use_cases::InspectUseCase;
use notez_core::domain::ResourceRef;
use notez_core::storage::SqliteProjection;

fn make_facade() -> (tempfile::TempDir, Engine<SqliteProjection>) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
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
    let ctx = notez_core::application::context::SourceContext::new(
        "test",
        dir.path().to_path_buf(),
        config,
    );
    (dir, Engine::with_source(store, ctx))
}

#[test]
fn inspect_rules_for_missing_resource_returns_none() {
    let (_dir, facade) = make_facade();
    let r_ref = ResourceRef::parse("document:01J0000000000000000000111A").unwrap();
    let out = <Engine<_> as InspectUseCase>::inspect_rules(&facade, &r_ref).unwrap();
    assert!(out.is_none());
}

#[test]
fn list_jobs_returns_unsupported_capability() {
    let (_dir, facade) = make_facade();
    let err = <Engine<_> as InspectUseCase>::list_jobs(&facade)
        .expect_err("list_jobs is not yet implemented");
    assert!(err.to_string().contains("unsupported capability"));
}

#[test]
fn check_artifact_freshness_returns_unsupported_capability() {
    let (_dir, facade) = make_facade();
    let err = <Engine<_> as InspectUseCase>::check_artifact_freshness(&facade)
        .expect_err("check_artifact_freshness is not yet implemented");
    assert!(err.to_string().contains("unsupported capability"));
}

#[test]
fn space_doctor_runs_against_a_missing_space_root() {
    let (_dir, facade) = make_facade();
    let result = <Engine<_> as InspectUseCase>::source_doctor(&facade);
    let _ = result;
}
