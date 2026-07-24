use std::path::PathBuf;

fn notez() -> assert_cmd::Command {
    assert_cmd::Command::cargo_bin("notez").unwrap()
}

#[test]
fn config_migrate_cli_previews_and_applies() {
    let tmp = tempfile::tempdir().unwrap();
    let space = tmp.path().join("legacy");
    std::fs::create_dir_all(space.join(".notez")).unwrap();
    
    // Legacy sources
    std::fs::write(
        space.join(".notez/sources.json"),
        r#"{
            "sources": [
                {
                    "id": "old_src",
                    "kind": "native",
                    "path": "/some/path",
                    "read_only": true,
                    "exclude_paths": []
                }
            ]
        }"#,
    ).unwrap();
    
    // Legacy communities
    std::fs::write(
        space.join(".notez/communities.json"),
        r#"{
            "communities": [
                {
                    "id": "old_comm",
                    "name": "Old Comm",
                    "selector": { "kind": "document", "exact_refs": [], "title_contains": null },
                    "pinned_members": [],
                    "excluded_members": []
                }
            ]
        }"#,
    ).unwrap();
    
    // Preview mode
    let out_preview = notez()
        .current_dir(&space)
        .arg("--space")
        .arg(&space)
        .arg("config")
        .arg("migrate")
        .output()
        .unwrap();
    assert!(out_preview.status.success(), "stderr={}", String::from_utf8_lossy(&out_preview.stderr));
    let stdout = String::from_utf8_lossy(&out_preview.stdout);
    assert!(stdout.contains("Would merge 1 source"));
    assert!(stdout.contains("Would extract 1 community"));
    assert!(!space.join("notez.toml").exists(), "preview must not write");
    
    // Apply mode
    let out_apply = notez()
        .current_dir(&space)
        .arg("--space")
        .arg(&space)
        .arg("config")
        .arg("migrate")
        .arg("--apply")
        .output()
        .unwrap();
    assert!(out_apply.status.success(), "apply failed: stderr={}", String::from_utf8_lossy(&out_apply.stderr));
    
    let toml_text = std::fs::read_to_string(space.join("notez.toml")).unwrap();
    assert!(toml_text.contains("old_src"));
    assert!(toml_text.contains("read_only = true"));
    
    let comm_text = std::fs::read_to_string(space.join(".notez/communities.json")).unwrap();
    assert!(comm_text.contains("old_comm"));
    
    // Old files must remain
    assert!(space.join(".notez/sources.json").exists());
}
