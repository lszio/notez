//! SyncUseCase impl for `ApplicationFacade`.
//!
//! Method bodies were previously inlined in `service.rs`; this file
//! is part of the 0.5.x-A1+A3 use-case impl split.

use crate::application::service::{ApplicationError, ApplicationFacade, StorageErrorKind};
use crate::application::use_cases::SyncUseCase;
use crate::application::write_check;
use crate::domain::ProjectionStore;
use std::path::Path;

impl<S: ProjectionStore> SyncUseCase for ApplicationFacade<S> {
    fn sync_push(
        &mut self,
        actor_id: &str,
        shared_folder: &Path,
    ) -> Result<crate::sync::PushReport, ApplicationError> {
        let source_root = self.require_space_root()?;
        write_check::check_capability(self, "sync")?;

        let transport = crate::sync::FolderTransport::new(shared_folder);
        let engine = crate::sync::SyncEngine::new(actor_id, &source_root, transport);
        let report = engine.push().map_err(|e| ApplicationError::Storage {
            kind: StorageErrorKind::InvalidState,
            message: e.to_string(),
        })?;
        Ok(report)
    }

    fn sync_pull(
        &mut self,
        actor_id: &str,
        shared_folder: &Path,
    ) -> Result<crate::sync::PullReport, ApplicationError> {
        let source_root = self.require_space_root()?;
        let transport = crate::sync::FolderTransport::new(shared_folder);
        let engine = crate::sync::SyncEngine::new(actor_id, &source_root, transport);
        let report = engine.pull().map_err(|e| ApplicationError::Storage {
            kind: StorageErrorKind::InvalidState,
            message: e.to_string(),
        })?;

        <Self as crate::application::use_cases::ScanUseCase>::scan_native(self)?;

        if !report.conflicts.is_empty() {
            crate::domain::ProjectionStore::replace_conflicts(&mut self.store, &report.conflicts)
                .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            })?;
        }

        Ok(report)
    }
    fn relay_sync(
        &self,
    ) -> Result<crate::application::writeback::RelaySyncReport, ApplicationError> {
        let _space_root = self.require_space_root()?;
        Err(ApplicationError::UnsupportedCapability {
            capability: "relay sync is not yet implemented; sync via folder transport",
        })
    }

    fn list_conflicts(&self) -> Result<Vec<crate::sync::ConflictRecord>, ApplicationError> {
        crate::domain::ProjectionStore::list_conflicts(&self.store).map_err(|e| {
            ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            }
        })
    }
}
