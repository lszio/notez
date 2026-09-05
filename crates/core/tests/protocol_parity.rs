//! Cross-surface protocol parity contracts.
//!
//! CLI and Web are intentionally thin translators: these tests exercise the
//! shared typed `Request` -> `ApplicationDispatcher` -> typed `Response`
//! contract they both consume, rather than duplicating adapter rendering.

use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_core::application::{ApplicationError, Engine};
use notez_core::config::model::{SourceConfig, SourceIdentity};
use notez_core::storage::SqliteProjection;
use notez_protocol::request::{
    ListBySourceRequest, QueryResourcesRequest, ReadResourceRequest, Request,
    ResourcePayload, UpdateDocumentRequest, UpsertResourceRequest,
};
use std::path::PathBuf;

struct Fixture {
    _dir: tempfile::TempDir,
    service: Engine<SqliteProjection>,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().expect("temporary source");
    std::fs::write(dir.path().join("alpha.md"), "# Alpha Note\n\nshared body\n")
        .expect("write alpha fixture");
    std::fs::write(dir.path().join("beta.md"), "# Beta Note\n\nother body\n")
        .expect("write beta fixture");
    std::fs::create_dir_all(dir.path().join(".notez")).expect("create index directory");

    let config = SourceConfig {
        version: 2,
        source: SourceIdentity {
            name: "test".into(),
            database: PathBuf::from(".notez/index.sqlite"),
        },
        workflow: Default::default(),
        sources: vec![],
        link_overrides: serde_json::Value::Null,
        scan: Default::default(),
    };
    let context = notez_core::application::context::SourceContext::new(
        "test",
        dir.path().to_path_buf(),
        config,
    );
    let db_path = dir.path().join(".notez/index.sqlite");
    let store = SqliteProjection::open(&db_path).expect("open fixture projection");
    let mut service = Engine::with_source(store, context);
    service.register_format_parser(Box::new(orgmode::OrgParser::new()));
    service.register_format_parser(Box::new(markdown::MarkdownParser::new()));
    Fixture { _dir: dir, service }
}

fn dispatch(fixture: &mut Fixture, request: Request) -> Result<Response, ApplicationError> {
    ApplicationDispatcher::new(&mut fixture.service).dispatch(request)
}

fn scan(fixture: &mut Fixture) {
    dispatch(
        fixture,
        Request::ScanNative(notez_protocol::request::ScanNativeRequest {}),
    )
    .expect("fixture scan");
}

fn page(response: Response) -> notez_protocol::response::QueryPage {
    match response {
        Response::ResourcePage(page) => page,
        other => panic!("expected typed resource page, got {other:?}"),
    }
}

#[test]
fn typed_list_query_read_and_search_responses_are_surface_equivalent() {
    let mut left = fixture();
    let mut right = fixture();
    scan(&mut left);
    scan(&mut right);

    let list = ListBySourceRequest {
        source_id: "native".into(),
        limit: None,
    };
    let left_list = dispatch(&mut left, Request::ListBySource(list.clone())).expect("left list");
    let right_list = dispatch(&mut right, Request::ListBySource(list)).expect("right list");
    assert_eq!(left_list, right_list);

    let query = QueryResourcesRequest {
        kind: None,
        title_contains: None,
        exact_ref: None,
        source_id: Some("native".into()),
        limit: None,
    };
    let left_query = dispatch(&mut left, Request::QueryResources(query.clone())).expect("left query");
    let right_query = dispatch(&mut right, Request::QueryResources(query)).expect("right query");
    assert_eq!(left_query, right_query);

    let queried = page(left_query);
    let row = queried
        .items
        .iter()
        .find(|row| row.locator == "alpha.md")
        .expect("alpha fixture resource");
    let read = ReadResourceRequest { r_ref: row.ref_.clone() };
    let left_read = dispatch(&mut left, Request::ReadResource(read.clone())).expect("left read");
    let right_read = dispatch(&mut right, Request::ReadResource(read)).expect("right read");
    assert_eq!(left_read, right_read);

    // Search is exercised through the shared QueryResources protocol request.
    let search = QueryResourcesRequest {
        kind: Some("document".into()),
        title_contains: Some("Alpha".into()),
        exact_ref: None,
        source_id: Some("native".into()),
        limit: None,
    };
    let left_search = dispatch(&mut left, Request::QueryResources(search.clone())).expect("left search");
    let right_search = dispatch(&mut right, Request::QueryResources(search)).expect("right search");
}

