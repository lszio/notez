use notez_core::application::{ApplicationError, ApplicationService, ResolveResult, SpaceContext};
use notez_core::config::RuntimeConfig;
use notez_core::domain::community::Community;
use notez_core::domain::schema::{
    PropertyType, SchemaField, Trait, TypeDefinition, TypeRegistry, ValidationError,
};
use notez_core::domain::{
    derived_id, LinkOccurrence, LinkTarget, Projection, ProjectionStore, QueryPage,
    RelationDirection, RelationType, Resource, ResourceAddress, ResourceKind, ResourceRef,
    ResourceRefError, ResourceRelation, RuleEngine, Selector,
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
        object_id: notez_core::domain::ObjectId::default(),
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

impl ProjectionStore for FailingStore {
    type Error = ContractFailure;

    fn replace_source(
        &mut self,
        _source_id: &str,
        _resources: Vec<Resource>,
        _relations: Vec<ResourceRelation>,
        _link_occurrences: Vec<LinkOccurrence>,
    ) -> Result<(), Self::Error> {
        Err(ContractFailure)
    }

    fn get(&self, _r_ref: &ResourceRef) -> Result<Option<Resource>, Self::Error> {
        Err(ContractFailure)
    }

    fn query(&self, _selector: &Selector) -> Result<QueryPage, Self::Error> {
        Err(ContractFailure)
    }

    fn upsert_resource(&mut self, _resource: &Resource) -> Result<(), Self::Error> {
        Err(ContractFailure)
    }

    fn delete_resource(&mut self, _r_ref: &ResourceRef) -> Result<(), Self::Error> {
        Err(ContractFailure)
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        Err(ContractFailure)
    }
}

struct MarkerParser;

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
    assert_eq!(ResourceRef::parse("missing-colon"), Err(ResourceRefError::InvalidFormat));
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
    assert_eq!(serde_json::from_str::<ResourceRelation>(&json).unwrap(), relation);

    let segment = notez_core::domain::SegmentRecord {
        id: "segment-1".into(),
        attachment_ref: source_ref.to_string(),
        text: "hello".into(),
        offset_start: 0,
        offset_end: 5,
    };
    assert_eq!(serde_json::from_value::<notez_core::domain::SegmentRecord>(
        serde_json::to_value(&segment).unwrap()
    ).unwrap(), segment);

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
    assert_eq!(id.as_resource_ref(), Some(rref(ResourceKind::Document, DOC_ID)));
    assert_eq!(LinkTarget::file("notes.org", Some("Intro".into())).to_string(), "file:notes.org::Intro");
    assert_eq!(LinkTarget::title("Design", Some("API".into())).to_string(), "[[Design#API]]");
    assert_eq!(LinkTarget::url("https://example.test").to_string(), "https://example.test");
    assert_eq!(LinkTarget::custom("zotero", "42", None).to_string(), "zotero:42");

    assert!(matches!(
        ResourceAddress::parse(&format!("document:{DOC_ID}")).unwrap(),
        ResourceAddress::Ref { .. }
    ));
    assert!(matches!(
        ResourceAddress::parse("file:notes.org::Intro").unwrap(),
        ResourceAddress::Locator { target: LinkTarget::File { .. } }
    ));
    assert!(matches!(
        ResourceAddress::from_locator(LinkTarget::title("Design", None)),
        ResourceAddress::Locator { .. }
    ));
    assert_eq!(ResourceAddress::parse("   "), Err(ResourceRefError::InvalidFormat));
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
    assert_eq!(registry.get("project").unwrap().traits, vec![Trait::Taskable, Trait::ParaItem]);

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
        Err(ValidationError::MissingRequiredField { field: "OWNER".into() })
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
        Err(ValidationError::UnknownType { name: "unknown".into() })
    );
}

