//! Cross-Space object identity contract test (spec §3.1 + §6.2).
//!
//! Two Space projections, each scanning the same file with a different
//! `source_id`, must produce the same `object_id` for the matching
//! Document, but a different `ResourceRef` and `source_id`. This is
//! the minimal end-to-end proof for the cross-Space aggregation
//! scenario in spec §10.

use notez_core::document::content_hash_of_file;
use notez_core::domain::{Resource, ResourceKind, derived_object_id};
use notez_core::source::{ScannedSource, SourceConfig, SourceKind, SourceRegistry};
use tempfile::tempdir;

#[test]
fn two_spaces_scanning_same_file_produce_same_object_id() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("shared.md");
    std::fs::write(&file_path, b"# heading\n\nbody\n").unwrap();

    // Source-relative locator. The native transport strips the
    // include_root prefix from the absolute path, so the locator the
    // scanner sees for a top-level file is just its filename.
    let locator = "shared.md";

    let cfg_a = SourceConfig {
        id: "src_a".to_string(), kind: SourceKind::Native, path: dir.path().to_path_buf(), read_only: true, url: None, include_paths: vec![], exclude_paths: vec![],
        scan: Default::default(),
    };
    let cfg_b = SourceConfig {
        id: "src_b".to_string(), kind: SourceKind::Native, path: dir.path().to_path_buf(), read_only: true, url: None, include_paths: vec![], exclude_paths: vec![],
        scan: Default::default(),
    };

    // Compute the expected object_id with the public helper.
    let hash = content_hash_of_file(&file_path).unwrap();
    let expected_doc = derived_object_id(&hash, locator, "");

    // Two independent registries → two independent adapters. Same
    // physical directory, different source_id.
    let registry = SourceRegistry::with_builtins();
    let adapter_a = registry.build(cfg_a).expect("build adapter_a");
    let adapter_b = registry.build(cfg_b).expect("build adapter_b");

    let scanned_a = adapter_a.scan().expect("scan space A");
    let scanned_b = adapter_b.scan().expect("scan space B");

    assert!(
        !scanned_a.resources.is_empty(),
        "space A should produce at least one resource"
    );
    assert!(
        !scanned_b.resources.is_empty(),
        "space B should produce at least one resource"
    );

    let find_doc = |scanned: &ScannedSource| -> Option<Resource> {
        scanned
            .resources
            .iter()
            .find(|r| matches!(r.kind, ResourceKind::Document))
            .cloned()
    };
    let doc_a = find_doc(&scanned_a).expect("space A document");
    let doc_b = find_doc(&scanned_b).expect("space B document");

    // object_id matches across sources (the headline assertion).
    assert_eq!(doc_a.object_id, doc_b.object_id);
    assert_eq!(doc_a.object_id, expected_doc);

    // But ResourceRef is different (different source_id ⇒ different
    // derived_id, spec §1.2 — 0.3 identity contract preserved).
    assert_ne!(doc_a.r#ref, doc_b.r#ref);

    // And source_id itself differs.
    assert_ne!(doc_a.source_id, doc_b.source_id);
    assert_eq!(doc_a.source_id, "src_a");
    assert_eq!(doc_b.source_id, "src_b");
}
