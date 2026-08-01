//! Sync use case: push, pull, relay, and conflict listing.

use std::path::Path;

use crate::application::writeback::RelaySyncReport;
use crate::application::ApplicationError;
use crate::sync::ConflictRecord;

pub trait SyncUseCase {
    fn sync_push(
        &mut self,
        actor_id: &str,
        space_root: &Path,
        shared_folder: &Path,
    ) -> Result<crate::sync::PushReport, ApplicationError>;
    fn sync_pull(
        &mut self,
        actor_id: &str,
        space_root: &Path,
        shared_folder: &Path,
    ) -> Result<crate::sync::PullReport, ApplicationError>;
    fn relay_sync(
        &self,
        source_id: &str,
        space_root: &Path,
    ) -> Result<RelaySyncReport, ApplicationError>;
    fn list_conflicts(&self) -> Result<Vec<ConflictRecord>, ApplicationError>;
}