//! # notez-api
//!
//! HTTP transport for the notez protocol, plus the remote client that
//! speaks it.
//!
//! The architecture contract (docs/current-architecture-and-redesign.md
//! §1) says every surface — CLI, MCP, web, HTTP API — is a *protocol
//! translator*: it converts native syntax into a
//! [`notez_protocol::Request`], hands it to the engine dispatcher, and
//! renders the [`Response`]. This crate is that translator for HTTP:
//!
//! * [`transport`] mounts `POST /api/v1/dispatch` (request JSON →
//!   dispatcher → response JSON) plus `healthz` / `schema` /
//!   `capabilities` discovery endpoints. Sources are selected per
//!   request with the `X-Notez-Source` header (registered source name
//!   or filesystem path), exactly like the web picker.
//! * [`auth`] is a tower/axum middleware that authenticates every
//!   route except `healthz`. Three policies: `anonymous` (local
//!   development), `token` (static shared secret), and `oidc`
//!   (JWT access tokens signed by an external identity provider —
//!   verified against the provider's published JWKS). Deployments plug
//!   further layers onto the returned `axum::Router` in the usual way.
//! * [`client`] is the remote-side mirror: [`client::NotezClient`]
//!   sends protocol requests over HTTP and returns typed responses.
//!   Token sources cover static secrets and OAuth2
//!   `client_credentials` (machine-to-machine against the same IdP).
//!
//! Server assembly:
//!
//! ```no_run
//! # async fn demo() -> Result<(), String> {
//! let config = notez_api::ServerConfig::from_env(None, None)?;
//! notez_api::serve(config).await
//! # }
//! ```

pub mod auth;
pub mod client;
pub mod config;
pub mod error_map;
pub mod transport;

pub use auth::{AuthConfig, AuthContext, auth_middleware};
pub use client::{ClientError, NotezClient, TokenSource};
pub use config::ServerConfig;
pub use transport::{ApiState, build_router, serve};

/// Re-export of the typed protocol response so server and client code
/// share one vocabulary.
pub use notez_protocol::Response;
