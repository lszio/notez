pub mod query;
pub mod resource;

pub use query::{Projection, QueryPage, Selector};
pub use resource::{Resource, ResourceKind, ResourceRef, ResourceRefError};
