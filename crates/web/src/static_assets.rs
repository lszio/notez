//! Static assets bundled into the `notez-web` binary.
//!
//! The contents of `crates/web/static/js/preview/*.mjs` are copied into
//! `OUT_DIR/static/` at compile time by `build.rs`. We `include_str!` them
//! here so they ship with the binary and can be served via
//! [`crate::routes::static_files`] without needing the operator to drop
//! the source tree next to the binary at runtime.

pub const MERMAID_MJS: &str = include_str!("../static/js/preview/mermaid.mjs");
pub const D2_MJS: &str = include_str!("../static/js/preview/d2.mjs");