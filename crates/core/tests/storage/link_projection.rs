use crate::domain::{
    LinkOccurrence, LinkTarget, ProjectionStore, ResolutionStatus, Resource,
    ResourceKind, ResourceRef, ResourceRelation, TextSpan,
};
use std::collections::BTreeMap;
use crate::storage::SqliteProjection;

fn sample_resource(
    kind: ResourceKind,
    title: &str,
    ulid_str: &str,
    source_id: &str,
    locator: &str,
) -> Resource {
    let r_ref = ResourceRef::new(kind, ulid_str.parse().unwrap());
    Resource {
        r#ref: r_ref,
        kind,
        title: title.to_string(),
        revision: "rev-1".to_string(),
        source_id: source_id.to_string(),
        locator: locator.to_string(),
        properties: BTreeMap::new(),
        object_id: notez_core::domain::ObjectId::default(),
    }
}

fn occurrence(source_ref: ResourceRef, target: LinkTarget, raw: &str, line: usize) -> LinkOccurrence {
    LinkOccurrence {
        source_ref,
        target,
        raw: raw.to_string(),
        display_text: None,
        span: TextSpan {
            line,
            col_start: 0,
            col_end: raw.len(),
        },
    }
}

#[test]
fn link_occurrences_persist_status_and_candidates() {
    let mut store = SqliteProjection::in_memory().unwrap();

    let doc_ref = ResourceRef::new(ResourceKind::Document, "01J00000000000000000000010".parse().unwrap());
    let res = sample_resource(
        ResourceKind::Document,
        "Source",
        "01J00000000000000000000010",
        "native",
        "notes/source.org",
    );
    let cand_a = ResourceRef::new(ResourceKind::Document, "01J00000000000000000000011".parse().unwrap());
    let cand_b = ResourceRef::new(ResourceKind::Document, "01J00000000000000000000012".parse().unwrap());
    let ambig_candidates = vec![cand_a, cand_b];

    let diagnostics = vec![
        (occurrence(doc_ref, LinkTarget::title("Design", None), "[[Design]]", 1),
         ResolutionStatus::Ambiguous,
         ambig_candidates.clone()),
        (occurrence(doc_ref, LinkTarget::url("https://example.com"), "https://example.com", 2),
         ResolutionStatus::External,
         vec![]),
        (occurrence(doc_ref, LinkTarget::title("Missing", None), "[[Missing]]", 3),
         ResolutionStatus::Unresolved,
         vec![]),
    ];

    store
        .replace_source(
            "native",
            vec![res.clone()],
            vec![ResourceRelation {
                source_ref: doc_ref,
                relation: "link".into(),
                target_ref: cand_a,
            }],
            diagnostics
                .iter()
                .map(|(o, _, _)| o.clone())
                .collect(),
        )
        .unwrap();

    store
        .write_link_diagnostics("native", &diagnostics)
        .unwrap();

    // Persistence round-trip via diagnostics query.
    let rows = store
        .list_link_diagnostics(&doc_ref)
        .unwrap()
        .expect("source diagnostics present");
    assert_eq!(rows.len(), 3);
    let by_raw: std::collections::HashMap<_, _> = rows
        .iter()
        .map(|d| (d.occurrence.raw.clone(), (d.status.clone(), d.candidates.clone())))
        .collect();
    assert_eq!(by_raw["[[Design]]"], (ResolutionStatus::Ambiguous, ambig_candidates.clone()));
    assert_eq!(by_raw["https://example.com"], (ResolutionStatus::External, vec![]));
    assert_eq!(by_raw["[[Missing]]"], (ResolutionStatus::Unresolved, vec![]));

    // Reopen file-backed DB and assert diagnostics survive.
    let tmp = tempdir_path();
    let mut disk = SqliteProjection::open(&tmp).unwrap();
    disk.replace_source("native", vec![res], vec![], diagnostics.iter().map(|(o,_,_)|o.clone()).collect()).unwrap();
    disk.write_link_diagnostics("native", &diagnostics).unwrap();
    let rows2 = disk.list_link_diagnostics(&doc_ref).unwrap().expect("disk diagnostics");
    assert_eq!(rows2.len(), 3);
    let _ = std::fs::remove_dir_all(&tmp);
}

fn tempdir_path() -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("notez-link-projection-{}", std::process::id()));
    p.push("index.sqlite");
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    p
}
