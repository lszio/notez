use storage::blob::BlobStore;
use tempfile::tempdir;

#[test]
fn test_content_addressed_blob_store() {
    let temp = tempdir().unwrap();
    let root = temp.path();

    let store = BlobStore::new(root);

    let content = b"Hello, Notez Attachment Blob!";
    let meta = store.store_bytes(content, "text/plain").unwrap();

    assert_eq!(meta.size_bytes, content.len() as u64);
    assert!(!meta.hash.is_empty());
    assert_eq!(meta.mime_type, "text/plain");

    assert!(store.has(&meta.hash));

    let fetched = store.get(&meta.hash).unwrap().expect("blob should exist");
    assert_eq!(fetched, content);

    let meta2 = store.store_bytes(content, "text/plain").unwrap();
    assert_eq!(meta.hash, meta2.hash);
}
