use source::{NativeSourceAdapter, SourceAdapter, SourceConfig, SourceKind};
use std::fs;
use std::path::PathBuf;

fn build_space() -> PathBuf {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    fs::create_dir_all(root.join("notes")).unwrap();
    fs::write(
        root.join("notes/keep.org"),
        "#+title: Keep\n#+ID: 01J000000000000000000000A1\n\nkeep\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("scratch")).unwrap();
    fs::write(
        root.join("scratch/drop.org"),
        "#+title: Drop\n#+ID: 01J000000000000000000000A2\n\ndrop\n",
    )
    .unwrap();
    // `extra.md` lives at the source root, *outside* any include path, to
    // prove that include_paths suppresses it. It uses a non-org extension to
    // also confirm the file-extension filter still applies.
    fs::write(
        root.join("extra.md"),
        "---\nid: 01J000000000000000000000A3\n---\n# Extra\n",
    )
    .unwrap();
    std::mem::forget(dir);
    root
}

#[test]
fn include_paths_restrict_scan_to_listed_subtrees() {
    let root = build_space();
    let config = SourceConfig {
        id: "with_include".into(),
        kind: SourceKind::Native,
        path: root.clone(),
        read_only: true,
        include_paths: vec![root.join("notes")],
        exclude_paths: vec![],
    };
    let scanned = NativeSourceAdapter::new(config).scan().unwrap();

    let titles: Vec<&str> = scanned.resources.iter().map(|r| r.title.as_str()).collect();
    assert!(titles.contains(&"Keep"), "include must allow listed subtree, got {titles:?}");
    assert!(!titles.contains(&"Drop"), "include must skip unlisted subtree");
    assert!(
        !titles.contains(&"Extra"),
        "include must drop files outside the include list"
    );
}

#[test]
fn exclude_paths_drop_listed_subtrees() {
    let root = build_space();
    let config = SourceConfig {
        id: "with_exclude".into(),
        kind: SourceKind::Native,
        path: root.clone(),
        read_only: true,
        include_paths: vec![],
        exclude_paths: vec![root.join("scratch"), root.join("extra.md")],
    };
    let scanned = NativeSourceAdapter::new(config).scan().unwrap();
    let titles: Vec<&str> = scanned.resources.iter().map(|r| r.title.as_str()).collect();
    assert!(titles.contains(&"Keep"), "keep file should still be scanned");
    assert!(!titles.contains(&"Drop"), "scratch must be excluded");
    assert!(!titles.contains(&"Extra"), "extra.md must be excluded");

}
