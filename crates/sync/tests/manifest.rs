use sync::manifest::Manifest;
use sync::object::{ObjectStore, SyncObject, TombstoneRecord};
use tempfile::tempdir;

#[test]
fn manifest_and_object_store_operations() {
    let temp = tempdir().unwrap();
    let store = ObjectStore::new(temp.path());

    let manifest = Manifest {
        space_id: "space_123".to_string(),
        actor_id: "actor_device_a".to_string(),
        parent_snapshots: vec!["snap_0".to_string()],
        logical_path: "note.org".to_string(),
        content_hash: "hash_abc123".to_string(),
        properties: Default::default(),
    };

    store.write_manifest(&manifest).unwrap();

    let loaded = store
        .read_manifest("note.org")
        .unwrap()
        .expect("manifest should exist");
    assert_eq!(loaded.space_id, "space_123");
    assert_eq!(loaded.content_hash, "hash_abc123");

    let obj = SyncObject {
        hash: "hash_abc123".to_string(),
        payload: b"Hello Sync Object Payload".to_vec(),
    };

    store.write_object(&obj).unwrap();
    assert!(store.has_object("hash_abc123"));

    let fetched = store.read_object("hash_abc123").unwrap().unwrap();
    assert_eq!(fetched.payload, b"Hello Sync Object Payload");

    let tomb = TombstoneRecord {
        logical_path: "deleted.org".to_string(),
        deleted_at: "2026-07-22 Wed 18:00".to_string(),
    };
    store.write_tombstone(&tomb).unwrap();
    assert!(store.has_tombstone("deleted.org"));
}
