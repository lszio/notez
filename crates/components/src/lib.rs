//! Dioxus server-renderable components for the `notez` web frontend.
//!
//! Each module exposes one or more `#[component]` functions that the
//! `crates/web` server composes into SSR HTML. The rendering is
//! intentionally plain — no client-side runtime is needed; the generated
//! HTML is suitable for axum responses.

pub mod escape;
pub mod layout;