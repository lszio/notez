use assert_cmd::Command;
use tempfile::tempdir;

fn notez_cmd() -> Command {
    Command::cargo_bin("notez").unwrap()
}

#[test]
fn cli_source_writeback_and_relay_sync() {
    let temp = tempdir().unwrap();
    let space = temp.path();

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("source")
        .arg("add")
        .arg("--id")
        .arg("anytype_src")
        .arg("--kind")
        .arg("anytype")
        .arg("--path")
        .arg("/anytype/path")
        .arg("--read-only")
        .assert()
        .success();

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("source")
        .arg("writeback")
        .arg("--id")
        .arg("anytype_src")
        .arg("--r-ref")
        .arg("heading:01J00000000000000000000033")
        .arg("--payload")
        .arg("Updated Title")
        .assert()
        .success()
        .stdout(predicates::str::contains("\"committed\":true"));

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("sync")
        .arg("relay")
        .arg("--id")
        .arg("anytype_src")
        .assert()
        .success()
        .stdout(predicates::str::contains("\"synced_via_relay\":true"));
}
