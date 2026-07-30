//! Crate `notez-web` — axum HTTP server that renders `notez` spaces,
//! resources, and attachments through the `preview::PreviewerCatalog`.
//!
//! The crate exposes both a library (`web::…`) and a binary (`notez-web`).
//! The `crates/cli` binary can also embed the same library under the
//! `web` cargo feature and dispatch `notez web …` to it.

pub mod preview;
pub mod router;
pub mod routes;
pub mod server;
pub mod state;
// Re-exports of public functions live in their respective modules until D2/D3.