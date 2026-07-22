use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

fn notez_cmd() -> Command {
    Command::cargo_bin("notez").unwrap()
}

#[test]
fn cli_scan_query_resolve_read_rebuild() {
    let temp = tempdir().unwrap();
    let space = temp.path();

    let doc1 = space.join("doc1.org");
    fs::write(
        &doc1,
        "#+title: Architecture\n#+ID: 01J00000000000000000000100\n* NEXT Design sync\n:PROPERTIES:\n:ID: 01J00000000000000000000101\n:END:\n",
    ).unwrap();

    let doc2 = space.join("doc2.org");
    fs::write(
        &doc2,
        "#+title: Design sync notes\n#+ID: 01J00000000000000000000102\n",
    )
    .unwrap();

    let mut cmd = notez_cmd();
    cmd.arg("--space")
        .arg(space)
        .arg("--json")
        .arg("scan")
        .assert()
        .success()
        .stdout(predicates::str::contains("\"scanned_files\":2"));

    let mut cmd_q = notez_cmd();
    cmd_q
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("query")
        .arg("--kind")
        .arg("heading")
        .arg("--title-contains")
        .arg("sync")
        .assert()
        .success()
        .stdout(predicates::str::contains("Design sync"));

    let mut cmd_amb = notez_cmd();
    cmd_amb
        .arg("--space")
        .arg(space)
        .arg("resolve")
        .arg("Design sync")
        .assert()
        .code(4);

    let mut cmd_nf = notez_cmd();
    cmd_nf
        .arg("--space")
        .arg(space)
        .arg("resolve")
        .arg("nonexistent_title_query")
        .assert()
        .code(3);

    let mut cmd_res = notez_cmd();
    cmd_res
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("resolve")
        .arg("01J00000000000000000000101")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "heading:01J00000000000000000000101",
        ));

    let mut cmd_read = notez_cmd();
    cmd_read
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("read")
        .arg("heading:01J00000000000000000000101")
        .assert()
        .success()
        .stdout(predicates::str::contains("\"title\":\"Design sync\""));

    let mut cmd_reb = notez_cmd();
    cmd_reb
        .arg("--space")
        .arg(space)
        .arg("space")
        .arg("rebuild")
        .assert()
        .success();
}