#[test]
fn typed_update_response_is_equivalent_for_identical_temp_sources() {
    let mut left = fixture();
    let mut right = fixture();
    scan(&mut left);
    scan(&mut right);

    let request = QueryResourcesRequest {
        kind: None,
        title_contains: None,
        exact_ref: None,
        source_id: Some("native".into()),
        limit: None,
    };
    let left_row = page(dispatch(&mut left, Request::QueryResources(request.clone())).expect("left query"))
        .items
        .into_iter()
        .find(|row| row.locator == "alpha.md")
        .expect("left alpha resource");
    let right_row = page(dispatch(&mut right, Request::QueryResources(request)).expect("right query"))
        .items
        .into_iter()
        .find(|row| row.locator == "alpha.md")
        .expect("right alpha resource");
    assert_eq!(left_row, right_row);

    let update = |revision: String| Request::UpdateDocument(UpdateDocumentRequest {
        source_id: "native".into(),
        locator: "alpha.md".into(),
        content: "# Alpha Note\n\nupdated body\n".into(),
        base_revision: None,
        format: Some("markdown".into()),
        expected_revision: Some(revision),
    });
    let left_update = dispatch(&mut left, update(left_row.revision.clone())).expect("left update");
    let right_update = dispatch(&mut right, update(right_row.revision.clone())).expect("right update");
    assert_eq!(left_update, right_update);
    let expected = "# Alpha Note\n\nupdated body\n";
    assert_eq!(std::fs::read_to_string(left.service.source().expect("source").root.join("alpha.md")).unwrap(), expected);
    assert_eq!(std::fs::read_to_string(right.service.source().expect("source").root.join("alpha.md")).unwrap(), expected);
}

#[test]
fn typed_error_classification_is_equivalent_for_stale_revision_and_invalid_ref() {
    let mut left = fixture();
    let mut right = fixture();
    let payload = ResourcePayload {
        ref_: "heading:01J000000000000000000000F1".into(),
        kind: "heading".into(),
        title: "Seed".into(),
        revision: "actual-revision".into(),
        source_id: "native".into(),
        locator: "seed.org".into(),
        properties: Default::default(),
        object_id: None,
        primary_source_id: String::new(),
    };
    for target in [&mut left, &mut right] {
        dispatch(target, Request::UpsertResource(UpsertResourceRequest {
            resource: payload.clone(),
            expected_revision: None,
        }))
        .expect("seed resource");
    }

    let stale = |expected: &str| Request::UpsertResource(UpsertResourceRequest {
        resource: payload.clone(),
        expected_revision: Some(expected.into()),
    });
    let left_stale = dispatch(&mut left, stale("stale-revision")).expect_err("left stale error");
    let right_stale = dispatch(&mut right, stale("stale-revision")).expect_err("right stale error");
    assert_eq!(left_stale, right_stale);
    assert!(matches!(left_stale, ApplicationError::RevisionConflict { .. }));

    let invalid = || Request::ReadResource(ReadResourceRequest {
        r_ref: "not-a-resource-ref".into(),
    });
    let left_invalid = dispatch(&mut left, invalid()).expect_err("left invalid-ref error");
    let right_invalid = dispatch(&mut right, invalid()).expect_err("right invalid-ref error");
    assert_eq!(left_invalid, right_invalid);
    assert!(matches!(left_invalid, ApplicationError::InvalidRequest { .. }));
}
