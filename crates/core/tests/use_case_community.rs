//! Contract tests for `CommunityUseCase`.

use notez_core::application::Engine;
use notez_core::application::use_cases::CommunityUseCase;
use notez_core::domain::Selector;
use notez_core::domain::community::Community;
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
fn list_communities_on_empty_community_config_returns_empty() {
    let (_dir, facade) = make_facade();
    let list = <Engine<_> as CommunityUseCase>::list_communities(&facade).unwrap();
    assert!(list.is_empty());
}

#[test]
fn create_community_writes_to_disk() {
    let (_dir, facade) = make_facade();
    let community = Community {
        id: "01J0000000000000000000000C1".to_string(),
        name: "Test".to_string(),
        selector: Selector::default(),
        pinned_members: Vec::new(),
        excluded_members: Vec::new(),
    };
    let result = <Engine<_> as CommunityUseCase>::create_community(&facade, community);
    assert!(result.is_ok());
    let list = <Engine<_> as CommunityUseCase>::list_communities(&facade).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "01J0000000000000000000000C1");
}
