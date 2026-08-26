use crate::document::MarkdownScanner;
use crate::domain::{LinkTarget, ResourceKind, ResourceRef};
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

    let scanned = { let __bytes = std::fs::read(&file).unwrap(); MarkdownScanner::parse_bytes(&__bytes, "native", &file.to_string_lossy()).unwrap() };

    assert_eq!(scanned.resources.len(), 2);

    let doc = &scanned.resources[0];
    assert_eq!(
        doc.r#ref,
        ResourceRef::parse("document:01J00000000000000000000900").unwrap()
    );
    assert_eq!(doc.kind, ResourceKind::Document);
    assert_eq!(doc.title, "Markdown Architecture");
    assert_eq!(
        doc.properties.get("type").map(|s| s.as_str()),
        Some("project")
    );

    let heading = &scanned.resources[1];
    assert_eq!(
        heading.r#ref,
        ResourceRef::parse("heading:01J00000000000000000000901").unwrap()
    );
    assert_eq!(heading.kind, ResourceKind::Heading);
    assert_eq!(heading.title, "Introduction");

    // link_occurrences captures all links (wikilink + id link)
    assert_eq!(scanned.link_occurrences.len(), 2);

    // First: [[Obsidian Vault]] → Title link
    let occ0 = &scanned.link_occurrences[0];
    assert!(matches!(&occ0.target, LinkTarget::Title { title, .. } if title == "Obsidian Vault"));

    // Second: [[id:01J...902][Target Link]] → Id link
    let occ1 = &scanned.link_occurrences[1];
    assert!(matches!(&occ1.target, LinkTarget::Id { value, .. } if value == "01J00000000000000000000902"));
    assert_eq!(occ1.display_text.as_deref(), Some("Target Link"));

    // Legacy links: bare id without kind_hint → not in legacy links
    assert_eq!(scanned.links.len(), 0);
}
