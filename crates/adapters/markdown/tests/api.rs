//! Contract tests for the Markdown format adapter's public API surface.
//!
//! Pin every entry point that the `FormatParser` dispatch layer and other
//! adapters rely on:
//!
//! - `MarkdownParser::new` and `Default::default` produce equivalent
//!   instances.
//! - `MarkdownParser::supports` returns `true` only for the configured MIME
//!   (`text/markdown`) and `false` for any other value.
//! - `MarkdownParser::parse` consumes only the `RawEntity.payload` bytes;
//!   the `locator` is propagated opaquely and never re-opened. The
//!   `source_id` passed alongside `&RawEntity` is the one written into every
//!   emitted `Resource`.
//! - `MarkdownScanner::parse_bytes`, `MarkdownScanner::parse_str`, and
//!   `MarkdownScanner::scan` all yield the same `resources` for the same
//!   payload (the `revision` field, a SHA-256 of the content text, is
//!   identical too).
//! - Error propagation:
//!   - Non-UTF-8 payload bytes surface `DocumentError::Other` (invalid UTF-8).
//!   - Malformed ULID in YAML `id:` or in a `<!-- id: ... -->` heading
//!     comment falls back to a derived ULID rather than raising (the
//!     Markdown parser is intentionally permissive here).

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use notez_core::document::MarkdownScanner;
use notez_core::source::{FormatParser, ParserError, RawEntity};
use markdown::MarkdownParser;

const MD_MIME: &str = "text/markdown";

const MD_FIXTURE: &str = "\
---
:id: 01J00000000000000000000001
title: Adapter Contract Markdown
---

# Top Section

## Sub Section <!-- id: 01J00000000000000000000002 -->

See [[Obsidian Note]] and [docs](https://example.com) and [[id:01J00000000000000000000003][target]].
";

fn build_entity(payload: &str, locator: &str, mime_type: &str) -> RawEntity {
    RawEntity {
        locator: locator.to_string(),
        mime_type: mime_type.to_string(),
        payload: payload.as_bytes().to_vec(),
    }
}

/// Generates a unique absolute path inside `std::env::temp_dir()` so concurrent
/// `cargo test` invocations don't share filenames. Uses `std::sync::atomic` to
/// disambiguate — avoids pulling in any new dev-dependencies.
fn unique_tmp_path(suffix: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let pid = std::process::id();
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("notez-markdown-api-{pid}-{n}.{suffix}"))
}

#[test]
fn markdown_parser_new_returns_usable_instance() {
    // Smoke: constructing via `new` must yield a parser that behaves exactly
    // like the unit-struct form (no state, just a trait object).
    let from_new = MarkdownParser::new();
    let raw = build_entity(MD_FIXTURE, "/does/not/exist/page.md", MD_MIME);
    let parsed = MarkdownParser
        .parse(&raw, "native")
        .expect("unit-struct parse must succeed");
    let from_new_parsed = from_new
        .parse(&raw, "native")
        .expect("`new()` parse must succeed");
    assert_eq!(parsed.resources.len(), from_new_parsed.resources.len());
    assert_eq!(
        parsed.resources[0].r#ref,
        from_new_parsed.resources[0].r#ref
    );
}

