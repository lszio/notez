//! HTTP route handlers, one module per logical endpoint.
//!
//! Handlers return axum responses directly. SSR is done with hand-written
//! HTML strings (no Dioxus fullstack SSR) — D2 of Phase D documents this as
//! the acceptable first cut.

pub mod agenda;
pub mod attachment;
pub mod healthz;
pub mod resource;
pub mod root;
pub mod space_hub;
pub mod static_files;