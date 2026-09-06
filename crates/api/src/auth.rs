//! Authentication middleware for the HTTP protocol API.
//!
//! One choke point (`auth_middleware`) authenticates every route except
//! `/api/v1/healthz` (liveness must work when the IdP is down). Three
//! policies, selected per deployment through [`AuthConfig`]:
//!
//! * [`AuthConfig::Anonymous`] — no authentication. Loopback-only dev
//!   servers; `ServerConfig::from_env` refuses to select it for
//!   public binds.
//! * [`AuthConfig::StaticToken`] — a shared secret accepted as
//!   `Authorization: Bearer <token>` (constant-time comparison).
//! * [`AuthConfig::Oidc`] — JWT access tokens issued by an external
//!   identity provider (e.g. a self-hosted authentik). Signatures are
//!   verified against the provider's published JWKS; `iss`, `aud`
//!   (client id), and `exp` are enforced. See [`oidc::OidcValidator`].
//!
//! On success the middleware inserts [`AuthContext`] as a request
//! extension, so handlers (and future authorization layers) can read
//! the authenticated identity without re-parsing tokens.
//!
//! This middleware only *authenticates*. Per-operation authorization
//! (scopes → request categories) is deliberately a separate layer and
//! not claimed here.

pub mod oidc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use jsonwebtoken::Algorithm;
use serde_json::json;

/// How the server authenticates requests.
#[derive(Clone)]
pub enum AuthConfig {
    /// No authentication (development only; refused for public binds).
    Anonymous,
    /// A single shared secret; `Bearer <token>` must match exactly.
    StaticToken { token: String },
    /// Validate provider-signed JWTs against the provider's JWKS.
    Oidc {
        validator: std::sync::Arc<oidc::OidcValidator>,
    },
}

impl AuthConfig {
    /// Stable mode label used by `/api/v1/healthz` diagnostics.
    pub fn mode_label(&self) -> &'static str {
        match self {
            AuthConfig::Anonymous => "anonymous",
            AuthConfig::StaticToken { .. } => "token",
            AuthConfig::Oidc { .. } => "oidc",
        }
    }
}

/// Identity established by [`auth_middleware`], stored as a request
/// extension.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// `sub` claim (OIDC) or a synthetic subject for the other modes.
    pub subject: String,
    /// Token issuer URL (OIDC only).
    pub issuer: Option<String>,
    /// Space-separated `scope` claim split into tokens (OIDC only).
    pub scopes: Vec<String>,
    /// Full validated claim set (OIDC only).
    pub claims: Option<serde_json::Value>,
}

impl AuthContext {
    fn anonymous() -> Self {
        Self {
            subject: "anonymous".into(),
            issuer: None,
            scopes: Vec::new(),
            claims: None,
        }
    }

    fn shared_token() -> Self {
        Self {
            subject: "token".into(),
            issuer: None,
            scopes: Vec::new(),
            claims: None,
        }
    }
}

/// Wire error envelope for authentication failures. Matches the
/// protocol error vocabulary (`{"error": {"kind": "unauthorized", ...}}`)
/// so clients parse auth failures like any other protocol error.
fn unauthorized(message: impl Into<String>) -> Response {
    let body = json!({
        "error": {
            "kind": "unauthorized",
            "message": message.into(),
        }
    });
    (StatusCode::UNAUTHORIZED, axum::Json(body)).into_response()
}

/// Liveness and unauthenticated discovery paths.
fn is_public_path(path: &str) -> bool {
    path == "/api/v1/healthz"
}

/// Extract the bearer token from `Authorization: Bearer <token>`.
fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|t| !t.trim().is_empty())
}

/// Constant-time equality for shared secrets. Length leaks by design
/// (standard practice); content does not.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// Authenticate the request, then hand control to the inner router.
///
/// Mounted with `axum::middleware::from_fn_with_state` in
/// [`crate::transport`]; the `AuthConfig` travels through the layer
/// state.
pub async fn auth_middleware(
    State(auth): State<AuthConfig>,
    req: axum::extract::Request,
    next: Next,
) -> Response {
    if is_public_path(req.uri().path()) {
        return next.run(req).await;
    }

    let ctx = match &auth {
        AuthConfig::Anonymous => AuthContext::anonymous(),
        AuthConfig::StaticToken { token: expected } => {
            let token = match bearer_token(req.headers()) {
                Some(t) => t,
                None => {
                    return unauthorized("missing bearer token (Authorization: Bearer <token>)");
                }
            };
            if constant_time_eq(token, expected) {
                AuthContext::shared_token()
            } else {
                return unauthorized("invalid token");
            }
        }
        AuthConfig::Oidc { validator } => {
            let token = match bearer_token(req.headers()) {
                Some(t) => t,
                None => {
                    return unauthorized("missing bearer token (Authorization: Bearer <token>)");
                }
            };
            match validator.validate(token).await {
                Ok(claims) => AuthContext {
                    subject: claims
                        .get("sub")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    issuer: Some(validator.issuer().to_string()),
                    scopes: claims
                        .get("scope")
                        .and_then(|v| v.as_str())
                        .map(|s| s.split_whitespace().map(str::to_string).collect())
                        .unwrap_or_default(),
                    claims: Some(claims),
                },
                Err(err) => return unauthorized(err.to_string()),
            }
        }
    };

    let mut req = req;
    req.extensions_mut().insert(ctx);
    next.run(req).await
}

/// Algorithm set accepted by the OIDC policy. Authentik signs RS256 by
/// default; extend here (and in the validator) if a deployment uses
/// another provider algorithm.
pub const OIDC_ALGORITHMS: [Algorithm; 1] = [Algorithm::RS256];
