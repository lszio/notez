use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

fn notez_cmd() -> Command {
    Command::cargo_bin("notez").unwrap()
}

#[test]
fn cli_community_create_derive_and_skill_export() {
    let temp = tempdir().unwrap();
    let space = temp.path();

    let doc = space.join("note.org");
    fs::write(
        &doc,
        "#+title: DevSync Note\n#+ID: 01J00000000000000000000055\n* NEXT Sync Item\n:PROPERTIES:\n:ID: 01J00000000000000000000056\n:END:\n",
    )
    .unwrap();

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("scan")
        .assert()
        .success();

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("community")
        .arg("create")
        .arg("--id")
        .arg("dev_comm")
        .arg("--name")
        .arg("DevSync")
        .arg("--title-contains")
        .arg("sync")
        .assert()
        .success();

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("community")
        .arg("list")
        .assert()
        .success()
        .stdout(predicates::str::contains("dev_comm"));

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("derive")
        .arg("--community")
        .arg("dev_comm")
        .arg("--recipe")
        .arg("summary")
        .assert()
        .success()
        .stdout(predicates::str::contains("# Community Summary"));

    let skill_dir = space.join("my_exported_skill");
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("skill")
        .arg("export")
        .arg("--community")
        .arg("dev_comm")
        .arg("--description")
        .arg("DevSync Skill Prompt")
        .arg("--out")
        .arg(&skill_dir)
        .assert()
        .success();

    assert!(skill_dir.join("SKILL.md").exists());
}
