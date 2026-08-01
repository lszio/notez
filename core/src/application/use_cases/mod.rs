//! Use case traits. The application facade implements each of these so
//! that callers (CLI, MCP, integration tests) can request a narrow
//! capability without depending on the full facade.

mod artifact;
mod attachment;
mod community;
mod inspect;
mod link;
mod resource;
mod scan;
mod sync;
mod task;

pub use artifact::ArtifactUseCase;
pub use attachment::AttachmentUseCase;
pub use community::CommunityUseCase;
pub use inspect::InspectUseCase;
pub use link::LinkUseCase;
pub use resource::ResourceUseCase;
pub use scan::ScanUseCase;
pub use sync::SyncUseCase;
pub use task::TaskUseCase;