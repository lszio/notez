//! Use case traits. The application facade implements each of these so
//! that callers (CLI, MCP, integration tests) can request a narrow
//! capability without depending on the full facade.

mod attachment;
mod link;
mod resource;
mod scan;
mod task;

pub use attachment::AttachmentUseCase;
pub use link::LinkUseCase;
pub use resource::ResourceUseCase;
pub use scan::ScanUseCase;
pub use task::TaskUseCase;