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
        .assert()
        .code(3);
}
