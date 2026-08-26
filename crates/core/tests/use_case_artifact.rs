//! Contract tests for `ArtifactUseCase`.

use notez_core::application::ApplicationFacade;
use notez_core::domain::{ProjectionReader, ProjectionWrite};
use notez_core::application::use_cases::ArtifactUseCase;
use notez_core::storage::SqliteProjection;

fn make_facade() -> (tempfile::TempDir, ApplicationFacade<SqliteProjection>) {
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
    (dir, ApplicationFacade::with_source(store, ctx))
}

#[test]
fn derive_artifact_with_unknown_recipe_returns_error() {
    let (_dir, facade) = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let err = <ApplicationFacade<_> as ArtifactUseCase>::derive_artifact(
        &facade,
        "missing-community",
        "nope",
    )
    .expect_err("missing community must error");
    assert!(!err.to_string().is_empty());
}

#[test]
fn export_skill_with_unknown_community_returns_error() {
    let (_dir, facade) = make_facade();
    let out = _dir.path().join("SKILL.md");
    let err = <ApplicationFacade<_> as ArtifactUseCase>::export_skill(
        &facade,
        "missing-community",
        "description",
        &out,
    )
    .expect_err("missing community must error");
    assert!(!err.to_string().is_empty());
}
