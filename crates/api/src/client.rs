//! Remote client: the other half of the HTTP protocol transport.
//!
//! [`NotezClient`] posts [`Request`] values to a notez API server and
//! returns typed [`Response`] values — the exact mirror of what the
//! server's `/api/v1/dispatch` speaks, so any Rust surface (CLI, a
//! future remote Source adapter, tests) talks to a remote notez
//! instance with the same vocabulary it uses in-process.
//!
//! Token sources:
//!
//! * [`TokenSource::None`] — anonymous servers (loopback dev).
//! * [`TokenSource::Static`] — shared secret (`NOTEZ_API_TOKEN`).
//! * [`TokenSource::ClientCredentials`] — OAuth2 machine-to-machine:
//!   fetches an access token from the identity provider's token
//!   endpoint (e.g. authentik application with the client-credentials
//!   grant enabled) and caches it until shortly before expiry. The
//!   provider must sign RS256 with a JWKS the server can reach —
//!   the standard authentik provider setup.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use notez_protocol::Response;
use notez_protocol::request::Request;
use notez_protocol::schema::request_schemas;

/// How the client authenticates against the server's auth middleware.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum TokenSource {
    /// No `Authorization` header (anonymous server policy only).
    None,
    /// Constant bearer token on every request.
    Static(String),
    /// OAuth2 `client_credentials` grant against an identity provider.
    ClientCredentials {
        token_url: String,
        client_id: String,
        client_secret: String,
        scope: Option<String>,
    },
}

#[derive(Debug, Default)]
struct CachedToken {
    value: Option<(String, Instant)>,
}

/// A thin async client for a remote notez API server.
#[derive(Debug, Clone)]
pub struct NotezClient {
    http: reqwest::Client,
    base_url: String,
    /// Default `X-Notez-Source` header; a server with no
    /// `NOTEZ_API_SOURCE` default needs it (or per-call override).
    default_source: Option<String>,
    tokens: TokenState,
}

#[derive(Debug, Clone)]
struct TokenState {
    source: TokenSource,
    cache: std::sync::Arc<Mutex<CachedToken>>,
}

/// Client-side failure taxonomy. `Protocol` carries the structured
/// error the server produced; everything else is transport-level.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("server returned {status}: {error:?}")]
    Protocol {
        status: u16,
        error: notez_protocol::Error,
    },
    #[error("cannot decode server payload: {0}")]
    Decode(String),
    #[error("invalid configuration: {0}")]
    Config(String),
}

