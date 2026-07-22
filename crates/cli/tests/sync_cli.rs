use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

fn notez_cmd() -> Command {
    Command::cargo_bin("notez").unwrap()
}

#[test]
fn cli_sync_push_pull_and_conflicts() {
    let temp = tempdir().unwrap();
    let space_a = temp.path().join("space_a");
    let space_b = temp.path().join("space_b");
    let shared = temp.path().join("shared");

    fs::create_dir_all(&space_a).unwrap();
    fs::create_dir_all(&space_b).unwrap();
    fs::create_dir_all(&shared).unwrap();

    let doc = space_a.join("note.org");
    fs::write(
        &doc,
        "#+title: Shared Note\n#+ID: 01J00000000000000000000044\n",
    )
    .unwrap();

    notez_cmd()
        .arg("--space")
        .arg(&space_a)
        .arg("--json")
        .arg("sync")
        .arg("push")
        .arg("--actor")
        .arg("actor_a")
        .arg("--folder")
        .arg(&shared)
        .assert()
        .success()
        .stdout(predicates::str::contains("\"pushed_files\":1"));

    notez_cmd()
        .arg("--space")
        .arg(&space_b)
        .arg("--json")
        .arg("sync")
        .arg("pull")
        .arg("--actor")
        .arg("actor_b")
        .arg("--folder")
        .arg(&shared)
        .assert()
        .success()
        .stdout(predicates::str::contains("\"pulled_files\":1"));

    notez_cmd()
        .arg("--space")
        .arg(&space_b)
        .arg("--json")
        .arg("sync")
        .arg("conflicts")
        .assert()
        .success();
}