#[test]
fn rule_engine_reports_matching_and_non_matching_traces() {
    let engine = RuleEngine::default_rules();
    let mut project = resource(ResourceKind::Heading, HEADING_ID, "Project", "native", "1");
    project.properties.insert("TYPE".into(), "project".into());
    let matching = engine.evaluate(&project);
    assert_eq!(matching.classified_type.as_deref(), Some("project"));
    assert_eq!(matching.derived_properties.get("para").map(String::as_str), Some("projects"));
    assert!(matching.traces.iter().all(|trace| trace.matched));

    let plain = resource(ResourceKind::Heading, TASK_ID, "Plain", "native", "1");
    let non_matching = engine.evaluate(&plain);
    assert_eq!(non_matching.classified_type, None);
    assert!(non_matching.traces.iter().all(|trace| !trace.matched));
}

#[test]
fn community_filter_honors_selector_pins_and_exclusions() {
    let project = resource(ResourceKind::Heading, HEADING_ID, "Project Alpha", "native", "1");
    let pinned = resource(ResourceKind::Document, DOC_ID, "Pinned", "native", "1");
    let excluded = resource(ResourceKind::Heading, TASK_ID, "Project Hidden", "native", "1");
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
fn space_context_is_explicit_and_service_exposes_bound_or_unbound_state() {
    let runtime = RuntimeConfig {
        space_root: PathBuf::from("/tmp/space"),
        space_name: "personal".into(),
        database: PathBuf::from("/tmp/space/.notez/index.sqlite"),
        ..RuntimeConfig::default()
    };
    let context = SpaceContext::new("space-1", PathBuf::from("/tmp/space"), runtime.clone());
    let service = ApplicationService::with_space(SqliteProjection::in_memory().unwrap(), context);
    let bound = service.space().unwrap();
    assert_eq!(bound.space_id, "space-1");
    assert_eq!(bound.root, PathBuf::from("/tmp/space"));
    assert_eq!(bound.runtime, runtime);

    let unbound = ApplicationService::new(SqliteProjection::in_memory().unwrap());
    assert!(unbound.space().is_none());
}

#[test]
fn application_scan_query_read_and_resolve_cover_success_and_not_found() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("notes.org"),
        format!("#+title: Notes\n#+ID: {DOC_ID}\n\n* Design API\n:PROPERTIES:\n:ID: {HEADING_ID}\n:END:\n"),
    )
    .unwrap();
    let mut service = ApplicationService::new(SqliteProjection::in_memory().unwrap());
    service.register_format_parser(Box::new(MarkerParser));

    let report = service.scan_native(dir.path()).unwrap();
    assert_eq!(report.scanned_files, 1);
    assert_eq!(report.scanned_resources, 2);

    let page = service
        .query(&Selector::kind(ResourceKind::Heading).with_title_contains("design"))
        .unwrap();
    assert_eq!(page.items.len(), 1);
    let heading_ref = rref(ResourceKind::Heading, HEADING_ID);
    assert_eq!(service.read(&heading_ref).unwrap().unwrap().title, "Design API");
    assert_eq!(service.resolve("Design API").unwrap(), ResolveResult::Found(heading_ref));
    assert_eq!(service.resolve("missing title").unwrap(), ResolveResult::NotFound);
    assert_eq!(
        service.resolve_address(&ResourceAddress::from_ref(heading_ref)).unwrap(),
        ResolveResult::Found(heading_ref)
    );
    assert_eq!(
        service
            .resolve_address(&ResourceAddress::from_locator(LinkTarget::url("https://example.test")))
            .unwrap(),
        ResolveResult::NotFound
    );
    assert!(service.read(&rref(ResourceKind::Heading, TASK_ID)).unwrap().is_none());
}

