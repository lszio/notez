//! Use case traits. The application facade implements each of these so
//! that callers (CLI, MCP, integration tests) can request a narrow
//! capability without depending on the full facade.

mod resource;
mod scan;

pub use resource::ResourceUseCase;
pub use scan::ScanUseCase;
