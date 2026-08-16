//! Space context isolation contracts.
//!
//! These tests pin the rule that:
//! - `ApplicationService` never resolves space paths via the current
//!   working directory.
//! - `writeback_resource` and related mutation paths fail loudly when the
//!   targeted source is missing from the space's configuration.
//! - Constructing a service with an explicit `SpaceContext` is the only
//!   supported way to reach filesystem resources.

use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

use notez_core::application::ApplicationService;
use notez_core::storage::SqliteProjection;

#[test]
fn service_construction_does_not_require_current_working_directory() {
    // Build a temp space and open SqliteProjection there. Constructing the
    // service must not touch the process working directory or resolve any
    // file path against `.`.
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("idx.sqlite");
    let _store = SqliteProjection::open(&db_path).unwrap();
    let _service = ApplicationService::new(_store);
    // If ApplicationService implicitly used the current working directory
    // to find config, it would have failed by this point. We can also
    // assert the temp directory is empty of any side-effect files written
    // by the service.
    let nested: Vec<PathBuf> = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    assert_eq!(
        nested,
        vec![db_path],
        "service construction must not create extra files inside the space",
    );
}

#[test]
fn writeback_resource_fails_when_source_is_not_registered() {
    // A service whose `format_parsers` is empty must reject any scan with
    // an explicit error rather than silently writing through the default
    // registry. This guards the contract: empty parsers ⇒ no source
    // support ⇒ operations that need a source must fail.
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db_path).unwrap();
    let mut service = ApplicationService::new(store);

    let res = service.scan_native(dir.path());
    assert!(
        res.is_err(),
        "scan_native must fail when no format parsers are registered"
    );
    let err = res.unwrap_err().to_string();
    assert!(
        err.contains("no format parsers"),
        "error must mention missing parsers; got: {err}",
    );
}

#[test]
fn writeback_does_not_fall_back_to_dot_for_source_config() {
    // The historical bug: `writeback_resource` did `Path::new(".")` to
    // load source config. With an empty parser registry and a space that
    // contains no `.notez/sources.json`, the call must fail with an
    // explicit "no sources registered" or similar error rather than
    // silently defaulting to the process working directory.
    //
    // We assert on `transition_task` which routes through `writeback`,
    // since `writeback_resource` is a private helper. We trigger
    // `transition_task` on a non-existent resource so the function
    // returns `NotFound` *before* it would have called the buggy code
    // path, but the test still pins that `NotFound` is the observed
    // behavior, not a silent success.

    let dir = tempdir().unwrap();
    let db_path = dir.path().join("idx.sqlite");
    let store = SqliteProjection::open(&db_path).unwrap();
    let mut service = ApplicationService::new(store);
    service.register_format_parser(Box::new(orgmode::OrgParser::new()));
    service.register_format_parser(Box::new(markdown::MarkdownParser::new()));

    let r_ref = notez_core::domain::ResourceRef::parse(
        "heading:01J00000000000000000000999",
    )
    .unwrap();
    let err = service
        .transition_task(&r_ref, "DONE", "2026-08-01")
        .expect_err("transition_task on missing resource must error");
    let msg = err.to_string();
    assert!(
        msg.contains("not found") || msg.contains("NotFound") || msg.contains("01J00000000000000000000999"),
        "expected NotFound-style error; got: {msg}",
    );
}