#[test]
fn application_scan_rejects_missing_registry_and_storage_errors_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let mut unregistered = ApplicationService::new(SqliteProjection::in_memory().unwrap());
    let err = unregistered.scan_native(dir.path()).unwrap_err();
    assert!(matches!(err, ApplicationError::Storage { kind: _, ref message } if message.contains("no format parsers")));

    let service = ApplicationService::new(FailingStore);
    for err in [
        service.query(&Selector::new()).unwrap_err(),
        service.read(&rref(ResourceKind::Document, DOC_ID)).unwrap_err(),
        service.resolve("anything").unwrap_err(),
        service
            .resolve_address(&ResourceAddress::from_ref(rref(ResourceKind::Document, DOC_ID)))
            .unwrap_err(),
    ] {
        assert!(matches!(err, ApplicationError::Storage { kind: _, ref message } if message == "contract failure"));
        assert_eq!(err.to_string(), "storage error (sqlite): contract failure");
    }
}

#[test]
fn application_mutation_listing_inspection_agenda_and_para_happy_paths() {
    let mut service = ApplicationService::new(SqliteProjection::in_memory().unwrap());
    let mut project = resource(ResourceKind::Heading, HEADING_ID, "Project", "native", "2026-01-01");
    project.properties.insert("TYPE".into(), "project".into());
    let mut task = resource(ResourceKind::Heading, TASK_ID, "Ship API", "git", "2026-02-01");
    task.properties.extend([
        ("TODO".into(), "NEXT".into()),
        ("SCHEDULED".into(), "2026-08-02".into()),
        ("PARENT_REF".into(), project.r#ref.to_string()),
    ]);

    service.upsert_resource(project.clone()).unwrap();
    service.upsert_resource(task.clone()).unwrap();
    assert_eq!(service.store().get(&project.r#ref).unwrap(), Some(project.clone()));
    assert!(service.store_mut().get(&task.r#ref).unwrap().is_some());

    let recent = service.list_recent(1).unwrap();
    assert_eq!(recent, vec![task.clone()]);
    assert_eq!(service.list_by_source("native", 10).unwrap(), vec![project.clone()]);
    assert!(service.list_by_source("missing", 10).unwrap().is_empty());

    let inspect = service.inspect_rules(&project.r#ref).unwrap().unwrap();
    assert_eq!(inspect.derived_properties.get("para").map(String::as_str), Some("projects"));
    assert!(service.inspect_rules(&rref(ResourceKind::Document, DOC_ID)).unwrap().is_none());

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
    let mut service = ApplicationService::new(FailingStore);
    let doc = resource(ResourceKind::Document, DOC_ID, "Doc", "native", "1");
    for err in [
        service.upsert_resource(doc.clone()).unwrap_err(),
        service.delete_resource(&doc.r#ref).unwrap_err(),
    ] {
        assert!(matches!(err, ApplicationError::Storage { kind: _, ref message } if message == "contract failure"));
    }

    let service = ApplicationService::new(FailingStore);
    for err in [
        service.list_recent(5).unwrap_err(),
        service.list_by_source("native", 5).unwrap_err(),
        service.inspect_rules(&doc.r#ref).unwrap_err(),
        service.agenda().unwrap_err(),
        service.para_overview().unwrap_err(),
    ] {
        assert!(matches!(err, ApplicationError::Storage { kind: _, ref message } if message == "contract failure"));
    }
}

#[test]
fn application_error_variants_expose_not_found_and_unsupported_content() {
    let mut service = ApplicationService::new(SqliteProjection::in_memory().unwrap());
    let missing = rref(ResourceKind::Attachment, DOC_ID);
    let err = service
        .run_extraction(PathBuf::from("/tmp/space").as_path(), &missing)
        .unwrap_err();
    assert!(matches!(err, ApplicationError::NotFound { r_ref, .. } if r_ref == missing));
    assert_eq!(
        err.to_string(),
        format!("resource not found: {missing} (kind=attachment)")
    );

    let err = service.list_conflicts().unwrap_err();
    assert!(matches!(err, ApplicationError::UnsupportedCapability { capability } if capability.contains("conflict list")));
    assert!(err.to_string().contains("unsupported capability"));
}
