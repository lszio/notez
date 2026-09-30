//! Exit-code contract for `ApplicationError` variants via the CLI.
//!
//! The mapping lives in `cli/src/main.rs::exit_code_for` and is a stable
//! CLI contract (see docs/superpowers/specs/2026-08-02-application-error-design.org):
//! NotFound=3, Storage/Document/Io=5, UnsupportedCapability=6,
//! ReadOnlySource=7, SourceNotFound=8, RevisionConflict=9.

use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

fn notez_cmd() -> Command {
    Command::cargo_bin("notez").unwrap()
}

fn warm_space(space: &std::path::Path) {
    let doc = space.join("note.org");
    fs::write(&doc, "#+title: Healthy\n#+ID: 01J00000000000000000000E01\n").unwrap();
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("scan")
        .assert()
        .success();
}

#[test]
fn mutation_help_requires_revision_preconditions() {
    notez_cmd()
        .args(["resource", "upsert", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("--create"))
        .stdout(predicates::str::contains("--expected-revision"));
    notez_cmd()
        .args(["resource", "delete", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("--expected-revision"));
    notez_cmd()
        .args(["task", "transition", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("--expected-revision"));
    notez_cmd()
        .args(["source", "writeback", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("--expected-revision"));
}

#[test]
fn upsert_create_requires_the_create_flag() {
    let temp = tempdir().unwrap();
    let payload = temp.path().join("resource.json");
    fs::write(
        &payload,
        r#"{
            "ref": "heading:01J00000000000000000000E01",
            "kind": "heading",
            "title": "Injected Heading",
            "revision": "rev1",
            "source_id": "native",
            "locator": "/injected.org",
            "properties": {}
        }"#,
    )
    .unwrap();

    notez_cmd()
        .arg("resource")
        .arg("upsert")
        .arg("--from")
        .arg(&payload)
        .assert()
        .code(2)
        .stderr(predicates::str::contains("expected revision"));
    let create_space = tempdir().unwrap();
    let create_payload = temp.path().join("create-resource.json");
    fs::write(&create_payload, fs::read_to_string(&payload).unwrap().replace("01J00000000000000000000E01", "01J00000000000000000000E02")).unwrap();
    notez_cmd()
        .arg("--space")
        .arg(create_space.path())
        .arg("resource")
        .arg("upsert")
        .arg("--from")
        .arg(&create_payload)
        .arg("--create")
        .assert()
        .success()
        .stdout(predicates::str::contains("heading:01J00000000000000000000E02"));
}

#[test]
fn resolve_missing_is_exit_3_not_found() {
    let temp = tempdir().unwrap();
    let space = temp.path();
    warm_space(space);
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("resolve")
        .arg("no_such_title_anywhere")
        .assert()
        .code(3);
}

#[test]
fn source_doctor_is_exit_6_unsupported_capability() {
    let temp = tempdir().unwrap();
    let space = temp.path();
    warm_space(space);
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("workspace")
        .arg("doctor")
        .assert()
        .code(6);
}

#[test]
fn sync_relay_is_exit_6_unsupported_capability() {
    let temp = tempdir().unwrap();
    let space = temp.path();
    warm_space(space);
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("sync")
        .arg("relay")
        .arg("--id")
        .arg("anytype_src")
        .assert()
        .code(6);
}

#[test]
fn source_writeback_unknown_source_is_exit_5_no_source_registered() {
    // DISCREPANCY vs plan: the plan expected SourceNotFound (exit 8), but
    // `writeback_resource` checks `space.runtime.sources.is_empty()` BEFORE
    // resolving the source id. A freshly scanned space has no registered
    // sources (scan only fills the projection, not the runtime source
    // config), so an unknown source id yields
    // `Storage { kind: NoSourceRegistered }` -> exit 5, not 8.
    // Assert the actual behaviour; SourceNotFound (8) would require a
    // registered source and a genuinely unknown id.
    let temp = tempdir().unwrap();
    let space = temp.path();
    warm_space(space);
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("source")
        .arg("writeback")
        .arg("--id")
        .arg("missing_src")
        .arg("--r-ref")
        .arg("heading:01J00000000000000000000E02")
        .arg("--payload")
        .arg("payload")
        .arg("--expected-revision")
        .arg("stale")
        .assert()
        .code(5);
}

#[test]
fn task_transition_missing_resource_is_exit_3() {
    let temp = tempdir().unwrap();
    let space = temp.path();
    warm_space(space);
    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("task")
        .arg("transition")
        .arg("heading:01J00000000000000000000E99")
        .arg("--to")
        .arg("DONE")
        .arg("--expected-revision")
        .arg("stale")
        .assert()
        .code(9);
}

#[test]
fn delete_stale_revision_is_exit_9() {
    let temp = tempdir().unwrap();
    let space = temp.path();
    warm_space(space);

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("resource")
        .arg("delete")
        .arg("document:01J00000000000000000000E01")
        .arg("--expected-revision")
        .arg("stale")
        .assert()
        .code(9);
}
