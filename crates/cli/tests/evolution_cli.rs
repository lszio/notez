//! Contract tests for `notez source` commands.
//!
//! As of the format-adapter refactor:
//! - `source add` writes through the canonical `SourceContext.runtime`.
//! - `source writeback` requires an explicit `SourceContext` and a
//!   registered, non-read-only source. The legacy behaviour that
//!   silently reported `committed: true` for unknown sources is gone.
//! - `sync relay` reports the relay capability as `Unsupported` until
//!   the relay subsystem is implemented.

use assert_cmd::Command;
use tempfile::tempdir;

fn notez_cmd() -> Command {
    Command::cargo_bin("notez").unwrap()
}

#[test]
fn cli_source_writeback_fails_when_source_missing() {
    let temp = tempdir().unwrap();
    let space = temp.path();

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
        .failure()
        .stderr(predicates::str::contains("registered"));
}

#[test]
fn cli_sync_relay_reports_unsupported() {
    let temp = tempdir().unwrap();
    let space = temp.path();

    notez_cmd()
        .arg("--space")
        .arg(space)
        .arg("--json")
        .arg("sync")
        .arg("relay")
        .arg("--id")
        .arg("anytype_src")
        .assert()
        .failure()
        .stderr(predicates::str::contains("unsupported capability"));
}
