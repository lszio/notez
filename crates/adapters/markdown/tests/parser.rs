//! Contract tests for the Markdown format adapter.
//!
//! Pin the `MarkdownParser` contract analogous to the Org adapter:
//!
//! - Parses only `RawEntity.payload`; never reads `locator` from disk.
//! - `supports("text/markdown")` returns `true`.
//! - Extracts wikilink + inline link occurrences as expected `LinkTarget`s.
//! - Honors YAML frontmatter for `title` and `id`.

use notez_core::domain::{LinkTarget, ResourceKind};
use notez_core::source::{FormatParser, RawEntity};
use markdown::MarkdownParser;

const MD_MIME: &str = "text/markdown";
const SOURCE_ID: &str = "native";

fn build_entity(payload: &str, locator: &str) -> RawEntity {
    RawEntity {
        locator: locator.to_string(),
        mime_type: MD_MIME.to_string(),
        payload: payload.as_bytes().to_vec(),
    }
}

#[test]
fn markdown_parser_supports_text_markdown_mime() {
    let parser = MarkdownParser;
    assert!(parser.supports(MD_MIME));
    assert!(!parser.supports("text/org"));
    assert!(!parser.supports("application/json"));
}

#[test]
fn markdown_parser_parses_frontmatter_and_headings() {
    let payload = "\
---
id: 01J00000000000000000000051
title: Adapter Contract Markdown
---

# Top Section

## Sub Section <!-- id: 01J00000000000000000000052 -->
";

    let entity = build_entity(payload, "/definitely/does/not/exist/page.md");
    let parsed = MarkdownParser
        .parse(&entity, SOURCE_ID)
        .expect("Markdown parser must succeed from payload");

    let doc = parsed
        .resources
        .iter()
        .find(|r| r.kind == ResourceKind::Document)
        .expect("Document resource expected");
    assert_eq!(doc.title, "Adapter Contract Markdown");
    assert_eq!(doc.r#ref.to_string(), "document:01J00000000000000000000051");
    assert_eq!(doc.source_id, SOURCE_ID);
    assert_eq!(doc.locator, "/definitely/does/not/exist/page.md");

    let headings: Vec<_> = parsed
        .resources
        .iter()
        .filter(|r| r.kind == ResourceKind::Heading)
        .collect();
    assert_eq!(headings.len(), 2);
    assert_eq!(headings[0].title, "Top Section");
    assert_eq!(
        headings[1].r#ref.to_string(),
        "heading:01J00000000000000000000052"
    );
}

#[test]
fn markdown_parser_extracts_wikilinks_and_inline_links() {
    let payload = "\
---
id: 01J00000000000000000000061
title: Link Markdown
---

See [[Obsidian Note]] and [docs](https://example.com) and [[id:01J00000000000000000000062][target]].
";

    let entity = build_entity(payload, "/missing/page.md");
    let parsed = MarkdownParser
        .parse(&entity, SOURCE_ID)
        .expect("Markdown parser must succeed");

    assert_eq!(parsed.link_occurrences.len(), 3);

    assert!(matches!(
        parsed.link_occurrences[0].target,
        LinkTarget::Title { .. }
    ));
    assert!(matches!(
        parsed.link_occurrences[1].target,
        LinkTarget::Url { .. }
    ));
    let id_link = &parsed.link_occurrences[2];
    assert!(matches!(id_link.target, LinkTarget::Id { .. }));
    assert_eq!(id_link.display_text.as_deref(), Some("target"));
}
