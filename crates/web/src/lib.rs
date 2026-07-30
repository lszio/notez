//! Crate `notez-web` — axum HTTP server that renders `notez` spaces,
//! resources, and attachments through the `preview::PreviewerCatalog`.
//!
//! The crate exposes both a library (`web::…`) and a binary (`notez-web`).
//! The `crates/cli` binary can also embed the same library under the
//! `web` cargo feature and dispatch `notez web …` to it.
pub mod preview;
pub mod router;
pub mod routes;
pub mod send;
pub mod server;
pub mod state;

pub use preview::default_catalog;
pub use send::SendService;
pub use state::WebState;