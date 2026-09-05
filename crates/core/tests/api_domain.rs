use notez_core::application::{ApplicationError, Engine, ResolveResult, SourceContext};
use notez_core::config::SourceConfig;
use notez_core::domain::community::Community;
use notez_core::domain::schema::{
    PropertyType, SchemaField, Trait, TypeDefinition, TypeRegistry, ValidationError,
};
use notez_core::domain::{
    LinkOccurrence, LinkTarget, Projection, ProjectionStore, QueryPage, RelationDirection,
    RelationType, Resource, ResourceAddress, ResourceKind, ResourceRef, ResourceRefError,
    ResourceRelation, RuleEngine, Selector, TextSpan, derived_id,
};
use notez_core::source::{FormatParser, ParsedEntity, ParserError, RawEntity};
use notez_core::storage::SqliteProjection;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use ulid::Ulid;

const DOC_ID: &str = "01J00000000000000000000001";
const HEADING_ID: &str = "01J00000000000000000000002";
const TASK_ID: &str = "01J00000000000000000000003";

fn rref(kind: ResourceKind, id: &str) -> ResourceRef {
    ResourceRef::parse(&format!("{}:{id}", kind.as_str())).unwrap()
}

fn resource(
    kind: ResourceKind,
    id: &str,
    title: &str,
    source_id: &str,
    revision: &str,
) -> Resource {
    Resource {
        r#ref: rref(kind, id),
        kind,
        title: title.into(),
        revision: revision.into(),
        source_id: source_id.into(),
        locator: format!("{source_id}/{title}"),
        properties: BTreeMap::new(),
        object_id: notez_core::domain::ObjectIdentity::default(),
        primary_source_id: String::new(),
    }
}

#[derive(Debug)]
struct ContractFailure;

impl fmt::Display for ContractFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("contract failure")
    }
}

impl Error for ContractFailure {}

struct FailingStore;

impl notez_core::domain::ProjectionReader for FailingStore {
    type Error = notez_core::storage::StorageError;

    fn get(&self, _r_ref: &ResourceRef) -> Result<Option<Resource>, Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

    fn query(&self, _selector: &Selector) -> Result<QueryPage, Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

    fn list_conflicts(&self) -> Result<Vec<notez_core::domain::ConflictRecord>, Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

    fn query_segments(
        &self,
        _attachment_ref: &str,
    ) -> Result<Vec<notez_core::domain::SegmentRecord>, Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

    fn query_link_occurrences(
        &self,
        _source_ref: &ResourceRef,
    ) -> Result<Vec<LinkOccurrence>, Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

    fn query_resolved_relations(
        &self,
        _source_ref: &ResourceRef,
    ) -> Result<Vec<notez_core::domain::ResolvedRelation>, Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

    fn list_link_diagnostics(
        &self,
        _source_ref: &ResourceRef,
    ) -> Result<Option<Vec<notez_core::domain::LinkDiagnostic>>, Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

    fn find_by_object(
        &self,
        _object_id: &notez_core::domain::ObjectIdentity,
    ) -> Result<Vec<Resource>, Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

}

impl notez_core::domain::ProjectionWrite for FailingStore {
    type Error = notez_core::storage::StorageError;

    fn replace_source(
        &mut self,
        _source_id: &str,
        _resources: Vec<Resource>,
        _relations: Vec<ResourceRelation>,
        _link_occurrences: Vec<LinkOccurrence>,
    ) -> Result<(), Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

    fn upsert_resource(&mut self, _resource: &Resource) -> Result<(), Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

    fn delete_resource(&mut self, _r_ref: &ResourceRef) -> Result<(), Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

