//! CommunityUseCase impl for `ApplicationFacade`.
//!
//! Method bodies were previously inlined in `service.rs`; this file
//! is part of the 0.5.x-A1+A3 use-case impl split.

use crate::application::service::{ApplicationError, ApplicationFacade, StorageErrorKind};
use crate::application::use_cases::CommunityUseCase;
use crate::application::write_check;
use crate::domain::query::{ProjectionReader, ProjectionStore, ProjectionWrite};
use crate::domain::community::Community;

impl<S> CommunityUseCase for ApplicationFacade<S>
where
    S: ProjectionStore,
    S: ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>, {
    fn create_community(
        &self,
        community: crate::domain::community::Community,
    ) -> Result<(), ApplicationError> {
        let source_root = self.require_space_root()?;
        write_check::check_capability(self, "community")?;

        let mut cfg = crate::application::community_app::SpaceCommunitiesConfig::load(&source_root)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;
        cfg.communities.retain(|c| c.id != community.id);
        cfg.communities.push(community);
        cfg.save(&source_root)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;
        Ok(())
    }

    fn list_communities(
        &self,
    ) -> Result<Vec<crate::domain::community::Community>, ApplicationError> {
        let source_root = self.require_space_root()?;
        let cfg = crate::application::community_app::SpaceCommunitiesConfig::load(&source_root)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;
        Ok(cfg.communities)
    }
}
