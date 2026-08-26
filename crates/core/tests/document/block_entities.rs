use crate::document::{MarkdownScanner, OrgScanner};
use crate::domain::{ResourceKind, ResourceRef};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_block_level_entities_in_org_and_markdown() {
    let temp = tempdir().unwrap();

    let org_file = temp.path().join("block_note.org");
    let org_content = r#"#+title: Block Doc
#+ID: 01J00000000000000000000050

* Header
Paragraph line 1.
#+NAME: src_block_1
#+BEGIN_SRC rust
fn hello() {}
#+END_SRC
"#;
    fs::write(&org_file, org_content).unwrap();

    let scanned_org = { let __bytes = std::fs::read(&org_file).unwrap(); OrgScanner::parse_bytes(&__bytes, "native", &org_file.to_string_lossy()).unwrap() };
    let block_resources: Vec<_> = scanned_org
        .resources
        .iter()
        .filter(|r| r.kind == ResourceKind::Block)
        .collect();

    assert!(!block_resources.is_empty());
    assert_eq!(block_resources[0].title, "src_block_1");

    let md_file = temp.path().join("block_note.md");
    let md_content = r#"---
title: MD Block Doc
id: 01J00000000000000000000051
---

Paragraph text line. <!-- id: block:01J00000000000000000000052 -->
"#;
    fs::write(&md_file, md_content).unwrap();

    let scanned_md = { let __bytes = std::fs::read(&md_file).unwrap(); MarkdownScanner::parse_bytes(&__bytes, "native", &md_file.to_string_lossy()).unwrap() };
    let md_block_resources: Vec<_> = scanned_md
        .resources
        .iter()
        .filter(|r| r.kind == ResourceKind::Block)
        .collect();

    assert_eq!(md_block_resources.len(), 1);
    assert_eq!(
        md_block_resources[0].r#ref,
        ResourceRef::parse("block:01J00000000000000000000052").unwrap()
    );
}
