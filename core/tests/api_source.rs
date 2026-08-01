use notez_core::domain::{LinkOccurrence, Resource, ResourceKind, ResourceRef, ResourceRelation};
use notez_core::source::protocol::{
    FormatParser, ParsedEntity, ParserError, RawEntity, SourceTransport, TransportError,
};
use notez_core::source::{
    ComposedSourceAdapter, NativeSourceAdapter, SourceAdapter, SourceCapabilities, SourceConfig,
    SourceError, SourceKind,
};
use std::path::PathBuf;

const DOC_ID: &str = "01J00000000000000000000001";

fn config(id: &str, path: PathBuf) -> SourceConfig {
    SourceConfig::new(id, SourceKind::Native, path, false)
}

fn fake_resource(source_id: &str, locator: &str) -> Resource {
    Resource {
        r#ref: ResourceRef::parse(&format!("document:{DOC_ID}")).unwrap(),
        kind: ResourceKind::Document,
        title: format!("Document at {locator}"),
        revision: "1".into(),
        source_id: source_id.into(),
        locator: locator.into(),
        properties: Default::default(),
    }
}

#[derive(Clone)]
struct ListTransport {
    entities: Vec<RawEntity>,
    mutation: Result<(), String>,
}

impl SourceTransport for ListTransport {
    fn fetch_raw(&self) -> Result<Vec<RawEntity>, TransportError> {
        Ok(self.entities.clone())
    }

    fn mutate(&self, _locator: &str, _payload: &str) -> Result<(), TransportError> {
        self.mutation
            .clone()
            .map_err(TransportError::Other)
    }
}

struct MimeParser {
    mime: &'static str,
    fail: bool,
}

impl FormatParser for MimeParser {
    fn supports(&self, mime_type: &str) -> bool {
        mime_type == self.mime
    }

    fn parse(&self, entity: &RawEntity, source_id: &str) -> Result<ParsedEntity, ParserError> {
        if self.fail {
            return Err(ParserError::Format("parser exploded".into()));
        }
        Ok(ParsedEntity {
            resources: vec![fake_resource(source_id, &entity.locator)],
            relations: Vec::<ResourceRelation>::new(),
            link_occurrences: Vec::<LinkOccurrence>::new(),
        })
    }
}

#[test]
fn source_config_new_and_source_kind_are_serializable_and_capabilities_have_defaults() {
    let mut cfg = SourceConfig::new("native", SourceKind::Native, "/notes", true);
    cfg.include_paths.push(PathBuf::from("docs"));
    cfg.exclude_paths.push(PathBuf::from("private"));
    assert_eq!(cfg.id, "native");
    assert_eq!(cfg.kind, SourceKind::Native);
    assert!(cfg.read_only);
    assert_eq!(cfg.include_paths, vec![PathBuf::from("docs")]);
    assert_eq!(cfg.exclude_paths, vec![PathBuf::from("private")]);
    let json = serde_json::to_string(&cfg).unwrap();
    assert_eq!(serde_json::from_str::<SourceConfig>(&json).unwrap(), cfg);
    assert_eq!(serde_json::to_string(&SourceKind::AppleNotes).unwrap(), "\"apple_notes\"");

    let transport = ListTransport { entities: vec![], mutation: Ok(()) };
    let adapter = ComposedSourceAdapter::new(cfg.clone(), Box::new(transport), vec![]);
    assert_eq!(adapter.config(), &cfg);
    assert_eq!(adapter.capabilities(), SourceCapabilities {
        can_read: true,
        can_write: false,
        can_import: false,
        can_watch: false,
    });
}

#[test]
fn source_transport_fetches_raw_entities_and_default_mutation_reports_unsupported() {
    let transport = ListTransport {
        entities: vec![RawEntity {
            locator: "notes.md".into(),
            mime_type: "text/markdown".into(),
            payload: b"# Notes".to_vec(),
        }],
        mutation: Ok(()),
    };
    let fetched = transport.fetch_raw().unwrap();
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].locator, "notes.md");
    assert!(transport.mutate("notes.md", "payload").is_ok());

    let unsupported = <ListTransport as SourceTransport>::mutate(
        &ListTransport { entities: vec![], mutation: Err("unused".into()) },
        "locator",
        "payload",
    );
    assert!(matches!(unsupported, Err(TransportError::Other(msg)) if msg == "unused"));

    let mut default_transport = NoMutationTransport;
    assert!(matches!(default_transport.fetch_raw(), Ok(items) if items.is_empty()));
    assert!(matches!(default_transport.mutate("x", "y"), Err(TransportError::Other(msg)) if msg.contains("not supported")));
}

struct NoMutationTransport;
impl SourceTransport for NoMutationTransport {
    fn fetch_raw(&self) -> Result<Vec<RawEntity>, TransportError> {
        Ok(vec![])
    }
}

#[test]
fn format_parser_supports_and_parses_entities_or_returns_parser_error() {
    let parser = MimeParser { mime: "text/fake", fail: false };
    assert!(parser.supports("text/fake"));
    assert!(!parser.supports("text/other"));
    let entity = RawEntity {
        locator: "virtual.fake".into(),
        mime_type: "text/fake".into(),
        payload: b"payload".to_vec(),
    };
    let parsed = parser.parse(&entity, "fake").unwrap();
    assert_eq!(parsed.resources[0].source_id, "fake");
    assert_eq!(parsed.resources[0].locator, "virtual.fake");

    let failing = MimeParser { mime: "text/fake", fail: true };
    let error = failing.parse(&entity, "fake").unwrap_err();
    assert!(matches!(&error, ParserError::Format(msg) if msg == "parser exploded"));
    assert!(error.to_string().contains("Format parse error"));
}

