use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

fn notez_cmd() -> Command {
    Command::cargo_bin("notez").unwrap()
}

#[test]
fn cli_space_doctor_job_list_and_artifact_stale() {
    let temp = tempdir().unwrap();
    let space = temp.path();

    let doc = space.join("note.org");
    fs::write(&doc, "#+title: Healthy\n#+ID: 01J00000000000000000000001\n").unwrap();

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("scan")
        .assert()
        .success();

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("space")
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicates::str::contains("\"status\":\"healthy\""));

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("job")
        .arg("list")
        .assert()
        .success();

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("artifact")
        .arg("stale")
        .assert()
        .success()
        .stdout(predicates::str::contains("\"status\":\"fresh\""));
}
