use application::ApplicationService;
use domain::Selector;
use source::{SourceConfig, SourceKind};
use std::fs;
use storage::SqliteProjection;

#[test]
fn multi_source_federation_scanning() {
    let temp_dir = tempfile::tempdir().unwrap();
    let space_root = temp_dir.path();
    let dot_notez = space_root.join(".notez");
    fs::create_dir_all(&dot_notez).unwrap();
    let db_path = dot_notez.join("index.sqlite");

    let native_file = space_root.join("native.org");
    fs::write(
        &native_file,
        "#+title: Native Note\n#+ID: 01J00000000000000000000210\n",
    )
    .unwrap();

    let vault_dir = space_root.join("external_vault");
    fs::create_dir_all(&vault_dir).unwrap();
    let vault_file = vault_dir.join("vault_note.md");
    fs::write(
        &vault_file,
        "---\ntitle: Vault Note\nid: 01J00000000000000000000211\n---\n",
    )
    .unwrap();

    let store = SqliteProjection::open(&db_path).unwrap();
    let mut service = ApplicationService::new(store);

    let vault_config = SourceConfig {
        id: "external_obsidian".to_string(),
        kind: SourceKind::Obsidian,
        path: vault_dir,
        read_only: true,
        exclude_paths: vec![],
    };
    service.add_source(space_root, vault_config).unwrap();

    let sources = service.list_sources(space_root).unwrap();
    assert_eq!(sources.len(), 1);

    let report = service.scan_federation(space_root).unwrap();
    assert!(report.scanned_resources >= 2);

    let page = service.query(&Selector::new()).unwrap();
    let titles: Vec<&str> = page.items.iter().map(|r| r.title.as_str()).collect();
    assert!(titles.contains(&"Native Note"));
    assert!(titles.contains(&"Vault Note"));
}
