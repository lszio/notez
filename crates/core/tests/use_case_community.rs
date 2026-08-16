//! Contract tests for `CommunityUseCase`.

use notez_core::application::use_cases::CommunityUseCase;
use notez_core::application::ApplicationFacade;
use notez_core::domain::community::Community;
use notez_core::domain::Selector;
use notez_core::storage::SqliteProjection;

fn make_facade() -> (tempfile::TempDir, ApplicationFacade<SqliteProjection>) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db).unwrap();
    (dir, ApplicationFacade::new(store))
}

#[test]
fn list_communities_on_empty_community_config_returns_empty() {
    let (_dir, facade) = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let list = <ApplicationFacade<_> as CommunityUseCase>::list_communities(&facade, dir.path()).unwrap();
    assert!(list.is_empty());
}

#[test]
fn create_community_writes_to_disk() {
    let (_dir, facade) = make_facade();
    let dir = tempfile::tempdir().unwrap();
    let community = Community {
        id: "01J0000000000000000000000C1".to_string(),
        name: "Test".to_string(),
        selector: Selector::default(),
        pinned_members: Vec::new(),
        excluded_members: Vec::new(),
    };
    let result = <ApplicationFacade<_> as CommunityUseCase>::create_community(
        &facade,
        dir.path(),
        community,
    );
    assert!(result.is_ok());
    let list = <ApplicationFacade<_> as CommunityUseCase>::list_communities(&facade, dir.path()).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, "01J0000000000000000000000C1");
}