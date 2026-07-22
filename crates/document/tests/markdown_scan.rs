use document::MarkdownScanner;
use domain::{ResourceKind, ResourceRef};
use std::fs;

#[test]
fn markdown_scanner_parses_frontmatter_and_comments() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file = temp_dir.path().join("note.md");
    let content = r#"---
title: Markdown Architecture
id: 01J00000000000000000000900
type: project
---

# Introduction <!-- id: 01J00000000000000000000901 -->

See [[Obsidian Vault]] and [[id:01J00000000000000000000902][Target Link]].
"#;
    fs::write(&file, content).unwrap();

    let scanned = MarkdownScanner::scan(&file, "native").unwrap();

    assert_eq!(scanned.resources.len(), 2);

    let doc = &scanned.resources[0];
    assert_eq!(
        doc.r#ref,
        ResourceRef::parse("document:01J00000000000000000000900").unwrap()
    );
    assert_eq!(doc.kind, ResourceKind::Document);
    assert_eq!(doc.title, "Markdown Architecture");
    assert_eq!(doc.properties.get("type").map(|s| s.as_str()), Some("project"));

    let heading = &scanned.resources[1];
    assert_eq!(
        heading.r#ref,
        ResourceRef::parse("heading:01J00000000000000000000901").unwrap()
    );
    assert_eq!(heading.kind, ResourceKind::Heading);
    assert_eq!(heading.title, "Introduction");

    assert_eq!(scanned.links.len(), 1);
    let link = &scanned.links[0];
    assert_eq!(
        link.target_ref,
        ResourceRef::parse("heading:01J00000000000000000000902").unwrap()
    );
}
