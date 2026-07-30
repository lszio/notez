//! Shared application state handed to every axum handler.
//!
//! `WebState` owns:
//!   - the absolute path of the space root,
//!   - an `Arc` over the `ApplicationService` so multiple handlers can read
//!     from the same `SqliteProjection`,
//!   - an `Arc` over the `PreviewerCatalog` so the catalog isn't rebuilt on
//!     every request.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use application::ApplicationService;
use preview::PreviewerCatalog;
use storage::SqliteProjection;

use crate::preview::default_catalog;

/// Process-wide state passed to every axum handler via `with_state`.
#[derive(Clone)]
pub struct WebState {
    pub space_root: PathBuf,
    pub service: Arc<ApplicationService<SqliteProjection>>,
    pub catalog: Arc<PreviewerCatalog>,
}

impl WebState {
    /// Build a fresh `WebState` for the given space directory.
    ///
    /// Opens (or creates) the SQLite index at `<space_root>/.notez/index.sqlite`,
    /// constructs an [`ApplicationService`] around it, and registers the
    /// default 14-previewers catalog.
    pub fn from_space(root: &Path) -> anyhow::Result<Self> {
        let notez_dir = root.join(".notez");
        std::fs::create_dir_all(&notez_dir)?;
        let db_path = notez_dir.join("index.sqlite");
        let store = SqliteProjection::open(&db_path)
            .map_err(|e| anyhow::anyhow!("open {}: {e}", db_path.display()))?;
        let service = Arc::new(ApplicationService::new(store));
        let catalog = Arc::new(default_catalog());
        Ok(WebState {
            space_root: root.to_path_buf(),
            service,
            catalog,
        })
    }
}