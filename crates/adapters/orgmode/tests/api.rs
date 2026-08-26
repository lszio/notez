//! Contract tests for the Org format adapter's public API surface.
//!
//! Pin every entry point that the `FormatParser` dispatch layer and other
//! adapters rely on:
//!
//! - `OrgParser::new` and `Default::default` produce equivalent instances.
//! - `OrgParser::supports` returns `true` only for the configured MIME
//!   (`text/org`) and `false` for any other value.
//! - `OrgParser::parse` consumes only the `RawEntity.payload` bytes; the
//!   `locator` is propagated opaquely and never re-opened. The `source_id`
//!   passed alongside `&RawEntity` is the one written into every emitted
//!   `Resource`.
//! - `OrgScanner::parse_bytes`, `OrgScanner::parse_str`, and
//!   `OrgScanner::scan` all yield the same `resources` for the same payload
//!   (the `revision` field, a SHA-256 of the content text, is identical too).
//! - Errors propagate as `ParserError`/`DocumentError`:
//!   - Malformed ULID in a `#+ID:` keyword surfaces `DocumentError::MalformedId`.
//!   - Non-UTF-8 payload bytes surface `DocumentError::Other` (invalid UTF-8).

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use notez_core::document::OrgScanner;
use notez_core::source::{FormatParser, ParserError, RawEntity};
use orgmode::OrgParser;

const ORG_MIME: &str = "text/org";

const ORG_FIXTURE: &str = "\
#+title: Adapter Contract Org
#+ID: 01J00000000000000000000001

* NEXT Refactor heading
:PROPERTIES:
:ID: 01J00000000000000000000002
:END:

See [[id:01J00000000000000000000099][target]] and [[file:notes/todo.org::heading]].
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
    std::env::temp_dir().join(format!("notez-orgmode-api-{pid}-{n}.{suffix}"))
}

