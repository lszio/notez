use crate::source::{AnytypeSourceAdapter, SourceAdapter, SourceConfig, SourceKind};
use tempfile::tempdir;

#[test]
fn anytype_stub_reports_no_capabilities_and_fails_loudly() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    let config = SourceConfig::new("anytype_src", SourceKind::Anytype, root, false);
    let adapter = AnytypeSourceAdapter::new(config);

    assert_eq!(adapter.config().id, "anytype_src");
    assert_eq!(adapter.config().kind, SourceKind::Anytype);

    // Honesty contract: the remote transport is not wired, so no
    // capability may be claimed and every operation must fail with an
    // explicit error — never fake data, never fake success.
    let caps = adapter.capabilities();
    assert!(!caps.can_read);
    assert!(!caps.can_write);
    assert!(!caps.can_import);
    assert!(!caps.can_watch);

    let err = adapter.scan().expect_err("scan must refuse on stub transport");
    assert!(
        err.to_string().contains("not implemented"),
        "unexpected error: {err}"
    );

    let prep_err = adapter
        .prepare_write("heading:01J00000000000000000000001", "new_title")
        .expect_err("prepare_write must refuse without capabilities");
    assert!(prep_err.to_string().contains("not supported"));

    // commit_write has no prepared write; the trait default refuses too.
    let prep = crate::source::PreparedWrite {
        target_ref: "heading:01J00000000000000000000001".into(),
        payload: "payload".into(),
        ready: true,
    };
    let err = adapter
        .commit_write(&prep)
        .expect_err("commit_write must refuse on stub transport");
    assert!(err.to_string().contains("not supported"));
}