    fn replace_conflicts(
        &mut self,
        _records: &[notez_core::domain::ConflictRecord],
    ) -> Result<(), Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }
    fn remove_conflicts(&mut self, _logical_paths: &[String]) -> Result<(), Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

    fn insert_segments(
        &mut self,
        _segments: &[notez_core::domain::SegmentRecord],
    ) -> Result<(), Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

    fn replace_link_occurrences(
        &mut self,
        _source_id: &str,
        _occurrences: Vec<LinkOccurrence>,
    ) -> Result<(), Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

    fn replace_resolved_relations(
        &mut self,
        _source_id: &str,
        _relations: Vec<notez_core::domain::ResolvedRelation>,
    ) -> Result<(), Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }

    fn write_link_diagnostics(
        &mut self,
        _source_id: &str,
        _diagnostics: &[(
            LinkOccurrence,
            notez_core::domain::ResolutionStatus,
            Vec<ResourceRef>,
        )],
    ) -> Result<(), Self::Error> {
        Err(notez_core::storage::StorageError::InvalidData("contract failure".into()))
    }
}

struct MarkerParser;
impl notez_core::domain::ProjectionStore for FailingStore {}

impl FormatParser for MarkerParser {
    fn supports(&self, _mime_type: &str) -> bool {
        false
    }

    fn parse(&self, _entity: &RawEntity, _source_id: &str) -> Result<ParsedEntity, ParserError> {
        Err(ParserError::Other("marker parser is never selected".into()))
    }
}

#[test]
fn resource_kinds_refs_and_derived_ids_have_stable_happy_contracts() {
    assert_eq!(ResourceKind::Document.as_str(), "document");
    assert_eq!(ResourceKind::Heading.to_string(), "heading");

    let parsed = rref(ResourceKind::Document, DOC_ID);
    assert_eq!(parsed.kind(), ResourceKind::Document);
    assert_eq!(parsed.id(), Ulid::from_string(DOC_ID).unwrap());
    assert_eq!(ResourceRef::new(parsed.kind(), parsed.id()), parsed);
    assert_eq!(parsed.to_string(), format!("document:{DOC_ID}"));

    let a = derived_id(ResourceKind::Block, "native", "notes.org", "heading/0");
    let b = derived_id(ResourceKind::Block, "native", "notes.org", "heading/0");
    let other = derived_id(ResourceKind::Block, "native", "notes.org", "heading/1");
    assert_eq!(a, b);
    assert_ne!(a, other);
}

