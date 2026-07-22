use source::{
    GitSourceAdapter, NativeSourceAdapter, ObsidianSourceAdapter, SourceAdapter, SourceConfig,
    SourceKind,
};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_native_git_obsidian_adapters() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    let native_dir = root.join("native");
    fs::create_dir_all(&native_dir).unwrap();
    fs::write(
        native_dir.join("a.org"),
        "#+title: Org Doc\n#+ID: 01J00000000000000000000111\n",
    )
    .unwrap();
    fs::write(
        native_dir.join("b.md"),
        "---\ntitle: MD Doc\nid: 01J00000000000000000000112\n---\n",
    )
    .unwrap();

    let native_adapter = NativeSourceAdapter::new(SourceConfig {
        id: "native_src".to_string(),
        kind: SourceKind::Native,
        path: native_dir,
        read_only: false,
    });
    let native_scanned = native_adapter.scan().unwrap();
    assert_eq!(native_scanned.resources.len(), 2);

    let obsidian_dir = root.join("obsidian_vault");
    fs::create_dir_all(obsidian_dir.join(".obsidian")).unwrap();
    fs::write(obsidian_dir.join(".obsidian/config.json"), "{}").unwrap();
    fs::write(
        obsidian_dir.join("note.md"),
        "---\ntitle: Obsidian Note\nid: 01J00000000000000000000113\n---\n",
    )
    .unwrap();

    let obsidian_adapter = ObsidianSourceAdapter::new(SourceConfig {
        id: "vault_src".to_string(),
        kind: SourceKind::Obsidian,
        path: obsidian_dir,
        read_only: true,
    });
    let obsidian_scanned = obsidian_adapter.scan().unwrap();
    assert_eq!(obsidian_scanned.resources.len(), 1);
    assert_eq!(obsidian_scanned.resources[0].title, "Obsidian Note");

    let git_dir = root.join("git_repo");
    fs::create_dir_all(git_dir.join(".git")).unwrap();
    fs::write(
        git_dir.join("git_doc.org"),
        "#+title: Git Doc\n#+ID: 01J00000000000000000000114\n",
    )
    .unwrap();

    let git_adapter = GitSourceAdapter::new(SourceConfig {
        id: "git_src".to_string(),
        kind: SourceKind::Git,
        path: git_dir,
        read_only: true,
    });
    let git_scanned = git_adapter.scan().unwrap();
    assert_eq!(git_scanned.resources.len(), 1);
    assert_eq!(git_scanned.resources[0].title, "Git Doc");
}