#[test]
fn markdown_parser_default_matches_new() {
    // `Default::default()` must be interchangeable with `MarkdownParser::new()`.
    let a: MarkdownParser = Default::default();
    let b = MarkdownParser::new();

    let raw = build_entity(MD_FIXTURE, "/missing/page.md", MD_MIME);
    let ra = a.parse(&raw, "native").expect("default parse");
    let rb = b.parse(&raw, "native").expect("new parse");

    assert_eq!(ra.resources.len(), rb.resources.len());
    assert_eq!(ra.resources[0].r#ref, rb.resources[0].r#ref);
    assert_eq!(ra.resources[0].title, rb.resources[0].title);
}

#[test]
fn markdown_parser_supports_only_text_markdown() {
    let parser = MarkdownParser;
    assert!(
        parser.supports(MD_MIME),
        "supports(\"text/markdown\") must be true"
    );
    // Variants / casing / leading whitespace must all reject.
    assert!(!parser.supports("text/org"));
    assert!(!parser.supports("application/json"));
    assert!(!parser.supports("TEXT/MARKDOWN"));
    assert!(!parser.supports("text/markdown "));
    assert!(!parser.supports(""));
}

#[test]
fn markdown_parser_parse_propagates_supplied_source_id() {
    // Contract: the `source_id` argument to `parse` is what each emitted
    // Resource carries in `resource.source_id`. The parser must NOT keep
    // anything from the RawEntity or default to a hard-coded value.
    let entity = build_entity(MD_FIXTURE, "/some/path/page.md", MD_MIME);
    let custom_source_id = "filesystem-import-42";

    let parsed = MarkdownParser
        .parse(&entity, custom_source_id)
        .expect("payload-only parse must succeed");

    assert!(
        parsed.resources.len() >= 3,
        "expected at least Document + 2 Heading resources, got {}",
        parsed.resources.len()
    );
    for r in &parsed.resources {
        assert_eq!(
            r.source_id, custom_source_id,
            "resource {:?} carried wrong source_id",
            r.r#ref
        );
        // Locator must be passed through opaquely.
        assert_eq!(r.locator, "/some/path/page.md");
    }

    // And a different source_id must round-trip with a different value.
    let alt = "vault-b";
    let parsed_alt = MarkdownParser
        .parse(&entity, alt)
        .expect("alternate source_id parse must succeed");
    for r in &parsed_alt.resources {
        assert_eq!(r.source_id, alt);
    }
}

#[test]
fn markdown_parser_parse_is_payload_only_and_ignores_locator_disk_state() {
    // Even when the locator points to a non-existent path, parsing must
    // succeed using only the bytes in `payload`. This pins the "no disk I/O"
    // contract of the adapter.
    let entity = build_entity(
        MD_FIXTURE,
        "/definitely/does/not/exist/some-md-doc.md",
        MD_MIME,
    );

    let parsed = MarkdownParser
        .parse(&entity, "native")
        .expect("parse must succeed without ever touching the locator path");

    let document = parsed
        .resources
        .iter()
        .find(|r| matches!(r.kind, notez_core::domain::ResourceKind::Document))
        .expect("Document resource must exist");
    assert_eq!(document.title, "Adapter Contract Markdown");
    assert_eq!(document.locator, "/definitely/does/not/exist/some-md-doc.md");
}

#[test]
fn markdown_scanner_parse_bytes_and_parse_str_produce_equivalent_resources() {
    // The two payload-only MarkdownScanner entry points must converge on the
    // same semantic content. `revision` is the SHA-256 of the content text,
    // so it is also identical.
    let locator = "/memory/page.md";
    let source_id = "native";

    let bytes_doc = MarkdownScanner::parse_bytes(MD_FIXTURE.as_bytes(), source_id, locator)
        .expect("parse_bytes must succeed");
    let str_doc = MarkdownScanner::parse_str(MD_FIXTURE, source_id, locator)
        .expect("parse_str must succeed");

    assert_eq!(
        bytes_doc.resources.len(),
        str_doc.resources.len(),
        "parse_bytes and parse_str produced different resource counts"
    );

    // Resource refs + titles + properties must match in order.
    for (a, b) in bytes_doc.resources.iter().zip(str_doc.resources.iter()) {
        assert_eq!(a.r#ref, b.r#ref);
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.title, b.title);
        assert_eq!(a.source_id, b.source_id);
        assert_eq!(a.locator, b.locator);
        assert_eq!(a.revision, b.revision);
        assert_eq!(a.properties, b.properties);
    }

    // Link occurrences must also be identical.
    assert_eq!(bytes_doc.link_occurrences.len(), str_doc.link_occurrences.len());
    for (a, b) in bytes_doc
        .link_occurrences
        .iter()
        .zip(str_doc.link_occurrences.iter())
    {
        assert_eq!(a.target, b.target);
        assert_eq!(a.display_text, b.display_text);
        assert_eq!(a.raw, b.raw);
    }
}

#[test]
fn markdown_scanner_scan_matches_parse_bytes_for_same_payload() {
    // The disk-backed entry point must reach the same resources as the
    // payload-only entry points. The locator used for emitted resources is
    // the file path, matching the parse_bytes call below.
    let path = unique_tmp_path("md");
    fs::write(&path, MD_FIXTURE).expect("failed to write md fixture");

    let locator_str = path.to_string_lossy().to_string();
    let scanned = MarkdownScanner::scan(&path, "native").expect("scan must succeed");
    let in_memory = MarkdownScanner::parse_bytes(MD_FIXTURE.as_bytes(), "native", &locator_str)
        .expect("parse_bytes must succeed");

    assert_eq!(scanned.resources.len(), in_memory.resources.len());
    for (a, b) in scanned.resources.iter().zip(in_memory.resources.iter()) {
        assert_eq!(a.r#ref, b.r#ref);
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.title, b.title);
        assert_eq!(a.revision, b.revision);
        assert_eq!(a.source_id, b.source_id);
        assert_eq!(a.locator, b.locator);
        assert_eq!(a.properties, b.properties);
    }

    fs::remove_file(&path).ok();
}

#[test]
fn markdown_scanner_falls_back_to_derived_id_on_malformed_frontmatter_id() {
    // The Markdown scanner is permissive: a malformed ULID in YAML `id:`
    // silently falls back to a derived ULID rather than raising. This pins
    // the asymmetry with the Org parser (which is strict).
    let bad = "\
---
id: not-a-ulid
title: Permissive MD
---

# Heading
";

    let parsed = MarkdownScanner::parse_bytes(bad.as_bytes(), "native", "/bad/page.md")
        .expect("malformed frontmatter id must NOT raise; it falls back");

    let document = parsed
        .resources
        .iter()
        .find(|r| matches!(r.kind, notez_core::domain::ResourceKind::Document))
        .expect("Document resource must exist");
    // The derived ID is non-empty and stable across runs (no random bits).
    assert!(!document.r#ref.to_string().is_empty());
    assert_eq!(document.title, "Permissive MD");

    // Heading-id comments follow the same rule.
    let bad_heading = "\
---
id: 01J00000000000000000000010
title: Bad Heading ID
---

# Heading <!-- id: not-a-ulid -->
";
    let parsed2 =
        MarkdownScanner::parse_bytes(bad_heading.as_bytes(), "native", "/bad/page.md")
            .expect("malformed heading id must NOT raise; it falls back");
    let heading = parsed2
        .resources
        .iter()
        .find(|r| matches!(r.kind, notez_core::domain::ResourceKind::Heading))
        .expect("Heading resource must exist");
    assert_eq!(heading.title, "Heading");
    assert!(
        !heading.r#ref.to_string().is_empty(),
        "heading ref must be a derived ULID, not blank"
    );
}

#[test]
fn markdown_scanner_rejects_non_utf8_bytes() {
    // 0xFF / 0xFE are never valid UTF-8 lead bytes, so this payload must
    // surface as `DocumentError::Other("invalid UTF-8 ...")`.
    let bad_bytes: &[u8] = &[0xFF, 0xFE, 0xFD, 0x00, b'h', b'i'];
    let err = MarkdownScanner::parse_bytes(bad_bytes, "native", "/binary/page.md")
        .expect_err("non-UTF-8 payload must surface as DocumentError");
    let msg = err.to_string();
    assert!(
        msg.contains("UTF-8") || msg.contains("utf-8") || msg.contains("invalid"),
        "error must mention UTF-8 / invalid encoding, got: {msg}"
    );

    // Same wrapping contract via the parser.
    let raw_invalid = RawEntity {
        locator: "/binary/page.md".to_string(),
        mime_type: MD_MIME.to_string(),
        payload: bad_bytes.to_vec(),
    };
    let parser_err = MarkdownParser
        .parse(&raw_invalid, "native")
        .expect_err("MarkdownParser must propagate non-UTF-8 as ParserError");
    assert!(
        matches!(parser_err, ParserError::Format(_)),
        "expected ParserError::Format, got {parser_err:?}"
    );
}