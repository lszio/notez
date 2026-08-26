use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

fn notez_cmd() -> Command {
    Command::cargo_bin("notez").unwrap()
}

#[test]
fn cli_rules_task_transition_and_agenda() {
    let temp = tempdir().unwrap();
    let space = temp.path();

    let doc = space.join("doc.org");
    fs::write(
        &doc,
        "#+title: Project Doc\n#+ID: 01J00000000000000000000700\n* NEXT Feature item\n:PROPERTIES:\n:ID: 01J00000000000000000000701\n:SCHEDULED: <2026-07-22 Wed>\n:TYPE: project\n:END:\n",
    ).unwrap();

    // 1. Scan
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("scan")
        .assert()
        .success();

    // 2. Agenda
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("agenda")
        .assert()
        .success()
        .stdout(predicates::str::contains("Feature item"));

    // 3. Inspect --rules
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("inspect")
        .arg("heading:01J00000000000000000000701")
        .arg("--rules")
        .assert()
        .success()
        .stdout(predicates::str::contains("\"classified_type\":\"project\""));
    // 4. Register a native source so the writeback path can resolve it.
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("source")
        .arg("add")
        .arg("--id")
        .arg("native")
        .arg("--kind")
        .arg("native")
        .arg("--path")
        .arg(space)
        .assert()
        .success();

    // 5. Task transition on a native source succeeds via the M4
    // surgical span writer: the heading line is patched in place and
    // every other byte of the file is preserved.
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("task")
        .arg("transition")
        .arg("heading:01J00000000000000000000701")
        .arg("--to")
        .arg("DONE")
        .assert()
        .success();
}
