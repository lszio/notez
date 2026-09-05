//! Remote client over a real TCP server: dispatch round-trip with a
//! static token, plus the OAuth2 client-credentials flow against the
//! mock provider.

mod common;

use common::fixture_source;

/// Spawn the API server on an ephemeral loopback port with a static
/// token policy; returns its base URL.
async fn spawn_server(token: &'static str) -> String {
    let state = notez_api::ApiState::new(None, "token").expect("api state");
    let app = notez_api::build_router(
        notez_api::AuthConfig::StaticToken {
            token: token.into(),
        },
        state,
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn client_dispatches_protocol_requests_over_real_http() {
    let (_dir, root) = fixture_source();
    let base = spawn_server("s3cret").await;

    let client = notez_api::NotezClient::new(&base)
        .expect("client")
        .with_token("s3cret")
        .with_default_source(root.to_string_lossy().to_string());

    let health = client.health().await.expect("health");
    assert_eq!(health["status"], "ok");
    assert_eq!(health["auth"], "token");

    let scan = client
        .dispatch(&serde_json::from_str(r#"{"op":"scan_native"}"#).unwrap())
        .await
        .expect("scan");
    let rendered = serde_json::to_value(&scan).unwrap();
    assert_eq!(rendered["result_of"], "scan");

    let query = client
        .dispatch(&serde_json::from_str(r#"{"op":"query_resources","kind":"document"}"#).unwrap())
        .await
        .expect("query");
    let rendered = serde_json::to_string(&query).unwrap();
    assert!(
        rendered.contains(r#""title":"hello""#),
        "indexed note visible: {rendered}"
    );
}

#[tokio::test]
async fn client_surfaces_structured_protocol_errors() {
    let (_dir, root) = fixture_source();
    let base = spawn_server("s3cret").await;
    let source = root.to_string_lossy().to_string();

    // Anonymous client → auth middleware 401, surfaced as protocol error.
    let anon = notez_api::NotezClient::new(&base)
        .expect("client")
        .with_default_source(&source);
    let err = anon
        .dispatch(&serde_json::from_str(r#"{"op":"query_resources"}"#).unwrap())
        .await
        .expect_err("must fail");
    match err {
        notez_api::ClientError::Protocol { status, error } => {
            assert_eq!(status, 401);
            assert!(matches!(error, notez_protocol::Error::Unauthorized { .. }));
        }
        other => panic!("expected protocol error, got {other:?}"),
    }

    // Authenticated but unresolvable source → 400 invalid_request.
    let bad_source = notez_api::NotezClient::new(&base)
        .expect("client")
        .with_token("s3cret")
        .with_default_source("/definitely/not/a/notez/source");
    let err = bad_source
        .dispatch(&serde_json::from_str(r#"{"op":"query_resources"}"#).unwrap())
        .await
        .expect_err("must fail");
    match err {
        notez_api::ClientError::Protocol { status, error } => {
            assert_eq!(status, 400);
            assert!(matches!(
                error,
                notez_protocol::Error::InvalidRequest { .. }
            ));
        }
        other => panic!("expected protocol error, got {other:?}"),
    }
}

#[tokio::test]
async fn client_credentials_flow_authenticates_against_the_mock_provider() {
    let provider = common::MockOidc::start(None).await;
    let validator = std::sync::Arc::new(notez_api::auth::oidc::OidcValidator::new(
        provider.issuer.clone(),
        "notez-client",
    ));
    let state = notez_api::ApiState::new(None, "oidc").expect("api state");
    let app = notez_api::build_router(notez_api::AuthConfig::Oidc { validator }, state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    let (_dir, root) = fixture_source();
    let client = notez_api::NotezClient::new(format!("http://{addr}"))
        .expect("client")
        .with_client_credentials(provider.token_url(), "machine", "secret", None)
        .with_default_source(root.to_string_lossy().to_string());

    // Two dispatches prove the token is fetched once and cached.
    for _ in 0..2 {
        let response = client
            .dispatch(&serde_json::from_str(r#"{"op":"query_resources"}"#).unwrap())
            .await
            .expect("client-credentials dispatch");
        let rendered = serde_json::to_value(&response).unwrap();
        assert_eq!(rendered["result_of"], "resource_page");
    }
}

#[tokio::test]
async fn client_rejects_non_http_base_urls() {
    let err = notez_api::NotezClient::new("ftp://example").expect_err("must fail");
    assert!(matches!(err, notez_api::ClientError::Config(_)));
}
