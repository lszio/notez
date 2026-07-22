use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

fn notez_cmd() -> Command {
    Command::cargo_bin("notez").unwrap()
}

#[test]
fn cli_source_add_list_sync() {
    let temp = tempdir().unwrap();
    let space = temp.path();

    let vault_dir = space.join("vault");
    fs::create_dir_all(&vault_dir).unwrap();
    fs::write(
        vault_dir.join("note.md"),
        "---\ntitle: Obsidian Vault Note\nid: 01J00000000000000000000950\n---\n",
    )
    .unwrap();

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("source")
        .arg("add")
        .arg("--id")
        .arg("vault_src")
        .arg("--kind")
        .arg("obsidian")
        .arg("--path")
        .arg(&vault_dir)
        .arg("--read-only")
        .assert()
        .success();

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("source")
        .arg("list")
        .assert()
        .success()
        .stdout(predicates::str::contains("vault_src"));

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("source")
        .arg("sync")
        .assert()
        .success()
        .stdout(predicates::str::contains("\"scanned_resources\":"));
}
