//! Community use case: groups of resources.

use crate::application::ApplicationError;
use crate::domain::community::Community;

pub trait CommunityUseCase {
    fn create_community(&self, community: Community) -> Result<(), ApplicationError>;
    fn list_communities(&self) -> Result<Vec<Community>, ApplicationError>;
}
