//! Sync use case: push, pull, relay, and conflict listing.

use std::path::Path;

use crate::application::ApplicationError;
use crate::application::writeback::RelaySyncReport;
use crate::sync::ConflictRecord;

pub trait SyncUseCase {
    fn sync_push(
        &mut self,
        actor_id: &str,
        shared_folder: &Path,
    ) -> Result<crate::sync::PushReport, ApplicationError>;
    fn sync_pull(
        &mut self,
        actor_id: &str,
        shared_folder: &Path,
    ) -> Result<crate::sync::PullReport, ApplicationError>;
    fn relay_sync(&self) -> Result<RelaySyncReport, ApplicationError>;
    fn list_conflicts(&self) -> Result<Vec<ConflictRecord>, ApplicationError>;

    /// Adjudicate a pending conflict: keep `mine` or `theirs`, write
    /// the chosen object payload over the working file, and clear the
    /// conflict record from the projection.
    fn resolve_conflict(
        &mut self,
        shared_folder: &Path,
        logical_path: &str,
        keep_mine: bool,
    ) -> Result<crate::application::writeback::WritebackReport, ApplicationError>;
}
