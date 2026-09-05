//! OIDC / OAuth2 access-token validation against a remote issuer.
//!
//! Deployment shape (self-hosted authentik):
//!
//! 1. In authentik, create an *OAuth2/OpenID Provider* (confidential
//!    client). Its issuer URL is
//!    `https://<authentik>/application/o/<provider-slug>/`.
//! 2. Configure the notez server with that issuer and the provider's
//!    client id as audience (`NOTEZ_API_OIDC_ISSUER` /
//!    `NOTEZ_API_OIDC_AUDIENCE`).
//! 3. Clients obtain a JWT access token — end users through the
//!    authorization-code flow, machine clients through
//!    `client_credentials` (see [`crate::client::TokenSource`]) — and
//!    send it as `Authorization: Bearer <token>`.
//!
//! Validation: on first use the validator fetches
//! `<issuer>/.well-known/openid-configuration`, pins the advertised
//! `issuer`, and caches the `jwks_uri` key set. Tokens must be RS256,
//! carry `aud` equal to the configured audience, be unexpired, and be
//! signed by a key in the cached JWKS. The key set refreshes every
//! [`JWKS_REFRESH`] and once more on an unknown `kid` so provider key
//! rotation converges without a restart.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;

/// How long a fetched JWKS stays trusted before proactive refresh.
const JWKS_REFRESH: Duration = Duration::from_secs(600);
/// Clock skew tolerated for `exp` / `nbf`.
const LEEWAY_SECS: u64 = 30;

/// Errors surfaced by [`OidcValidator::validate`]. Rendered into the
/// 401 response body, so messages are safe to expose (no secrets —
/// only validation outcomes).
#[derive(Debug, thiserror::Error)]
pub enum OidcError {
    #[error("token is not a JWT (unreadable header)")]
    MalformedHeader,
    #[error("token signing algorithm is not accepted (expected RS256)")]
    UnsupportedAlgorithm,
    #[error("token key id is unknown to the issuer (kid={0})")]
    UnknownKey(String),
    #[error("token validation failed: {0}")]
    Invalid(String),
    #[error("identity provider discovery failed: {0}")]
    Discovery(String),
    #[error("identity provider JWKS unavailable: {0}")]
    Jwks(String),
}

#[derive(Deserialize)]
struct Discovery {
    issuer: String,
    jwks_uri: String,
}

#[derive(Clone)]
struct KeyCache {
    jwks: JwkSet,
    fetched_at: Instant,
}

/// Immutable configuration plus a mutable key cache. `validate` is
/// async but the cache lock is only ever held synchronously.
pub struct OidcValidator {
    http: reqwest::Client,
    issuer: String,
    audience: String,
    cache: Mutex<Option<KeyCache>>,
}

impl OidcValidator {
    /// Configure a validator. No network I/O here: discovery and JWKS
    /// fetch happen lazily on first token, so a start-up does not fail
    /// when the provider is momentarily unreachable.
    pub fn new(issuer: impl Into<String>, audience: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("reqwest client with static config"),
            issuer: issuer.into(),
            audience: audience.into(),
            cache: Mutex::new(None),
        }
    }

    /// The configured issuer URL.
    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// The configured audience (the authentik provider client id).
    pub fn audience(&self) -> &str {
        &self.audience
    }

    /// Validate one access token and return its claim set.
    pub async fn validate(&self, token: &str) -> Result<serde_json::Value, OidcError> {
        let header = decode_header(token).map_err(|_| OidcError::MalformedHeader)?;
        if !crate::auth::OIDC_ALGORITHMS.contains(&header.alg) {
            return Err(OidcError::UnsupportedAlgorithm);
        }
        let kid = header
            .kid
            .clone()
            .ok_or_else(|| OidcError::UnknownKey("<missing>".to_string()))?;

        let validation = self.validation();
        let decode_with = |jwks: &JwkSet| -> Option<Result<serde_json::Value, OidcError>> {
            let jwk = jwks.find(&kid)?;
            let key = DecodingKey::from_jwk(jwk).ok()?;
            match decode::<serde_json::Value>(token, &key, &validation) {
                Ok(data) => Some(Ok(data.claims)),
                Err(e) => Some(Err(OidcError::Invalid(e.to_string()))),
            }
        };

        // Fast path: cached, fresh keys.
        if let Some(claims) = self.with_cache(|cache| {
            let cached = cache.as_ref()?;
            if cached.fetched_at.elapsed() < JWKS_REFRESH {
                decode_with(&cached.jwks)
            } else {
                None // expired cache: fall through to refresh
            }
        }) {
            return claims;
        }

        // Kid present in a stale cache but signature/claims invalid
        // would already have returned above; reaching the refresh path
        // means cache expired or kid unknown. Refresh once, then retry.
        let jwks = self.fetch_jwks().await?;
        *self.cache.lock().expect("oidc cache lock") = Some(KeyCache {
            jwks,
            fetched_at: Instant::now(),
        });

        let result = self.with_cache(|cache| {
            cache
                .as_ref()
                .and_then(|c| decode_with(&c.jwks))
                .or_else(|| Some(Err(OidcError::UnknownKey(kid.clone()))))
        });
        result.unwrap_or_else(|| Err(OidcError::UnknownKey(kid)))
    }

    fn validation(&self) -> Validation {
        let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);
        validation.leeway = LEEWAY_SECS;
        validation
    }

    /// Run `f` against a snapshot of the cache without holding the
    /// lock across the closure (it is synchronous).
    fn with_cache<T>(&self, f: impl FnOnce(&Option<KeyCache>) -> T) -> T {
        let guard = self.cache.lock().expect("oidc cache lock");
        f(&guard)
    }

    /// Fetch (or refresh) discovery + JWKS from the provider.
    async fn fetch_jwks(&self) -> Result<JwkSet, OidcError> {
        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            self.issuer.trim_end_matches('/')
        );
        let discovery: Discovery = self
            .http
            .get(&discovery_url)
            .send()
            .await
            .map_err(|e| OidcError::Discovery(e.to_string()))?
            .error_for_status()
            .map_err(|e| OidcError::Discovery(e.to_string()))?
            .json()
            .await
            .map_err(|e| OidcError::Discovery(format!("invalid discovery document: {e}")))?;
        // Pin the advertised issuer: a misconfigured issuer URL must
        // fail loudly instead of validating against the wrong tenant.
        if discovery.issuer.trim_end_matches('/') != self.issuer.trim_end_matches('/') {
            return Err(OidcError::Discovery(format!(
                "issuer mismatch: provider advertises '{}', configured '{}'",
                discovery.issuer, self.issuer
            )));
        }
        self.http
            .get(&discovery.jwks_uri)
            .send()
            .await
            .map_err(|e| OidcError::Jwks(e.to_string()))?
            .error_for_status()
            .map_err(|e| OidcError::Jwks(e.to_string()))?
            .json()
            .await
            .map_err(|e| OidcError::Jwks(format!("invalid jwks document: {e}")))
    }
}
