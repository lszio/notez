//! SyncUseCase impl for `ApplicationFacade`.
//!
//! Method bodies were previously inlined in `service.rs`; this file
//! is part of the 0.5.x-A1+A3 use-case impl split.

use crate::application::service::{ApplicationError, ApplicationFacade, StorageErrorKind};
use crate::application::use_cases::SyncUseCase;
use crate::domain::ProjectionStore;
use crate::sync::ConflictRecord;
use std::path::Path;

impl<S: ProjectionStore> SyncUseCase for ApplicationFacade<S> {
    fn sync_push(
        &mut self,
        actor_id: &str,
        space_root: &Path,
        shared_folder: &Path,
    ) -> Result<crate::sync::PushReport, ApplicationError> {
        let transport = crate::sync::FolderTransport::new(shared_folder);
        let engine = crate::sync::SyncEngine::new(actor_id, space_root, transport);
        let report = engine
            .push()
            .map_err(|e| {
                ApplicationError::Storage {
                    kind: StorageErrorKind::InvalidState,
                    message: e.to_string(),
                }
            })?;
        Ok(report)
    }

    fn sync_pull(
        &mut self,
        actor_id: &str,
        space_root: &Path,
        shared_folder: &Path,
    ) -> Result<crate::sync::PullReport, ApplicationError> {
        let transport = crate::sync::FolderTransport::new(shared_folder);
        let engine = crate::sync::SyncEngine::new(actor_id, space_root, transport);
        let report = engine
            .pull()
            .map_err(|e| {
                ApplicationError::Storage {
                    kind: StorageErrorKind::InvalidState,
                    message: e.to_string(),
                }
            })?;

        <Self as crate::application::use_cases::ScanUseCase>::scan_native(self, space_root)?;

        Ok(report)
    }

    fn relay_sync(
        &self,
        _source_id: &str,
        _space_root: &Path,
    ) -> Result<crate::application::writeback::RelaySyncReport, ApplicationError> {
        Err(ApplicationError::UnsupportedCapability {
            capability: "relay sync is not yet implemented; sync via folder transport",
        })
    }

    fn list_conflicts(&self) -> Result<Vec<crate::sync::ConflictRecord>, ApplicationError> {
        Err(ApplicationError::UnsupportedCapability {
            capability: "conflict list is not yet implemented; use `notez sync` commands",
        })
    }

}
