//! Crate `notez-web` — axum HTTP server that renders `notez` spaces,
//! resources, and attachments through the `preview::PreviewerCatalog`.
//!
//! This crate is library-only. The `notez` binary embeds it under the
//! `web` cargo feature and dispatches `notez web …` to it.
pub mod html_escape;
pub mod preview;
pub mod router;
pub mod routes;
pub mod send;
pub mod server;
pub mod state;
pub mod static_assets;
pub use preview::default_catalog;
pub use send::SendService;
pub use state::WebState;