#[test]
fn composed_source_adapter_dispatches_parsers_and_supports_write_contract() {
    let transport = ListTransport {
        entities: vec![
            RawEntity {
                locator: "a.fake".into(),
                mime_type: "text/fake".into(),
                payload: vec![1],
            },
            RawEntity {
                locator: "b.fake".into(),
                mime_type: "text/fake".into(),
                payload: vec![2],
            },
        ],
        mutation: Ok(()),
    };
    let mut adapter = ComposedSourceAdapter::new(
        config("fake", PathBuf::from("/space")),
        Box::new(transport),
        vec![Box::new(MimeParser { mime: "text/fake", fail: false })],
    );
    let scanned = adapter.scan().unwrap();
    assert_eq!(scanned.source_id, "fake");
    assert_eq!(scanned.resources.len(), 2);
    assert_eq!(scanned.relations.len(), 0);
    assert_eq!(scanned.link_occurrences.len(), 0);

    let prep = adapter.prepare_write("a.fake", "new payload").unwrap();
    assert_eq!(prep.target_ref, "a.fake");
    assert_eq!(prep.payload, "new payload");
    assert!(prep.ready);
    let result = adapter.commit_write(&prep).unwrap();
    assert_eq!(result.target_ref, "a.fake");
    assert!(result.committed);
}

#[test]
fn composed_source_adapter_reports_unknown_mime_and_parser_errors() {
    let unknown = ComposedSourceAdapter::new(
        config("fake", PathBuf::from("/space")),
        Box::new(ListTransport { entities: vec![RawEntity {
            locator: "unknown.bin".into(),
            mime_type: "application/unknown".into(),
            payload: vec![],
        }], mutation: Ok(()) }),
        vec![],
    );
    let error = unknown.scan().unwrap_err();
    assert!(matches!(&error, SourceError::ParserNotFound { source_id, mime, locator } if source_id == "fake" && mime == "application/unknown" && locator == "unknown.bin"));
    assert!(error.to_string().contains("no format parser"));

    let parser_error = ComposedSourceAdapter::new(
        config("fake", PathBuf::from("/space")),
        Box::new(ListTransport { entities: vec![RawEntity {
            locator: "bad.fake".into(),
            mime_type: "text/fake".into(),
            payload: vec![],
        }], mutation: Ok(()) }),
        vec![Box::new(MimeParser { mime: "text/fake", fail: true })],
    );
    let error = parser_error.scan().unwrap_err();
    assert!(matches!(error, SourceError::Parse(msg) if msg.contains("parser exploded")));
}

#[test]
fn composed_source_adapter_surfaces_transport_errors_and_unsupported_write_errors() {
    struct FailingTransport;
    impl SourceTransport for FailingTransport {
        fn fetch_raw(&self) -> Result<Vec<RawEntity>, TransportError> {
            Err(TransportError::Other("transport down".into()))
        }
    }
    let adapter = ComposedSourceAdapter::new(config("bad", PathBuf::from("/space")), Box::new(FailingTransport), vec![]);
    let error = adapter.scan().unwrap_err();
    assert!(matches!(&error, SourceError::Other(msg) if msg.contains("transport down")));
    assert!(error.to_string().contains("Source error"));

    let adapter = ComposedSourceAdapter::new(
        config("read-only", PathBuf::from("/space")),
        Box::new(ListTransport { entities: vec![], mutation: Err("denied".into()) }),
        vec![],
    );
    let prep = adapter.prepare_write("x", "payload").unwrap();
    let error = adapter.commit_write(&prep).unwrap_err();
    assert!(matches!(error, SourceError::Other(msg) if msg.contains("denied")));
}

#[test]
fn native_source_adapter_scans_supported_files_deterministically_and_skips_hidden() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".hidden")).unwrap();
    std::fs::write(dir.path().join("b.md"), "# B").unwrap();
    std::fs::write(dir.path().join("a.org"), "#+title: A\n#+ID: 01J00000000000000000000011\n").unwrap();
    std::fs::write(dir.path().join("ignored.txt"), "ignored").unwrap();
    std::fs::write(dir.path().join(".hidden/secret.md"), "# Secret").unwrap();

    let mut cfg = config("native", dir.path().to_path_buf());
    cfg.include_paths = vec![dir.path().to_path_buf()];
    let adapter = NativeSourceAdapter::new(cfg);
    assert_eq!(adapter.config().id, "native");
    let scanned = adapter.scan().unwrap();
    assert_eq!(scanned.source_id, "native");
    assert_eq!(scanned.resources.len(), 3);
    let locators: Vec<_> = scanned.resources.iter().map(|r| r.locator.clone()).collect();
    assert!(locators[0] < locators[1]);
    assert!(locators.iter().all(|locator| !locator.contains("secret")));
}

#[test]
fn source_error_and_transport_error_display_contents_are_stable() {
    let io_error = SourceError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "missing"));
    assert!(io_error.to_string().contains("I/O error: missing"));
    let parser = SourceError::Parse("bad format".into());
    assert_eq!(parser.to_string(), "format parser error: bad format");
    let other = SourceError::Other("custom".into());
    assert_eq!(other.to_string(), "Source error: custom");
    let transport = TransportError::Other("offline".into());
    assert_eq!(transport.to_string(), "Transport error: offline");
}

#[test]
fn source_kind_round_trip_covers_all_registry_variants() {
    let variants = [
        SourceKind::Native,
        SourceKind::Git,
        SourceKind::Obsidian,
        SourceKind::Anytype,
        SourceKind::AppleNotes,
        SourceKind::AppleCalendar,
    ];
    for kind in variants {
        let encoded = serde_json::to_string(&kind).unwrap();
        assert_eq!(serde_json::from_str::<SourceKind>(&encoded).unwrap(), kind);
    }
}
