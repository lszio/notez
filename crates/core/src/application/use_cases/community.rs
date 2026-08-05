//! Community use case: groups of resources.

use std::path::Path;

use crate::application::ApplicationError;
use crate::domain::community::Community;

pub trait CommunityUseCase {
    fn create_community(
        &self,
        space_root: &Path,
        community: Community,
    ) -> Result<(), ApplicationError>;
    fn list_communities(
        &self,
        space_root: &Path,
    ) -> Result<Vec<Community>, ApplicationError>;
}