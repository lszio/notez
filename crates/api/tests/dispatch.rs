//! Protocol dispatch transport: end-to-end request → engine →
//! response over the in-process router (no network).

mod common;

use axum::http::{Request as HttpRequest, StatusCode};
use tower::ServiceExt;

use common::{anonymous_router, body_json, fixture_source};

fn post(uri: &str, body: &str, source: Option<&std::path::Path>) -> HttpRequest<String> {
    let mut builder = HttpRequest::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(source) = source {
        builder = builder.header("x-notez-source", source.to_string_lossy().to_string());
    }
    builder.body(body.to_string()).expect("request")
}

#[tokio::test]
async fn scan_then_query_round_trips_through_the_protocol() {
    let (_dir, root) = fixture_source();
    let app = anonymous_router();

    let response = app
        .clone()
        .oneshot(post(
            "/api/v1/dispatch",
            r#"{"op":"scan_native"}"#,
            Some(&root),
        ))
        .await
        .expect("scan response");
    assert_eq!(response.status(), StatusCode::OK);
    let scan = body_json(response).await;
    assert_eq!(scan["result_of"], "scan", "scan payload: {scan}");

    let response = app
        .oneshot(post(
            "/api/v1/dispatch",
            r#"{"op":"query_resources","kind":"document"}"#,
            Some(&root),
        ))
        .await
        .expect("query response");
    assert_eq!(response.status(), StatusCode::OK);
    let page = body_json(response).await;
    assert_eq!(page["result_of"], "resource_page", "page payload: {page}");
    let rendered = page.to_string();
    assert!(
        rendered.contains(r#""title":"hello""#),
        "indexed note missing: {rendered}"
    );
}

#[tokio::test]
async fn server_default_source_is_used_when_header_is_absent() {
    let (_dir, root) = fixture_source();
    let state = notez_api::ApiState::new(Some(root.to_string_lossy().to_string()), "anonymous")
        .expect("api state");
    let app = notez_api::build_router(notez_api::AuthConfig::Anonymous, state);

    let response = app
        .oneshot(post(
            "/api/v1/dispatch",
            r#"{"op":"query_resources"}"#,
            None,
        ))
        .await
        .expect("response");
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "default source must serve"
    );
}

#[tokio::test]
async fn unresolvable_source_is_rejected_as_invalid_request() {
    let app = anonymous_router();
    let response = app
        .oneshot(post(
            "/api/v1/dispatch",
            r#"{"op":"query_resources"}"#,
            Some(std::path::Path::new("/definitely/not/a/notez/source")),
        ))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"]["kind"], "invalid_request");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("cannot resolve source")
    );
}

#[tokio::test]
async fn malformed_body_yields_invalid_request_envelope() {
    let app = anonymous_router();
    let response = app
        .oneshot(post(
            "/api/v1/dispatch",
            r#"{"op":"no_such_operation"}"#,
            None,
        ))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"]["kind"], "invalid_request");
}

#[tokio::test]
async fn discovery_endpoints_expose_schema_capabilities_and_health() {
    let app = anonymous_router();

    let response = app
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri("/api/v1/healthz")
                .body(String::new())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let health = body_json(response).await;
    assert_eq!(health["status"], "ok");
    assert_eq!(health["auth"], "anonymous");

    let response = app
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri("/api/v1/schema")
                .body(String::new())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let schema = body_json(response).await;
    assert!(
        schema.get("request").is_some(),
        "request enum schema missing"
    );

    let response = app
        .oneshot(
            HttpRequest::builder()
                .uri("/api/v1/capabilities")
                .body(String::new())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let capabilities = body_json(response).await;
    assert!(capabilities.is_array(), "capability catalog is an array");
}
