//! `notez_web` — the notez web surface.
//!
//! Two layers live here:
//!
//! * [`ui`] — the server-rendered workspace (browse / read / edit /
//!   preview attachments). Plain HTML + forms, no client framework.
//! * [`host`] — process assembly: the workspace router, the HTTP
//!   protocol API (`/api/v1/*`) and MCP (`/mcp`) share one composition
//!   `Runtime`, so every surface sees the same engine cache and
//!   watcher.
//!
//! [`body`] renders document and attachment previews through the
//! `notez-preview` catalog; [`server`] and [`routes`] hold the
//! projection helpers and process-global state.

pub mod body;
pub mod host;
pub mod janet;
pub mod model;
pub mod routes;
pub mod server;
pub mod ui;
