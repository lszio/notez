//! Server assembly configuration: bind address, default source, and
//! the authentication policy.
//!
//! Environment contract (all optional):
//!
//! | Variable | Meaning |
//! |---|---|
//! | `NOTEZ_API_BIND` | `addr:port` to listen on (default `127.0.0.1:8700`) |
//! | `NOTEZ_API_SOURCE` | default source (registered name or path) when a request carries no `X-Notez-Source` |
//! | `NOTEZ_API_TOKEN` | static bearer token (shared secret) |
//! | `NOTEZ_API_OIDC_ISSUER` | OIDC issuer URL, e.g. `https://auth.example.com/application/o/notez/` |
//! | `NOTEZ_API_OIDC_AUDIENCE` | accepted token audience (the provider's client id) |
//!
//! Policy resolution: OIDC (issuer + audience) wins over the static
//! token; a public (non-loopback) bind with neither is refused — the
//! server never silently serves an unauthenticated public API.

use crate::auth::AuthConfig;
use crate::auth::oidc::OidcValidator;

/// Fully resolved server launch configuration.
pub struct ServerConfig {
    /// Listen address.
    pub bind: std::net::SocketAddr,
    /// Default source selector when a request omits `X-Notez-Source`.
    pub default_source: Option<String>,
    /// Authentication policy for every non-liveness route.
    pub auth: AuthConfig,
}

impl ServerConfig {
    /// Read the environment contract. `bind_override` /
    /// `source_override` (CLI flags) win over the env defaults.
    pub fn from_env(
        bind_override: Option<String>,
        source_override: Option<String>,
    ) -> Result<ServerConfig, String> {
        let bind_text = bind_override
            .or_else(|| std::env::var("NOTEZ_API_BIND").ok())
            .unwrap_or_else(|| "127.0.0.1:8700".to_string());
        let bind: std::net::SocketAddr = bind_text
            .parse()
            .map_err(|e| format!("invalid bind address '{bind_text}': {e}"))?;

        let default_source = source_override
            .or_else(|| std::env::var("NOTEZ_API_SOURCE").ok())
            .filter(|s| !s.trim().is_empty());

        let issuer = std::env::var("NOTEZ_API_OIDC_ISSUER")
            .ok()
            .filter(|v| !v.trim().is_empty());
        let audience = std::env::var("NOTEZ_API_OIDC_AUDIENCE")
            .ok()
            .filter(|v| !v.trim().is_empty());
        let token = std::env::var("NOTEZ_API_TOKEN")
            .ok()
            .filter(|v| !v.trim().is_empty());

        let auth = match (issuer, audience) {
            (Some(issuer), Some(audience)) => AuthConfig::Oidc {
                validator: std::sync::Arc::new(OidcValidator::new(issuer, audience)),
            },
            (Some(_), None) | (None, Some(_)) => {
                return Err(
                    "NOTEZ_API_OIDC_ISSUER and NOTEZ_API_OIDC_AUDIENCE must be set together"
                        .to_string(),
                );
            }
            (None, None) => match token {
                Some(token) => AuthConfig::StaticToken { token },
                None => {
                    let loopback = bind.ip().is_loopback();
                    if loopback {
                        AuthConfig::Anonymous
                    } else {
                        return Err(format!(
                            "refusing to serve '{bind}' without authentication: set \
                             NOTEZ_API_TOKEN or NOTEZ_API_OIDC_ISSUER + \
                             NOTEZ_API_OIDC_AUDIENCE (loopback binds default to anonymous)"
                        ));
                    }
                }
            },
        };

        Ok(ServerConfig {
            bind,
            default_source,
            auth,
        })
    }
}
