//! Contract tests for `ArtifactUseCase`.

use notez_core::application::use_cases::ArtifactUseCase;
use notez_core::application::ApplicationFacade;
use notez_core::storage::SqliteProjection;

fn make_facade() -> (tempfile::TempDir, ApplicationFacade<SqliteProjection>) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    (dir, ApplicationFacade::new(store))
}

#[test]
fn derive_artifact_with_unknown_recipe_returns_error() {
    let (_dir, facade) = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let err = <ApplicationFacade<_> as ArtifactUseCase>::derive_artifact(
        &facade,
        dir.path(),
        "missing-community",
        "nope",
    )
    .expect_err("missing community must error");
    assert!(!err.to_string().is_empty());
}

#[test]
fn export_skill_with_unknown_community_returns_error() {
    let (_dir, facade) = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("SKILL.md");
    let err = <ApplicationFacade<_> as ArtifactUseCase>::export_skill(
        &facade,
        dir.path(),
        "missing-community",
        "description",
        &out,
    )
    .expect_err("missing community must error");
    assert!(!err.to_string().is_empty());
}