#[test]
fn resource_ref_and_resource_deserialization_reject_invalid_input() {
    assert_eq!(
        ResourceRef::parse("missing-colon"),
        Err(ResourceRefError::InvalidFormat)
    );
    assert!(matches!(
        ResourceRef::parse(&format!("unknown:{DOC_ID}")),
        Err(ResourceRefError::UnknownKind(kind)) if kind == "unknown"
    ));
    assert!(matches!(
        ResourceRef::parse("document:not-a-ulid"),
        Err(ResourceRefError::InvalidUlid(_))
    ));

    let err = serde_json::from_str::<Resource>(
        r#"{"ref":"document:bad","kind":"document","title":"x","revision":"1","source_id":"native","locator":"x","properties":{}}"#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("invalid ULID"));
}

#[test]
fn resource_relation_segment_and_projection_domain_types_round_trip() {
    let source_ref = rref(ResourceKind::Document, DOC_ID);
    let target_ref = rref(ResourceKind::Heading, HEADING_ID);
    let relation = ResourceRelation {
        source_ref,
        relation: "contains".into(),
        target_ref,
        relation_type: RelationType::References,
        direction: RelationDirection::Unknown,
        evidence_json: serde_json::json!({}),
        created_at: String::new(),
        creator: "scan".to_string(),
    };
    let json = serde_json::to_string(&relation).unwrap();
    assert_eq!(
        serde_json::from_str::<ResourceRelation>(&json).unwrap(),
        relation
    );

    let segment = notez_core::domain::SegmentRecord {
        id: "segment-1".into(),
        attachment_ref: source_ref.to_string(),
        text: "hello".into(),
        offset_start: 0,
        offset_end: 5,
    };
    assert_eq!(
        serde_json::from_value::<notez_core::domain::SegmentRecord>(
            serde_json::to_value(&segment).unwrap()
        )
        .unwrap(),
        segment
    );

    assert_eq!(Projection::summary().fields, ["ref", "title", "revision"]);
    assert!(serde_json::from_str::<ResourceRelation>("{}").is_err());
}

#[test]
fn selector_builders_select_and_invalid_json_is_rejected() {
    let selected_ref = rref(ResourceKind::Heading, HEADING_ID);
    let selector = Selector::kind(ResourceKind::Heading)
        .with_title_contains("Design")
        .with_exact_ref(selected_ref)
        .with_source("native");
    assert_eq!(selector.kind, Some(ResourceKind::Heading));
    assert_eq!(selector.title_contains.as_deref(), Some("Design"));
    assert_eq!(selector.exact_refs, vec![selected_ref]);
    assert_eq!(selector.source_id.as_deref(), Some("native"));
    assert!(serde_json::from_str::<Selector>(r#"{"kind":"bogus"}"#).is_err());
}

#[test]
fn link_targets_and_addresses_cover_locators_refs_and_parse_errors() {
    let id = LinkTarget::id(DOC_ID, Some(ResourceKind::Document));
    assert_eq!(
        id.as_resource_ref(),
        Some(rref(ResourceKind::Document, DOC_ID))
    );
    assert_eq!(
        LinkTarget::file("notes.org", Some("Intro".into())).to_string(),
        "file:notes.org::Intro"
    );
    assert_eq!(
        LinkTarget::title("Design", Some("API".into())).to_string(),
        "[[Design#API]]"
    );
    assert_eq!(
        LinkTarget::url("https://example.test").to_string(),
        "https://example.test"
    );
    assert_eq!(
        LinkTarget::custom("zotero", "42", None).to_string(),
        "zotero:42"
    );

    assert!(matches!(
        ResourceAddress::parse(&format!("document:{DOC_ID}")).unwrap(),
        ResourceAddress::Ref { .. }
    ));
    assert!(matches!(
        ResourceAddress::parse("file:notes.org::Intro").unwrap(),
        ResourceAddress::Locator {
            target: LinkTarget::File { .. }
        }
    ));
    assert!(matches!(
        ResourceAddress::from_locator(LinkTarget::title("Design", None)),
        ResourceAddress::Locator { .. }
    ));
    assert_eq!(
        ResourceAddress::parse("   "),
        Err(ResourceRefError::InvalidFormat)
    );
}

#[test]
fn type_registry_validates_registered_types_and_reports_schema_errors() {
    let mut registry = TypeRegistry::new();
    registry.register(TypeDefinition {
        name: "project".into(),
        traits: vec![Trait::Taskable, Trait::ParaItem],
        fields: BTreeMap::from([
            (
                "OWNER".into(),
                SchemaField {
                    property_type: PropertyType::String,
                    required: true,
                    target_types: vec![],
                },
            ),
            (
                "ESTIMATE".into(),
                SchemaField {
                    property_type: PropertyType::Duration,
                    required: false,
                    target_types: vec![],
                },
            ),
        ]),
    });
    assert_eq!(
        registry.get("project").unwrap().traits,
        vec![Trait::Taskable, Trait::ParaItem]
    );

    let mut valid = resource(ResourceKind::Heading, HEADING_ID, "Project", "native", "1");
    valid.properties.extend([
        ("TYPE".into(), "project".into()),
        ("OWNER".into(), "Ada".into()),
        ("ESTIMATE".into(), "2h".into()),
    ]);
    assert_eq!(registry.validate(&valid), Ok(()));

    valid.properties.remove("OWNER");
    assert_eq!(
        registry.validate(&valid),
        Err(ValidationError::MissingRequiredField {
            field: "OWNER".into()
        })
    );
    valid.properties.insert("OWNER".into(), "Ada".into());
    valid.properties.insert("ESTIMATE".into(), "soon".into());
    assert_eq!(
        registry.validate(&valid),
        Err(ValidationError::InvalidPropertyType {
            field: "ESTIMATE".into(),
            expected: "duration".into(),
        })
    );
    valid.properties.insert("TYPE".into(), "unknown".into());
    assert_eq!(
        registry.validate(&valid),
        Err(ValidationError::UnknownType {
            name: "unknown".into()
        })
    );
}

#[test]
fn rule_engine_reports_matching_and_non_matching_traces() {
    let engine = RuleEngine::default_rules();
    let mut project = resource(ResourceKind::Heading, HEADING_ID, "Project", "native", "1");
    project.properties.insert("TYPE".into(), "project".into());
    let matching = engine.evaluate(&project);
    assert_eq!(matching.classified_type.as_deref(), Some("project"));
    assert_eq!(
        matching.derived_properties.get("para").map(String::as_str),
        Some("projects")
    );
    assert!(matching.traces.iter().all(|trace| trace.matched));

    let plain = resource(ResourceKind::Heading, TASK_ID, "Plain", "native", "1");
    let non_matching = engine.evaluate(&plain);
    assert_eq!(non_matching.classified_type, None);
    assert!(non_matching.traces.iter().all(|trace| !trace.matched));
}

#[test]
fn community_filter_honors_selector_pins_and_exclusions() {
    let project = resource(
        ResourceKind::Heading,
        HEADING_ID,
        "Project Alpha",
        "native",
        "1",
    );
    let pinned = resource(ResourceKind::Document, DOC_ID, "Pinned", "native", "1");
    let excluded = resource(
        ResourceKind::Heading,
        TASK_ID,
        "Project Hidden",
        "native",
        "1",
    );
    let community = Community {
        id: "projects".into(),
        name: "Projects".into(),
        selector: Selector::kind(ResourceKind::Heading).with_title_contains("Project"),
        pinned_members: vec![pinned.r#ref],
        excluded_members: vec![excluded.r#ref],
    };
    let candidates = vec![project.clone(), pinned.clone(), excluded];
    let members = community.filter_members(&candidates);
    assert_eq!(members, vec![&project, &pinned]);

    let empty = Community {
        pinned_members: vec![],
        excluded_members: candidates.iter().map(|candidate| candidate.r#ref).collect(),
        ..community
    };
    assert!(empty.filter_members(&candidates).is_empty());
}

#[test]
fn source_context_is_explicit_and_service_exposes_bound_or_unbound_state() {
    let cfg = SourceConfig {
        version: notez_core::config::model::CURRENT_VERSION,
        source: notez_core::config::model::SourceIdentity {
            name: "personal".into(),
            database: PathBuf::from("/tmp/source/.notez/index.sqlite"),
        },
        workflow: Default::default(),
        sources: Vec::new(),
        link_overrides: serde_json::Value::Null,
        scan: Default::default(),
    };
    let context = SourceContext::new("source-1", PathBuf::from("/tmp/source"), cfg);
    let service = Engine::with_source(SqliteProjection::in_memory().unwrap(), context);
    let bound = service.source().unwrap();
    assert_eq!(bound.source_id, "source-1");
    assert_eq!(bound.root, PathBuf::from("/tmp/source"));
    assert_eq!(bound.config.source.name, "personal");
    let unbound = Engine::new(SqliteProjection::in_memory().unwrap());
    assert!(unbound.source().is_none());
}

#[test]
fn application_scan_query_read_and_resolve_cover_success_and_not_found() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("notes.org"),
        format!("#+title: Notes\n#+ID: {DOC_ID}\n\n* Design API\n:PROPERTIES:\n:ID: {HEADING_ID}\n:END:\n"),
    )
    .unwrap();
    let config = notez_core::config::model::SourceConfig {
        version: 2,
        source: notez_core::config::model::SourceIdentity {
            name: "test".into(),
            database: std::path::PathBuf::from(".notez/index.sqlite"),
        },
        workflow: Default::default(),
        sources: vec![],
        link_overrides: serde_json::Value::Null,
        scan: Default::default(),
    };
    let ctx = SourceContext::new("test", dir.path().to_path_buf(), config);
    let mut service = Engine::with_source(SqliteProjection::in_memory().unwrap(), ctx);
    service.register_format_parser(Box::new(MarkerParser));

    let report = service.scan_native().unwrap();
    assert_eq!(report.scanned_files, 1);
    assert_eq!(report.scanned_resources, 2);

    let page = service
        .query(&Selector::kind(ResourceKind::Heading).with_title_contains("design"))
        .unwrap();
    assert_eq!(page.items.len(), 1);
    let heading_ref = rref(ResourceKind::Heading, HEADING_ID);
    assert_eq!(
        service.read(&heading_ref).unwrap().unwrap().title,
        "Design API"
    );
    assert_eq!(
        service.resolve("Design API").unwrap(),
        ResolveResult::Found(heading_ref)
    );
    assert_eq!(
        service.resolve("missing title").unwrap(),
        ResolveResult::NotFound
    );
    assert_eq!(
        service
            .resolve_address(&ResourceAddress::from_ref(heading_ref))
            .unwrap(),
        ResolveResult::Found(heading_ref)
    );
    assert_eq!(
        service
            .resolve_address(&ResourceAddress::from_locator(LinkTarget::url(
                "https://example.test"
            )))
            .unwrap(),
        ResolveResult::NotFound
    );
    assert!(
        service
            .read(&rref(ResourceKind::Heading, TASK_ID))
            .unwrap()
            .is_none()
    );
}

#[test]
fn application_scan_rejects_missing_registry_and_storage_errors_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let config = notez_core::config::model::SourceConfig {
        version: 2,
        source: notez_core::config::model::SourceIdentity {
            name: "test".into(),
            database: std::path::PathBuf::from(".notez/index.sqlite"),
        },
        workflow: Default::default(),
        sources: vec![],
        link_overrides: serde_json::Value::Null,
        scan: Default::default(),
    };
    let ctx = SourceContext::new("test", dir.path().to_path_buf(), config);
    let mut unregistered =
        Engine::with_source(SqliteProjection::in_memory().unwrap(), ctx);
    let err = unregistered.scan_native().unwrap_err();
    assert!(
        matches!(err, ApplicationError::Storage { kind: _, ref message } if message.contains("no format parsers"))
    );

    let service = Engine::new(FailingStore);
    for err in [
        service.query(&Selector::new()).unwrap_err(),
        service
            .read(&rref(ResourceKind::Document, DOC_ID))
            .unwrap_err(),
        service.resolve("anything").unwrap_err(),
        service
            .resolve_address(&ResourceAddress::from_ref(rref(
                ResourceKind::Document,
                DOC_ID,
            )))
            .unwrap_err(),
    ] {
        assert!(
            matches!(err, ApplicationError::Storage { kind: _, ref message } if message.contains("contract failure"))
        );
        assert!(err.to_string().contains("contract failure"), "{err}");
    }
}

#[test]
fn application_mutation_listing_inspection_agenda_and_para_happy_paths() {
    let mut service = Engine::new(SqliteProjection::in_memory().unwrap());
    let mut project = resource(
        ResourceKind::Heading,
        HEADING_ID,
        "Project",
        "native",
        "2026-01-01",
    );
    project.properties.insert("TYPE".into(), "project".into());
    let mut task = resource(
        ResourceKind::Heading,
        TASK_ID,
        "Ship API",
        "git",
        "2026-02-01",
    );
    task.properties.extend([
        ("TODO".into(), "NEXT".into()),
        ("SCHEDULED".into(), "2026-08-02".into()),
        ("PARENT_REF".into(), project.r#ref.to_string()),
    ]);

    service.upsert_resource(project.clone()).unwrap();
    service.upsert_resource(task.clone()).unwrap();
    // `upsert_resource` materializes the implicit "self is primary" rule on
    // persist: a Resource whose `primary_source_id` is empty is stored as
    // `primary_source_id == source_id`, which is what the round trip returns.
    let mut expected = project.clone();
    expected.primary_source_id = "native".to_string();
    assert_eq!(service.read(&project.r#ref).unwrap(), Some(expected));

    let recent = service.list_recent(1).unwrap();
    let mut recent_task = task.clone();
    recent_task.primary_source_id = task.source_id.clone();
    assert_eq!(recent, vec![recent_task]);
    let mut expected_project = project.clone();
    expected_project.primary_source_id = project.source_id.clone();
    assert_eq!(
        service.list_by_source("native", 10).unwrap(),
        vec![expected_project]
    );
    assert!(service.list_by_source("missing", 10).unwrap().is_empty());

    let inspect = service.inspect_rules(&project.r#ref).unwrap().unwrap();
    assert_eq!(
        inspect.derived_properties.get("para").map(String::as_str),
        Some("projects")
    );
    assert!(
        service
            .inspect_rules(&rref(ResourceKind::Document, DOC_ID))
            .unwrap()
            .is_none()
    );

    let agenda = service.agenda().unwrap();
    assert_eq!(agenda.items.len(), 1);
    assert_eq!(agenda.items[0].title, "Ship API");
    let para = service.para_overview().unwrap();
    assert_eq!(para.projects.len(), 1);
    assert_eq!(para.projects[0].tasks.len(), 1);

    service.delete_resource(&task.r#ref).unwrap();
    service.delete_resource(&task.r#ref).unwrap();
    assert!(service.read(&task.r#ref).unwrap().is_none());
}

#[test]
fn application_main_paths_surface_storage_errors() {
    let mut service = Engine::new(FailingStore);
    let doc = resource(ResourceKind::Document, DOC_ID, "Doc", "native", "1");
    for err in [
        service.upsert_resource(doc.clone()).unwrap_err(),
        service.delete_resource(&doc.r#ref).unwrap_err(),
    ] {
        assert!(
            matches!(err, ApplicationError::Storage { kind: _, ref message } if message.contains("contract failure"))
        );
    }

    let service = Engine::new(FailingStore);
    for err in [
        service.list_recent(5).unwrap_err(),
        service.list_by_source("native", 5).unwrap_err(),
        service.inspect_rules(&doc.r#ref).unwrap_err(),
        service.agenda().unwrap_err(),
        service.para_overview().unwrap_err(),
    ] {
        assert!(
            matches!(err, ApplicationError::Storage { kind: _, ref message } if message.contains("contract failure"))
        );
    }
}

#[test]
fn application_error_variants_expose_not_found_and_unsupported_content() {
    let config = notez_core::config::model::SourceConfig {
        version: 2,
        source: notez_core::config::model::SourceIdentity {
            name: "test".into(),
            database: std::path::PathBuf::from(".notez/index.sqlite"),
        },
        workflow: Default::default(),
        sources: vec![],
        link_overrides: serde_json::Value::Null,
        scan: Default::default(),
    };
    let ctx = SourceContext::new("test", PathBuf::from("/tmp/space"), config);
    let mut service = Engine::with_source(SqliteProjection::in_memory().unwrap(), ctx);
    let missing = rref(ResourceKind::Attachment, DOC_ID);
    let err = service.run_extraction(&missing).unwrap_err();
    assert!(matches!(err, ApplicationError::NotFound { r_ref, .. } if r_ref == missing));
    assert_eq!(
        err.to_string(),
        format!("resource not found: {missing} (kind=attachment)")
    );

    // `list_conflicts` now reads the persisted conflict table (returns an
    // empty list on a fresh projection); the unsupported-capability shape
    // is pinned by `relay_sync` instead.
    let err = service.relay_sync().unwrap_err();
    assert!(
        matches!(err, ApplicationError::UnsupportedCapability { capability } if capability.contains("not yet implemented"))
    );
    assert!(err.to_string().contains("unsupported capability"));

    let conflicts = service.list_conflicts().unwrap();
    assert!(conflicts.is_empty());
}
