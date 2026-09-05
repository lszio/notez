//! End-to-end cross-Space object identity test (spec §3, §10 scenario 1).
//!
//! Validates the full loop: two independent Space projections each scan
//! the same underlying file with a different `source_id`, persist the
//! scan into a separate `SqliteProjection`, and then `find_by_object`
//! on each projection returns the matching row whose `object_id` is
//! equal across sources while `ref` and `source_id` differ.

use notez_core::domain::{ProjectionStore, ResourceKind, ProjectionReader, ProjectionWrite};
use notez_core::source::{SourceConfig, SourceKind, SourceRegistry};
use notez_core::storage::SqliteProjection;
use tempfile::tempdir;

#[test]
fn cross_space_scan_then_aggregate_by_object_id() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("x.md");
    std::fs::write(&file_path, b"# Title\n\nsee [other](other.md)\n").unwrap();

    // Two independent source configs over the same physical directory.
    let cfg_a = SourceConfig {
        id: "src_a".to_string(),
        kind: SourceKind::Native,
        path: dir.path().to_path_buf(),
        read_only: true,
        url: None,
        include_paths: vec![],
        exclude_paths: vec![],
        scan: Default::default(),
    };
    let cfg_b = SourceConfig {
        id: "src_b".to_string(),
        kind: SourceKind::Native,
        path: dir.path().to_path_buf(),
        read_only: true,
        url: None,
        include_paths: vec![],
        exclude_paths: vec![],
        scan: Default::default(),
    };

    let registry = SourceRegistry::with_builtins();
    let adapter_a = registry.build(cfg_a).expect("build space_a adapter");
    let adapter_b = registry.build(cfg_b).expect("build space_b adapter");

    let scanned_a = adapter_a.scan().expect("scan space_a");
    let scanned_b = adapter_b.scan().expect("scan space_b");

    // Persist each scan into its own projection (separate on-disk DBs
    // ⇒ two independent Spaces).
    let db_a = dir.path().join("a.sqlite");
    let db_b = dir.path().join("b.sqlite");
    let mut proj_a = SqliteProjection::open(&db_a).unwrap();
    let mut proj_b = SqliteProjection::open(&db_b).unwrap();

    proj_a
        .replace_source(
            &scanned_a.source_id,
            scanned_a.resources.clone(),
            scanned_a.relations.clone(),
            scanned_a.link_occurrences.clone(),
        )
        .unwrap();
    proj_b
        .replace_source(
            &scanned_b.source_id,
            scanned_b.resources.clone(),
            scanned_b.relations.clone(),
            scanned_b.link_occurrences.clone(),
        )
        .unwrap();

    // Pick the Document resource from space A and capture its
    // object_id so we can query for it in both sources.
    let doc_a = scanned_a
        .resources
        .iter()
        .find(|r| matches!(r.kind, ResourceKind::Document))
        .cloned()
        .expect("space A should produce a Document resource");
    let object_id = doc_a.object_id;

    // Cross-space aggregation: same object_id in both projections.
    let found_a = proj_a
        .find_by_object(&object_id)
        .expect("find_by_object on space A");
    let found_b = proj_b
        .find_by_object(&object_id)
        .expect("find_by_object on space B");

    assert!(
        !found_a.is_empty(),
        "space A should find the object by object_id"
    );
    assert!(
        !found_b.is_empty(),
        "space B should find the object by object_id"
    );

    // object_id matches across sources (the cross-Space identity claim).
    assert_eq!(found_a[0].object_id, found_b[0].object_id);

    // ResourceRef differs (different source_id ⇒ different derived id,
    // spec §1.2 / §3.1 — 0.3 identity contract preserved).
    assert_ne!(found_a[0].r#ref, found_b[0].r#ref);

    // And source_id itself differs, with each row pointing at its own
    // Space.
    assert_ne!(found_a[0].source_id, found_b[0].source_id);
    assert_eq!(found_a[0].source_id, "src_a");
    assert_eq!(found_b[0].source_id, "src_b");
}

/// The same object appeared via two source configs that share a physical
/// directory. Each Space owns its own projection (separate on-disk DBs)
/// and each scan's resources land in its own DB as the **primary** of that
/// scan — that is, `primary_source_id == source_id`. References to the
/// same `object_id` from a *different* source remain stubs whose
/// `source_id` is the reference source and whose `primary_source_id`
/// points at the owning source, but those references do not duplicate
/// the body.
#[test]
fn cross_space_each_scan_marks_itself_as_primary_and_exposes_primary_source() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("y.md");
    std::fs::write(&file_path, b"# Header\n\nbody\n").unwrap();

    let cfg_a = SourceConfig {
        id: "src_a".to_string(),
        kind: SourceKind::Native,
        path: dir.path().to_path_buf(),
        read_only: true,
        url: None,
        include_paths: vec![],
        exclude_paths: vec![],
        scan: Default::default(),
    };
    let cfg_b = SourceConfig {
        id: "src_b".to_string(),
        kind: SourceKind::Native,
        path: dir.path().to_path_buf(),
        read_only: true,
        url: None,
        include_paths: vec![],
        exclude_paths: vec![],
        scan: Default::default(),
    };
    let registry = SourceRegistry::with_builtins();
    let scanned_a = registry.build(cfg_a).unwrap().scan().unwrap();
    let scanned_b = registry.build(cfg_b).unwrap().scan().unwrap();

    let mut proj_a = SqliteProjection::open(&dir.path().join("a.sqlite")).unwrap();
    let mut proj_b = SqliteProjection::open(&dir.path().join("b.sqlite")).unwrap();
    proj_a
        .replace_source(
            &scanned_a.source_id,
            scanned_a.resources.clone(),
            scanned_a.relations.clone(),
            scanned_a.link_occurrences.clone(),
        )
        .unwrap();
    proj_b
        .replace_source(
            &scanned_b.source_id,
            scanned_b.resources.clone(),
            scanned_b.relations.clone(),
            scanned_b.link_occurrences.clone(),
        )
        .unwrap();

    // Pick the matching document from both scans and verify both sides
    // are recorded as primary for their own source (empty
    // primary_source_id ⇒ falls back to source_id).
    let doc_a = scanned_a
        .resources
        .iter()
        .find(|r| matches!(r.kind, ResourceKind::Document))
        .unwrap();
    let doc_b = scanned_b
        .resources
        .iter()
        .find(|r| matches!(r.kind, ResourceKind::Document))
        .unwrap();
    let object_id = doc_a.object_id.clone();
    assert_eq!(object_id, doc_b.object_id, "scanned docs share object_id");

    let row_a = proj_a.find_by_object(&object_id).unwrap();
    let row_b = proj_b.find_by_object(&object_id).unwrap();
    assert_eq!(row_a.len(), 1);
    assert_eq!(row_b.len(), 1);
    assert_eq!(row_a[0].primary_source(), "src_a");
    assert_eq!(row_b[0].primary_source(), "src_b");

    // `replace_source` materializes the implicit "self is primary" rule at
    // write time, so the column is populated to the row's own source_id
    // even when callers leave `primary_source_id` empty (the rendering is
    // the persisted contract; the `primary_source()` accessor falls back
    // to source_id for in-memory Resources that haven't gone through
    // persistence yet).
    assert_eq!(row_a[0].primary_source_id, "src_a");
    assert_eq!(row_b[0].primary_source_id, "src_b");
}
