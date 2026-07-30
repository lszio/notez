//! Bridge between `web` and the `preview::PreviewerCatalog`.
//!
//! Right now this is just a re-export of the canonical 14-previewers catalog.
//! Future work (overrides from `~/.config/notez/config.toml`, custom
//! previewer plugins, …) lives here.

pub use preview::default_catalog;

/// Convenience wrapper that callers can use without spelling out the full
/// path. Equivalent to [`default_catalog`].
pub fn build_catalog() -> preview::PreviewerCatalog {
    default_catalog()
}