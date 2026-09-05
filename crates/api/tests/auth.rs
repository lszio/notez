//! Auth middleware: static token and OIDC (JWKS-validated JWT)
//! policies against the in-process router and a mock identity
//! provider.

mod common;

use axum::http::{Request as HttpRequest, StatusCode};
use tower::ServiceExt;

use common::{
    anonymous_router, body_json, fixture_source, rsa_keypair, sign_token, token_claims,
    token_router,
};

fn post_dispatch(body: &str, token: Option<&str>, source: &std::path::Path) -> HttpRequest<String> {
    let mut builder = HttpRequest::builder()
        .method("POST")
        .uri("/api/v1/dispatch")
        .header("content-type", "application/json")
        .header("x-notez-source", source.to_string_lossy().to_string());
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(body.to_string()).expect("request")
}

#[tokio::test]
async fn healthz_stays_public_but_dispatch_requires_a_token() {
    let app = token_router("s3cret");

    let health = app
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri("/api/v1/healthz")
                .body(String::new())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        health.status(),
        StatusCode::OK,
        "liveness must not need auth"
    );

    let (_dir, root) = fixture_source();
    let missing = app
        .clone()
        .oneshot(post_dispatch(r#"{"op":"query_resources"}"#, None, &root))
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    let body = body_json(missing).await;
    assert_eq!(body["error"]["kind"], "unauthorized");

    let wrong = app
        .clone()
        .oneshot(post_dispatch(
            r#"{"op":"query_resources"}"#,
            Some("wrong"),
            &root,
        ))
        .await
        .unwrap();
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

    let valid = app
        .oneshot(post_dispatch(
            r#"{"op":"query_resources"}"#,
            Some("s3cret"),
            &root,
        ))
        .await
        .unwrap();
    assert_eq!(valid.status(), StatusCode::OK);
}

#[tokio::test]
async fn anonymous_policy_serves_without_any_header() {
    let (_dir, root) = fixture_source();
    let app = anonymous_router();
    let response = app
        .oneshot(post_dispatch(r#"{"op":"query_resources"}"#, None, &root))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn oidc_tokens_validate_against_the_mock_provider() {
    let provider = common::MockOidc::start(None).await;
    let validator = std::sync::Arc::new(notez_api::auth::oidc::OidcValidator::new(
        provider.issuer.clone(),
        "notez-client",
    ));
    let state = notez_api::ApiState::new(None, "oidc").expect("api state");
    let app = notez_api::build_router(notez_api::AuthConfig::Oidc { validator }, state);

    let (_dir, root) = fixture_source();

    // Valid token → dispatch succeeds.
    let ok = app
        .clone()
        .oneshot(post_dispatch(
            r#"{"op":"query_resources"}"#,
            Some(&provider.token("alice", 300)),
            &root,
        ))
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK, "valid OIDC token must pass");

    // Wrong audience → 401.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let wrong_aud = sign_token(
        &provider.signing_pem,
        provider.kid,
        serde_json::json!({
            "iss": provider.issuer, "aud": "other-app",
            "sub": "alice", "exp": now + 300,
        }),
    );
    let rejected = app
        .clone()
        .oneshot(post_dispatch(
            r#"{"op":"query_resources"}"#,
            Some(&wrong_aud),
            &root,
        ))
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);

    // Expired → 401.
    let expired = provider.token("alice", -100);
    let rejected = app
        .clone()
        .oneshot(post_dispatch(
            r#"{"op":"query_resources"}"#,
            Some(&expired),
            &root,
        ))
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);

    // Signed by an unknown key (kid not in the JWKS) → 401.
    let (stranger_pem, _unused) = rsa_keypair("stranger-key");
    let stranger = sign_token(
        &stranger_pem,
        "stranger-key",
        token_claims(&provider.issuer, "notez-client", "mallory", 300),
    );
    let rejected = app
        .oneshot(post_dispatch(
            r#"{"op":"query_resources"}"#,
            Some(&stranger),
            &root,
        ))
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn issuer_mismatch_fails_closed() {
    // The discovery document advertises a different issuer than the
    // validator was configured with → every token must be rejected.
    let provider = common::MockOidc::start(Some("https://evil.example/other/".to_string())).await;
    let validator = std::sync::Arc::new(notez_api::auth::oidc::OidcValidator::new(
        provider.issuer.clone(),
        "notez-client",
    ));
    let state = notez_api::ApiState::new(None, "oidc").expect("api state");
    let app = notez_api::build_router(notez_api::AuthConfig::Oidc { validator }, state);

    let (_dir, root) = fixture_source();
    let response = app
        .oneshot(post_dispatch(
            r#"{"op":"query_resources"}"#,
            Some(&provider.token("alice", 300)),
            &root,
        ))
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "issuer pinning must reject"
    );
}