impl NotezClient {
    /// Target `http(s)://host[:port]` of the API server. Trailing
    /// slashes are normalized.
    pub fn new(base_url: impl Into<String>) -> Result<NotezClient, ClientError> {
        let base_url = base_url.into();
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            return Err(ClientError::Config(format!(
                "base url must start with http:// or https:// (got '{base_url}')"
            )));
        }
        Ok(NotezClient {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| ClientError::Config(e.to_string()))?,
            base_url: base_url.trim_end_matches('/').to_string(),
            default_source: None,
            tokens: TokenState {
                source: TokenSource::None,
                cache: Default::default(),
            },
        })
    }

    /// Attach a static bearer token.
    pub fn with_token(mut self, token: impl Into<String>) -> NotezClient {
        self.tokens.source = TokenSource::Static(token.into());
        self
    }

    /// Authenticate via OAuth2 client credentials (authentik M2M).
    pub fn with_client_credentials(
        mut self,
        token_url: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        scope: Option<String>,
    ) -> NotezClient {
        self.tokens.source = TokenSource::ClientCredentials {
            token_url: token_url.into(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            scope,
        };
        self
    }

    /// Send an `X-Notez-Source` header with every dispatch by default.
    pub fn with_default_source(mut self, source: impl Into<String>) -> NotezClient {
        self.default_source = Some(source.into());
        self
    }

    /// Dispatch one protocol request; returns the typed response or
    /// the server's structured error.
    pub async fn dispatch(&self, request: &Request) -> Result<Response, ClientError> {
        let mut req = self.http.post(format!("{}/api/v1/dispatch", self.base_url));
        if let Some(bearer) = self.bearer().await? {
            req = req.bearer_auth(bearer);
        }
        if let Some(source) = &self.default_source {
            req = req.header("x-notez-source", source);
        }
        let response = req.json(request).send().await?;

        let status = response.status();
        let bytes = response.bytes().await?;
        if status.is_success() {
            return serde_json::from_slice(&bytes).map_err(|e| {
                ClientError::Decode(format!("response is not a protocol Response: {e}"))
            });
        }
        let envelope: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| ClientError::Decode(format!("error body ({status}) is not JSON: {e}")))?;
        let error = serde_json::from_value(
            envelope
                .get("error")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        )
        .map_err(|e| ClientError::Decode(format!("error body is not a protocol Error: {e}")))?;
        Err(ClientError::Protocol {
            status: status.as_u16(),
            error,
        })
    }

    /// Liveness probe (`GET /api/v1/healthz`, unauthenticated).
    pub async fn health(&self) -> Result<serde_json::Value, ClientError> {
        let response = self
            .http
            .get(format!("{}/api/v1/healthz", self.base_url))
            .send()
            .await?;
        let value: serde_json::Value = response.json().await?;
        Ok(value)
    }

    /// Protocol request schemas (`GET /api/v1/schema`).
    pub async fn schema(&self) -> Result<serde_json::Value, ClientError> {
        let mut req = self.http.get(format!("{}/api/v1/schema", self.base_url));
        if let Some(bearer) = self.bearer().await? {
            req = req.bearer_auth(bearer);
        }
        let value: serde_json::Value = req.send().await?.json().await?;
        Ok(value)
    }

    /// Engine capability catalog (`GET /api/v1/capabilities`).
    pub async fn capabilities(&self) -> Result<serde_json::Value, ClientError> {
        let mut req = self
            .http
            .get(format!("{}/api/v1/capabilities", self.base_url));
        if let Some(bearer) = self.bearer().await? {
            req = req.bearer_auth(bearer);
        }
        let value: serde_json::Value = req.send().await?.json().await?;
        Ok(value)
    }

    /// JSON Schema map for local construction of requests (same map
    /// the server would return under `/api/v1/schema`).
    pub fn local_request_schemas() -> impl Iterator<Item = (String, serde_json::Value)> {
        request_schemas().into_iter().map(|(name, schema)| {
            (
                name,
                serde_json::to_value(&schema).expect("schema serializes"),
            )
        })
    }

    /// Resolve the bearer token for one request, refreshing the
    /// client-credentials cache when it is at (or within 60s of)
    /// expiry. `None` means "send no Authorization header".
    async fn bearer(&self) -> Result<Option<String>, ClientError> {
        match &self.tokens.source {
            TokenSource::None => Ok(None),
            TokenSource::Static(token) => Ok(Some(token.clone())),
            TokenSource::ClientCredentials {
                token_url,
                client_id,
                client_secret,
                scope,
            } => {
                if let Some((value, until)) = self
                    .tokens
                    .cache
                    .lock()
                    .expect("token cache lock")
                    .value
                    .as_ref()
                {
                    if *until > Instant::now() {
                        return Ok(Some(value.clone()));
                    }
                }
                let fetched = self
                    .fetch_client_credentials(token_url, client_id, client_secret, scope.as_deref())
                    .await?;
                let until =
                    Instant::now() + Duration::from_secs(fetched.expires_in.saturating_sub(60));
                *self.tokens.cache.lock().expect("token cache lock") = CachedToken {
                    value: Some((fetched.access_token.clone(), until)),
                };
                Ok(Some(fetched.access_token))
            }
        }
    }

    async fn fetch_client_credentials(
        &self,
        token_url: &str,
        client_id: &str,
        client_secret: &str,
        scope: Option<&str>,
    ) -> Result<TokenGrant, ClientError> {
        let mut form = vec![
            ("grant_type", "client_credentials".to_string()),
            ("client_id", client_id.to_string()),
            ("client_secret", client_secret.to_string()),
        ];
        if let Some(scope) = scope {
            form.push(("scope", scope.to_string()));
        }
        let grant: TokenGrant = self
            .http
            .post(token_url)
            .form(&form)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .map_err(|e| ClientError::Decode(format!("token endpoint payload invalid: {e}")))?;
        Ok(grant)
    }
}

#[derive(Debug, serde::Deserialize)]
struct TokenGrant {
    access_token: String,
    #[serde(default = "default_expires_in")]
    expires_in: u64,
}

fn default_expires_in() -> u64 {
    300
}
