//! Contract tests for `SyncUseCase`.

use notez_core::application::Engine;
use notez_core::application::use_cases::SyncUseCase;
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
        scan: Default::default(),
    };
    let ctx = notez_core::application::context::SourceContext::new(
        "test",
        dir.path().to_path_buf(),
        config,
    );
    (dir, Engine::with_source(store, ctx))
}

#[test]
fn list_conflicts_starts_empty_on_fresh_space() {
    let (_dir, facade) = make_facade();
    let conflicts = <Engine<_> as SyncUseCase>::list_conflicts(&facade)
        .expect("list_conflicts must query the persisted conflict table");
    assert!(conflicts.is_empty());
}

#[test]
fn relay_sync_returns_unsupported_capability() {
    let (_dir, facade) = make_facade();
    let err = <Engine<_> as SyncUseCase>::relay_sync(&facade)
        .expect_err("relay_sync is not yet implemented");
    assert!(err.to_string().contains("unsupported capability"));
}
