//! `notez_web` — the notez web surface.
//!
//! Two layers live here:
//!
//! * [`app`] — the Dioxus rendering core: pages are server-rendered
//!   from data read during the render, so the browser receives
//!   complete HTML (no hydration, no `#[server]` round trip).
//! * [`data`] — server-side data access (spaces, files, documents,
//!   saves) and the utility HTTP endpoints (raw bytes, form writes,
//!   live preview, status).
//!
//! [`host`] assembles the process: the workspace, the HTTP protocol API
//! (`/api/v1/*`) and MCP (`/mcp`) share one composition `Runtime`, so
//! every surface sees the same engine cache and watcher.
//!
//! [`body`] renders document and attachment previews through the
//! `notez-preview` catalog; [`server`] holds the projection helpers and
//! the save outcome types.

pub mod app;
pub mod body;
pub mod data;
pub mod host;
pub mod janet;
pub mod model;
pub mod routes;
pub mod server;
