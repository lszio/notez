//! SyncUseCase impl for `ApplicationFacade`.
//!
//! Method bodies were previously inlined in `service.rs`; this file
//! is part of the 0.5.x-A1+A3 use-case impl split.

use crate::application::service::{ApplicationError, ApplicationFacade, StorageErrorKind};
use crate::application::use_cases::SyncUseCase;
use crate::application::write_check;
use crate::domain::query::{ProjectionReader, ProjectionStore, ProjectionWrite};
use std::path::Path;

impl<S> SyncUseCase for ApplicationFacade<S>
where
    S: ProjectionStore,
    S: ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>, {
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
            crate::domain::ProjectionWrite::replace_conflicts(&mut self.store, &report.conflicts)
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

    fn resolve_conflict(
        &mut self,
        shared_folder: &Path,
        logical_path: &str,
        keep_mine: bool,
    ) -> Result<crate::application::writeback::WritebackReport, ApplicationError> {
        use crate::domain::ProjectionReader;
        let pending = crate::domain::ProjectionReader::list_conflicts(&self.store)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            })?;
        let record = pending
            .iter()
            .find(|c| c.logical_path == logical_path)
            .ok_or_else(|| ApplicationError::InvalidRequest {
                message: format!("no pending conflict for `{logical_path}`"),
            })?;

        let transport = crate::sync::FolderTransport::new(shared_folder);
        let chosen_hash = if keep_mine {
            record.mine_hash.clone()
        } else {
            record.theirs_hash.clone()
        };
        let obj = transport
            .store
            .read_object(&chosen_hash)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            })?
            .ok_or_else(|| ApplicationError::Storage {
                kind: StorageErrorKind::BlobMissing,
                message: format!("object {chosen_hash} missing from sync folder"),
            })?;

        let target = self.require_space_root()?.join(logical_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ApplicationError::Io {
                path: Some(parent.to_path_buf()),
                source: e.kind(),
            })?;
        }
        std::fs::write(&target, &obj.payload).map_err(|e| ApplicationError::Io {
            path: Some(target.clone()),
            source: e.kind(),
        })?;

        // Clear the adjudicated record; keep any others.
        let remaining: Vec<crate::sync::ConflictRecord> = pending
            .into_iter()
            .filter(|c| c.logical_path != logical_path)
            .collect();
        crate::domain::ProjectionWrite::replace_conflicts(&mut self.store, &remaining)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            })?;

        Ok(crate::application::writeback::WritebackReport {
            target_ref: logical_path.to_string(),
            committed: true,
        })
    }

    fn list_conflicts(&self) -> Result<Vec<crate::sync::ConflictRecord>, ApplicationError> {
        crate::domain::ProjectionReader::list_conflicts(&self.store).map_err(|e| {
            ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            }
        })
    }
}
