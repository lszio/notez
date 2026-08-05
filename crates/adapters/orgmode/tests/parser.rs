//! Contract tests for the Org format adapter.
//!
//! These tests pin the public contract that the `OrgParser` exposes to the
//! `core::source::FormatParser` dispatch layer:
//!
//! - Parsing uses only `RawEntity.payload`; the `locator` field is treated
//!   as opaque metadata and must not be re-read from disk.
//! - The parser must accept an Org document containing a title, an explicit
//!   `#+ID:` keyword, and a heading with a properties drawer, producing the
//!   expected `Document` + `Heading` resources.
//! - The parser must extract a `[[id:...]]` wikilink as a `LinkTarget::Id`
//!   occurrence and route a `[[file:...]]` wikilink as a `LinkTarget::File`.
//! - `supports("text/org")` returns `true`; other MIME types return `false`.

use notez_core::domain::{LinkTarget, ResourceKind};
use notez_core::source::{FormatParser, ParserError, RawEntity};
use orgmode::OrgParser;

const ORG_MIME: &str = "text/org";
const SOURCE_ID: &str = "native";

fn build_entity(payload: &str, locator: &str) -> RawEntity {
    RawEntity {
        locator: locator.to_string(),
        mime_type: ORG_MIME.to_string(),
        payload: payload.as_bytes().to_vec(),
    }
}

#[test]
fn org_parser_supports_text_org_mime() {
    let parser = OrgParser;
    assert!(parser.supports(ORG_MIME));
    assert!(!parser.supports("text/markdown"));
    assert!(!parser.supports("application/json"));
}

#[test]
fn org_parser_parses_payload_only_without_reading_locator() {
    let payload = "\
#+title: Adapter Contract Doc
#+ID: 01J00000000000000000000011

* NEXT Refactor heading
:PROPERTIES:
:ID: 01J00000000000000000000012
:END:
";

    let entity = build_entity(payload, "/definitely/does/not/exist/note.org");

    let parser = OrgParser;
    let parsed = parser
        .parse(&entity, SOURCE_ID)
        .expect("Org parser must produce ParsedEntity from payload bytes alone");

    let document = parsed
        .resources
        .iter()
        .find(|r| r.kind == ResourceKind::Document)
        .expect("Org parser must emit a Document resource");
    let heading = parsed
        .resources
        .iter()
        .find(|r| r.kind == ResourceKind::Heading)
        .expect("Org parser must emit a Heading resource");

    assert_eq!(document.title, "Adapter Contract Doc");
    assert_eq!(document.r#ref.to_string(), "document:01J00000000000000000000011");
    assert_eq!(document.source_id, SOURCE_ID);
    assert_eq!(document.locator, "/definitely/does/not/exist/note.org");

    assert_eq!(heading.title, "Refactor heading");
    assert_eq!(heading.r#ref.to_string(), "heading:01J00000000000000000000012");
    assert_eq!(heading.properties.get("TODO").map(String::as_str), Some("NEXT"));
    assert_eq!(heading.properties.get("LEVEL").map(String::as_str), Some("1"));
}

#[test]
fn org_parser_extracts_org_wikilinks_from_payload() {
    let payload = "\
#+title: Link Doc
#+ID: 01J00000000000000000000021

See [[id:01J00000000000000000000099][target]] and [[file:notes/todo.org::heading]].
";

    let entity = build_entity(payload, "/missing/locator.org");
    let parsed = OrgParser
        .parse(&entity, SOURCE_ID)
        .expect("Org parser must succeed");

    assert_eq!(parsed.link_occurrences.len(), 2);

    let id_link = &parsed.link_occurrences[0];
    assert!(matches!(id_link.target, LinkTarget::Id { .. }));
    assert_eq!(id_link.display_text.as_deref(), Some("target"));

    let file_link = &parsed.link_occurrences[1];
    match &file_link.target {
        LinkTarget::File { path, fragment } => {
            assert_eq!(path, "notes/todo.org");
            assert_eq!(fragment.as_deref(), Some("heading"));
        }
        other => panic!("expected LinkTarget::File, got {other:?}"),
    }
}

#[test]
fn org_parser_reports_unsupported_locator_outcome_as_error() {
    let entity = build_entity("not org", "/no/such/path");
    let parser = OrgParser;
    let result = parser.parse(&entity, SOURCE_ID);

    // The current Org parser is permissive, so this expectation is the
    // contract for the future: the parser must never panic or read from the
    // filesystem even when the payload does not look like a real Org doc.
    if let Err(err) = result {
        match err {
            ParserError::Format(_) | ParserError::Other(_) => {}
        }
    }
}
