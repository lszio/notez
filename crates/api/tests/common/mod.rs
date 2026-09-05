//! Shared integration-test fixtures: a real notez source on disk,
//! router assembly, and an in-process OIDC provider (discovery +
//! JWKS + token endpoint) that mirrors the authentik wire shape.

#![cfg(test)]

use axum::extract::{Form, State};
use notez_api::auth::AuthConfig;
use notez_api::transport::{ApiState, build_router};
use std::sync::Arc;

/// Create a minimal v2 source: `notez.toml` + one markdown note.
pub fn fixture_source() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("notez.toml"),
        "version = 2\n\
         [source]\n\
         name = \"api-test\"\n\
         database = \"notez.db\"\n",
    )
    .expect("write notez.toml");
    std::fs::create_dir_all(dir.path().join("notes")).expect("mkdir notes");
    std::fs::write(
        dir.path().join("notes").join("hello.md"),
        "# Hello\n\nA note for the api test.\n",
    )
    .expect("write note");
    let root = dir.path().to_path_buf();
    (dir, root)
}

/// Anonymous router over an empty engine cache.
pub fn anonymous_router() -> axum::Router {
    let state = ApiState::new(None, "anonymous").expect("api state");
    build_router(AuthConfig::Anonymous, state)
}

pub fn token_router(token: &str) -> axum::Router {
    let state = ApiState::new(None, "token").expect("api state");
    build_router(
        AuthConfig::StaticToken {
            token: token.to_string(),
        },
        state,
    )
}

/// Read a response body as JSON.
pub async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    serde_json::from_slice(&bytes).expect("json body")
}

/// Generate a fresh RSA keypair and return (signing PEM, JWK JSON).
///
/// The JWK mirrors what an authentik provider publishes under its
/// `jwks_uri` (RS256, `kid` set) so `jsonwebtoken`'s `JwkSet`
/// deserializes it unchanged.
pub fn rsa_keypair(kid: &str) -> (Vec<u8>, serde_json::Value) {
    use rsa::pkcs8::EncodePrivateKey;
    use rsa::traits::PublicKeyParts;
    use rsa::{RsaPrivateKey, RsaPublicKey};

    let mut rng = rand_core::OsRng;
    let private = RsaPrivateKey::new(&mut rng, 2048).expect("rsa keygen");
    let public = RsaPublicKey::from(&private);
    let pem = private
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .expect("pkcs8 pem");

    let n = base64url(&public.n().to_bytes_be());
    let e = base64url(&public.e().to_bytes_be());
    let jwk = serde_json::json!({
        "kty": "RSA",
        "kid": kid,
        "alg": "RS256",
        "use": "sig",
        "n": n,
        "e": e,
    });
    (pem.as_bytes().to_vec(), jwk)
}

/// Unpadded base64url (JWK encoding).
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[n as usize & 63] as char);
        }
    }
    out
}

/// Sign one RS256 token with `kid` in the header.
pub fn sign_token(signing_pem: &[u8], kid: &str, claims: serde_json::Value) -> String {
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_string());
    jsonwebtoken::encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(signing_pem).expect("encoding key"),
    )
    .expect("encode token")
}

/// Claim set carrying the mandatory registered claims.
pub fn token_claims(
    issuer: &str,
    audience: &str,
    subject: &str,
    exp_offset: i64,
) -> serde_json::Value {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    serde_json::json!({
        "iss": issuer,
        "aud": audience,
        "sub": subject,
        "iat": now,
        "exp": now + exp_offset,
        "scope": "read write",
    })
}

/// Shared state for the mock provider routes.
#[derive(Clone)]
struct ProviderState {
    issuer: String,
    jwk: serde_json::Value,
    signing_pem: Arc<Vec<u8>>,
}

/// An in-process OIDC provider: discovery document, JWKS, and a
/// client-credentials token endpoint. Lives until the returned guard
/// is dropped.
pub struct MockOidc {
    pub issuer: String,
    pub signing_pem: Vec<u8>,
    pub kid: &'static str,
    local_addr: std::net::SocketAddr,
    _server: tokio::task::JoinHandle<()>,
}

impl MockOidc {
    /// Start the provider. `advertised_issuer` overrides what the
    /// discovery document claims (to test issuer pinning); pass
    /// `None` for the truthful default.
    pub async fn start(advertised_issuer: Option<String>) -> MockOidc {
        let (signing_pem, jwk) = rsa_keypair("test-key");

        // Bind first: the issuer URL (and thus the discovery payload)
        // depends on the ephemeral port.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let local_addr = listener.local_addr().expect("local addr");
        let issuer = advertised_issuer
            .unwrap_or_else(|| format!("http://{local_addr}/application/o/notez/"));

        let st = Arc::new(ProviderState {
            issuer: issuer.clone(),
            jwk,
            signing_pem: Arc::new(signing_pem.clone()),
        });

        let app = axum::Router::new()
            .route(
                "/application/o/notez/.well-known/openid-configuration",
                axum::routing::get(|State(st): State<Arc<ProviderState>>| async move {
                    axum::Json(serde_json::json!({
                        "issuer": st.issuer,
                        "jwks_uri": format!("{}jwks/", st.issuer),
                    }))
                }),
            )
            .route(
                "/application/o/notez/jwks/",
                axum::routing::get(|State(st): State<Arc<ProviderState>>| async move {
                    axum::Json(serde_json::json!({ "keys": [st.jwk] }))
                }),
            )
            .route(
                "/application/o/notez/token/",
                axum::routing::post(
                    |State(st): State<Arc<ProviderState>>,
                     Form(form): Form<std::collections::HashMap<String, String>>| async move {
                        if form.get("grant_type").map(String::as_str)
                            != Some("client_credentials")
                            || form.get("client_id").is_none()
                            || form.get("client_secret").is_none()
                        {
                            return (
                                axum::http::StatusCode::UNAUTHORIZED,
                                axum::Json(serde_json::json!({ "error": "invalid_client" })),
                            );
                        }
                        let claims =
                            token_claims(&st.issuer, "notez-client", "service-account", 3600);
                        (
                            axum::http::StatusCode::OK,
                            axum::Json(serde_json::json!({
                                "access_token": sign_token(&st.signing_pem, "test-key", claims),
                                "token_type": "Bearer",
                                "expires_in": 3600,
                            })),
                        )
                    },
                ),
            )
            .with_state(st);

        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("mock oidc serve");
        });

        MockOidc {
            issuer,
            signing_pem,
            kid: "test-key",
            local_addr,
            _server: server,
        }
    }

    /// Token endpoint URL for client-credentials tests.
    pub fn token_url(&self) -> String {
        format!("http://{}/application/o/notez/token/", self.local_addr)
    }

    /// Mint a valid access token.
    pub fn token(&self, subject: &str, exp_offset: i64) -> String {
        sign_token(
            &self.signing_pem,
            self.kid,
            token_claims(&self.issuer, "notez-client", subject, exp_offset),
        )
    }
}
