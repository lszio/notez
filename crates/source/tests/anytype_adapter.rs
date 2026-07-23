use source::{AnytypeSourceAdapter, SourceAdapter, SourceConfig, SourceKind};
use tempfile::tempdir;

#[test]
fn anytype_adapter_capabilities_and_writeback_contract() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    let config = SourceConfig::new("anytype_src", SourceKind::Anytype, root, false);
    let adapter = AnytypeSourceAdapter::new(config);

    assert_eq!(adapter.config().id, "anytype_src");
    assert_eq!(adapter.config().kind, SourceKind::Anytype);

    let caps = adapter.capabilities();
    assert!(caps.can_read);
    assert!(caps.can_write);
    assert!(caps.can_import);

    let scanned = adapter.scan().unwrap();
    assert_eq!(scanned.source_id, "anytype_src");

    let prep = adapter
        .prepare_write("heading:01J00000000000000000000001", "new_title")
        .unwrap();
    assert!(prep.ready);

    let commit_res = adapter.commit_write(&prep).unwrap();
    assert!(commit_res.committed);
}
