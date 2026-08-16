//! Registry behavior tests for the unified source scanning pipeline.
//!
//! These tests pin the contract that `ComposedSourceAdapter` (and any
//! future scanner driven by the same registry) must satisfy:
//!
//! 1. A single scan over a transport that yields both Org and Markdown
//!    entities dispatches each entity to the matching parser, producing
//!    combined `resources`, `relations`, and `link_occurrences`.
//! 2. An entity with an unknown MIME causes the scan to fail with
//!    `SourceError::ParserNotFound`; partial resources are NOT returned.
//! 3. A parser error short-circuits the scan; nothing reaches the caller.
//! 4. On success, exactly one `ScannedSource` is produced and contains the
//!    full set of expected items.

use notez_core::source::protocol::{ParserError, RawEntity, SourceTransport, TransportError};
use notez_core::source::{
    ComposedSourceAdapter, FormatParser, ParsedEntity, PreparedWrite, ScannedSource, SourceAdapter,
    SourceCapabilities, SourceConfig, SourceError, SourceKind, WriteResult,
};
use std::path::PathBuf;

/// Test transport that returns a handcrafted stream of raw entities in a
/// deterministic order. The transport is not required to know which
/// parsers are present; the adapter picks them.
struct ListTransport {
    entities: Vec<RawEntity>,
}

impl ListTransport {
    fn new(entities: Vec<RawEntity>) -> Self {
        Self { entities }
    }
}

impl SourceTransport for ListTransport {
    fn fetch_raw(&self) -> Result<Vec<RawEntity>, TransportError> {
        Ok(self.entities.clone())
    }

    fn mutate(&self, _locator: &str, _payload: &str) -> Result<(), TransportError> {
        Err(TransportError::Other("mutation unsupported".into()))
    }
}

/// Parser that always reports success for `text/org` and returns the
/// locator as a single Document resource, plus the supplied link.
struct FakeOrg {
    extra_links: usize,
}

impl FormatParser for FakeOrg {
    fn supports(&self, mime: &str) -> bool {
        mime == "text/org"
    }
    fn parse(&self, entity: &RawEntity, source_id: &str) -> Result<ParsedEntity, ParserError> {
use notez_core::domain::{Resource, ResourceKind, ResourceRef, derived_id};
        let doc_ref = derived_id(ResourceKind::Document, source_id, &entity.locator, "fake-org");
        let mut resources = vec![Resource {
            r#ref: doc_ref,
            kind: ResourceKind::Document,
            title: format!("OrgDoc:{}", entity.locator),
            revision: "r1".to_string(),
            source_id: source_id.to_string(),
            locator: entity.locator.clone(),
            properties: Default::default(),
            object_id: notez_core::domain::ObjectId::default(),
        }];
        // We can't synthesise LinkOccurrence without going through the
        // link model; just emit empty occurrences.
        let _ = self.extra_links;
        let _ = resources.pop();
        // Re-add as Document so resource count is 1
        resources.push(Resource {
            r#ref: doc_ref,
            kind: ResourceKind::Document,
            title: format!("OrgDoc:{}", entity.locator),
            revision: "r1".to_string(),
            source_id: source_id.to_string(),
            locator: entity.locator.clone(),
            properties: Default::default(),
            object_id: notez_core::domain::ObjectId::default(),
        });
        Ok(ParsedEntity {
            resources,
            relations: vec![],
            link_occurrences: vec![],
        })
    }
}

/// Markdown parser that emits a single document resource per entity.
struct FakeMarkdown;

impl FormatParser for FakeMarkdown {
    fn supports(&self, mime: &str) -> bool {
        mime == "text/markdown"
    }
    fn parse(&self, entity: &RawEntity, source_id: &str) -> Result<ParsedEntity, ParserError> {
use notez_core::domain::{Resource, ResourceKind, ResourceRef, derived_id};
        let doc_ref = derived_id(ResourceKind::Document, source_id, &entity.locator, "fake-md");
        Ok(ParsedEntity {
            resources: vec![Resource {
                r#ref: doc_ref,
                kind: ResourceKind::Document,
                title: format!("MdDoc:{}", entity.locator),
                revision: "r1".to_string(),
                source_id: source_id.to_string(),
                locator: entity.locator.clone(),
                properties: Default::default(),
                object_id: notez_core::domain::ObjectId::default(),
            }],
            relations: vec![],
            link_occurrences: vec![],
        })
    }
}

/// Parser that always returns an error.
struct BoomParser;