#[test]
fn org_parser_new_returns_usable_instance() {
    // Smoke: constructing via `new` must yield a parser that behaves exactly
    // like the unit-struct form (no state, just a trait object).
    let from_new = OrgParser::new();
    let raw = build_entity(ORG_FIXTURE, "/does/not/exist/note.org", ORG_MIME);
    let parsed = OrgParser
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
fn org_parser_default_matches_new() {
    // `Default::default()` must be interchangeable with `OrgParser::new()`.
    let a: OrgParser = Default::default();
    let b = OrgParser::new();

    let raw = build_entity(ORG_FIXTURE, "/missing/note.org", ORG_MIME);
    let ra = a.parse(&raw, "native").expect("default parse");
    let rb = b.parse(&raw, "native").expect("new parse");

    assert_eq!(ra.resources.len(), rb.resources.len());
    assert_eq!(ra.resources[0].r#ref, rb.resources[0].r#ref);
    assert_eq!(ra.resources[0].title, rb.resources[0].title);
}

#[test]
fn org_parser_supports_only_text_org() {
    let parser = OrgParser;
    assert!(
        parser.supports(ORG_MIME),
        "supports(\"text/org\") must be true"
    );
    // Variants / casing / leading whitespace must all reject.
    assert!(!parser.supports("text/markdown"));
    assert!(!parser.supports("application/json"));
    assert!(!parser.supports("TEXT/ORG"));
    assert!(!parser.supports("text/org "));
    assert!(!parser.supports(""));
}

#[test]
fn org_parser_parse_propagates_supplied_source_id() {
    // Contract: the `source_id` argument to `parse` is what each emitted
    // Resource carries in `resource.source_id`. The parser must NOT keep
    // anything from the RawEntity or default to a hard-coded value.
    let entity = build_entity(ORG_FIXTURE, "/some/path/note.org", ORG_MIME);
    let custom_source_id = "filesystem-import-42";

    let parsed = OrgParser
        .parse(&entity, custom_source_id)
        .expect("payload-only parse must succeed");

    assert!(
        parsed.resources.len() >= 2,
        "expected at least Document + Heading resources, got {}",
        parsed.resources.len()
    );
    for r in &parsed.resources {
        assert_eq!(
            r.source_id, custom_source_id,
            "resource {:?} carried wrong source_id",
            r.r#ref
        );
        // Locator must be passed through opaquely.
        assert_eq!(r.locator, "/some/path/note.org");
    }

    // And a different source_id must round-trip with a different value.
    let alt = "vault-b";
    let parsed_alt = OrgParser
        .parse(&entity, alt)
        .expect("alternate source_id parse must succeed");
    for r in &parsed_alt.resources {
        assert_eq!(r.source_id, alt);
    }
}

#[test]
fn org_parser_parse_is_payload_only_and_ignores_locator_disk_state() {
    // Even when the locator points to a non-existent path, parsing must
    // succeed using only the bytes in `payload`. This pins the "no disk I/O"
    // contract of the adapter.
    let entity = build_entity(
        ORG_FIXTURE,
        "/definitely/does/not/exist/some-org-doc.org",
        ORG_MIME,
    );

    let parsed = OrgParser
        .parse(&entity, "native")
        .expect("parse must succeed without ever touching the locator path");

    // Document + at least the heading from the fixture must be present.
    let document = parsed
        .resources
        .iter()
        .find(|r| matches!(r.kind, notez_core::domain::ResourceKind::Document))
        .expect("Document resource must exist");
    assert_eq!(document.title, "Adapter Contract Org");
    assert_eq!(document.locator, "/definitely/does/not/exist/some-org-doc.org");
}

#[test]
fn org_scanner_parse_bytes_and_parse_str_produce_equivalent_resources() {
    // The two payload-only OrgScanner entry points must converge on the same
    // semantic content. `revision` is the SHA-256 of the content text, so it
    // is also identical.
    let locator = "/memory/note.org";
    let source_id = "native";

    let bytes_doc = OrgScanner::parse_bytes(ORG_FIXTURE.as_bytes(), source_id, locator)
        .expect("parse_bytes must succeed");
    let str_doc =
        OrgScanner::parse_str(ORG_FIXTURE, source_id, locator).expect("parse_str must succeed");

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
fn org_scanner_scan_matches_parse_bytes_for_same_payload() {
    // The disk-backed entry point must reach the same resources as the
    // payload-only entry points. The locator used for emitted resources is
    // the file path, matching the parse_bytes call below.
    let path = unique_tmp_path("org");
    fs::write(&path, ORG_FIXTURE).expect("failed to write org fixture");

    let locator_str = path.to_string_lossy().to_string();
    let scanned = orgmode::OrgParser::new()
        .parse_path(&path, "native")
        .expect("scan must succeed");
    let in_memory = OrgScanner::parse_bytes(ORG_FIXTURE.as_bytes(), "native", &locator_str)
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
fn org_scanner_returns_malformed_id_error_for_bad_ulid_in_keyword() {
    // A malformed ULID in `#+ID:` must surface as a parse error.
    let bad = "\
#+title: Bad ID Doc
#+ID: not-a-ulid

* Heading
";

    let err = OrgScanner::parse_bytes(bad.as_bytes(), "native", "/bad/note.org")
        .expect_err("malformed ULID must surface as DocumentError");

    // The display message must mention the malformed ID.
    let msg = err.to_string();
    assert!(
        msg.contains("malformed ID") || msg.contains("MalformedId") || msg.contains("invalid"),
        "error message must mention malformed/invalid ID, got: {msg}"
    );

    // Wrapping through the parser contract must yield a `ParserError::Format`.
    let entity = build_entity(bad, "/bad/note.org", ORG_MIME);
    let parser_err = OrgParser
        .parse(&entity, "native")
        .expect_err("OrgParser must propagate malformed-id as ParserError");
    assert!(
        matches!(parser_err, ParserError::Format(_)),
        "expected ParserError::Format, got {parser_err:?}"
    );
}

#[test]
fn org_scanner_rejects_non_utf8_bytes() {
    // 0xFF / 0xFE are never valid UTF-8 lead bytes, so this payload must
    // surface as `DocumentError::Other("invalid UTF-8 ...")`.
    let bad_bytes: &[u8] = &[0xFF, 0xFE, 0xFD, 0x00, b'h', b'i'];
    let err = OrgScanner::parse_bytes(bad_bytes, "native", "/binary/note.org")
        .expect_err("non-UTF-8 payload must surface as DocumentError");
    let msg = err.to_string();
    assert!(
        msg.contains("UTF-8") || msg.contains("utf-8") || msg.contains("invalid"),
        "error must mention UTF-8 / invalid encoding, got: {msg}"
    );

    // Same wrapping contract via the parser.
    let raw_invalid = RawEntity {
        locator: "/binary/note.org".to_string(),
        mime_type: ORG_MIME.to_string(),
        payload: bad_bytes.to_vec(),
    };
    let parser_err = OrgParser
        .parse(&raw_invalid, "native")
        .expect_err("OrgParser must propagate non-UTF-8 as ParserError");
    assert!(
        matches!(parser_err, ParserError::Format(_)),
        "expected ParserError::Format, got {parser_err:?}"
    );
}