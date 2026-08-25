use crate::source::adapter::{
    ScannedSource, SourceAdapter, SourceCapabilities, SourceConfig, SourceError,
};

/// Anytype source adapter.
///
/// The remote Anytype transport is NOT wired yet (roadmap「后续扩展」).
/// The factory exists so compositions can opt in explicitly, but the
/// adapter reports no capabilities and every read or write attempt fails
/// with a `SourceError` instead of returning fake data or fake success.
pub struct AnytypeSourceAdapter {
    config: SourceConfig,
}

impl AnytypeSourceAdapter {
    pub fn new(config: SourceConfig) -> Self {
        Self { config }
    }
}

impl SourceAdapter for AnytypeSourceAdapter {
    fn config(&self) -> &SourceConfig {
        &self.config
    }

    fn scan(&self) -> Result<ScannedSource, SourceError> {
        Err(SourceError::Other(
            "anytype source adapter is not implemented yet; no remote transport is \
             wired (roadmap: 后续扩展 — Notion / Anytype 双向 Adapter)"
                .into(),
        ))
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities {
            can_read: false,
            can_write: false,
            can_import: false,
            can_watch: false,
        }
    }

    // `prepare_write` / `commit_write` inherit the trait defaults, which
    // refuse writes with "write not supported by this source adapter".
}