impl FormatParser for BoomParser {
    fn supports(&self, mime: &str) -> bool {
        mime == "application/x-fake"
    }
    fn parse(&self, _entity: &RawEntity, _source_id: &str) -> Result<ParsedEntity, ParserError> {
        Err(ParserError::Format("boom".into()))
    }
}

fn config(id: &str) -> SourceConfig {
    SourceConfig {
        id: id.to_string(),
        kind: SourceKind::Native,
        path: PathBuf::from("/space"),
        read_only: true,
        include_paths: vec![],
        exclude_paths: vec![],
    }
}

#[test]
fn registry_dispatches_per_mime_and_combines_results() {
    let transport = ListTransport::new(vec![
        RawEntity {
            locator: "/space/a.org".to_string(),
            mime_type: "text/org".to_string(),
            payload: b"* Heading".to_vec(),
        },
        RawEntity {
            locator: "/space/b.md".to_string(),
            mime_type: "text/markdown".to_string(),
            payload: b"# Heading".to_vec(),
        },
    ]);

    let adapter = ComposedSourceAdapter::new(
        config("native"),
        Box::new(transport),
        vec![Box::new(FakeOrg { extra_links: 0 }), Box::new(FakeMarkdown)],
    );

    let scanned = adapter.scan().expect("scan must succeed for known MIMEs");
    assert_eq!(scanned.source_id, "native");
    assert_eq!(scanned.resources.len(), 2);
    let titles: Vec<&str> = scanned.resources.iter().map(|r| r.title.as_str()).collect();
    assert!(titles.contains(&"OrgDoc:/space/a.org"));
    assert!(titles.contains(&"MdDoc:/space/b.md"));
}

#[test]
fn registry_rejects_unknown_mime_without_partial_results() {
    let transport = ListTransport::new(vec![
        RawEntity {
            locator: "/space/a.org".to_string(),
            mime_type: "text/org".to_string(),
            payload: b"* Heading".to_vec(),
        },
        RawEntity {
            locator: "/space/c.json".to_string(),
            mime_type: "application/json".to_string(),
            payload: b"{}".to_vec(),
        },
    ]);

    let adapter = ComposedSourceAdapter::new(
        config("native"),
        Box::new(transport),
        vec![Box::new(FakeOrg { extra_links: 0 })],
    );

    let err = adapter.scan().expect_err("scan must fail on unknown MIME");
    match err {
        SourceError::ParserNotFound { source_id, mime, locator } => {
            assert_eq!(source_id, "native");
            assert_eq!(mime, "application/json");
            assert_eq!(locator, "/space/c.json");
        }
        other => panic!("expected ParserNotFound, got {other:?}"),
    }
}

#[test]
fn registry_short_circuits_on_parser_error() {
    let transport = ListTransport::new(vec![
        RawEntity {
            locator: "/space/a.fake".to_string(),
            mime_type: "application/x-fake".to_string(),
            payload: vec![],
        },
        RawEntity {
            locator: "/space/b.org".to_string(),
            mime_type: "text/org".to_string(),
            payload: b"* Heading".to_vec(),
        },
    ]);

    let adapter = ComposedSourceAdapter::new(
        config("native"),
        Box::new(transport),
        vec![Box::new(BoomParser), Box::new(FakeOrg { extra_links: 0 })],
    );

    let err = adapter.scan().expect_err("scan must surface parser error");
    match err {
        SourceError::Parse(msg) => assert_eq!(msg, "Format parse error: boom"),
        other => panic!("expected Parse, got {other:?}"),
    }
}

#[test]
fn registry_returns_exactly_one_scanned_source_on_success() {
    let transport = ListTransport::new(vec![
        RawEntity {
            locator: "/space/a.org".to_string(),
            mime_type: "text/org".to_string(),
            payload: b"* Heading".to_vec(),
        },
        RawEntity {
            locator: "/space/b.org".to_string(),
            mime_type: "text/org".to_string(),
            payload: b"* Second".to_vec(),
        },
    ]);

    let adapter = ComposedSourceAdapter::new(
        config("native"),
        Box::new(transport),
        vec![Box::new(FakeOrg { extra_links: 0 })],
    );

    let scanned: ScannedSource = adapter.scan().expect("scan must succeed");
    assert_eq!(scanned.resources.len(), 2);
    assert_eq!(scanned.relations.len(), 0);
    assert_eq!(scanned.link_occurrences.len(), 0);
}
