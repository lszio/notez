//! CommunityUseCase impl for `ApplicationFacade`.
//!
//! Method bodies were previously inlined in `service.rs`; this file
//! is part of the 0.5.x-A1+A3 use-case impl split.

use crate::application::service::{ApplicationError, ApplicationFacade, StorageErrorKind};
use crate::application::use_cases::CommunityUseCase;
use crate::domain::ProjectionStore;
use crate::domain::community::Community;
use std::path::Path;

impl<S: ProjectionStore> CommunityUseCase for ApplicationFacade<S> {
    fn create_community(
        &self,
        space_root: &Path,
        community: crate::domain::community::Community,
    ) -> Result<(), ApplicationError> {
        let mut cfg = crate::application::community_app::SpaceCommunitiesConfig::load(space_root)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;
        cfg.communities.retain(|c| c.id != community.id);
        cfg.communities.push(community);
        cfg.save(space_root)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;
        Ok(())
    }

    fn list_communities(
        &self,
        space_root: &Path,
    ) -> Result<Vec<crate::domain::community::Community>, ApplicationError> {
        let cfg = crate::application::community_app::SpaceCommunitiesConfig::load(space_root)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;
        Ok(cfg.communities)
    }

}
