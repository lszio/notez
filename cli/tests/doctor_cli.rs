//! Contract tests for `notez space doctor`, `notez task jobs`, and
//! `notez artifact stale`.
//!
//! As of the format-adapter refactor, these capabilities are explicitly
//! marked as unsupported. The CLI must surface the failure with a
//! non-zero exit code and a structured message, not fake success.

use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

fn notez_cmd() -> Command {
    Command::cargo_bin("notez").unwrap()
}

fn warm_space(space: &std::path::Path) {
    let doc = space.join("note.org");
    fs::write(
        &doc,
        "#+title: Healthy\n#+ID: 01J00000000000000000000001\n",
    )
    .unwrap();
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("scan")
        .assert()
        .success();
}

#[test]
fn cli_space_doctor_reports_unsupported() {
    let temp = tempdir().unwrap();
    let space = temp.path();
    warm_space(space);

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("space")
        .arg("doctor")
        .assert()
        .failure()
        .stderr(predicates::str::contains("unsupported capability"));
}

#[test]
fn cli_task_jobs_reports_unsupported() {
    let temp = tempdir().unwrap();
    let space = temp.path();
    warm_space(space);

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("task")
        .arg("jobs")
        .assert()
        .failure()
        .stderr(predicates::str::contains("unsupported capability"));
}

#[test]
fn cli_artifact_stale_reports_unsupported() {
    let temp = tempdir().unwrap();
    let space = temp.path();
    warm_space(space);

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("artifact")
        .arg("stale")
        .assert()
        .failure()
        .stderr(predicates::str::contains("unsupported capability"));
}
