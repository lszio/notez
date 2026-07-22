use application::ApplicationService;
use domain::community::Community;
use domain::{ResourceKind, Selector};
use std::fs;
use storage::SqliteProjection;

#[test]
fn community_management_artifact_derivation_and_skill_export() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_root = temp_dir.path();
    let dot_notez = space_root.join(".notez");
    fs::create_dir_all(&dot_notez).unwrap();
    let db_path = dot_notez.join("index.sqlite");

    let file = space_root.join("community_doc.org");
    let content = r#"#+title: DevSync Notes
#+ID: 01J00000000000000000000090

* NEXT Core architecture sync
:PROPERTIES:
:ID: 01J00000000000000000000091
:TYPE: project
:END:
"#;
    fs::write(&file, content).unwrap();

    let store = SqliteProjection::open(&db_path).unwrap();
    let mut service = ApplicationService::new(store);
    service.scan_native(space_root).unwrap();

    let mut selector = Selector::kind(ResourceKind::Heading);
    selector.title_contains = Some("sync".to_string());

    let community = Community {
        id: "comm_devsync".to_string(),
        name: "DevSync".to_string(),
        selector,
        pinned_members: vec![],
        excluded_members: vec![],
    };

    service.create_community(space_root, community).unwrap();

    let communities = service.list_communities(space_root).unwrap();
    assert_eq!(communities.len(), 1);
    assert_eq!(communities[0].name, "DevSync");

    let summary_artifact = service
        .derive_artifact(space_root, "comm_devsync", "summary")
        .unwrap();
    assert!(summary_artifact.content.contains("# Community Summary"));
    assert!(summary_artifact.content.contains("Core architecture sync"));

    let export_dir = space_root.join("exported_skill");
    let package = service
        .export_skill(space_root, "comm_devsync", "DevSync Skill", &export_dir)
        .unwrap();

    assert!(package.package_path.join("SKILL.md").exists());
    let skill_content = fs::read_to_string(package.package_path.join("SKILL.md")).unwrap();
    assert!(skill_content.contains("DevSync Skill"));
}
