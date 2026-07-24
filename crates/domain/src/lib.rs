pub mod community;
pub mod query;
pub use community::{Community, CommunityCandidate, CommunitySelector};
pub mod resource;
pub mod rule;
pub mod schema;
pub mod link;
pub use link::{
    LinkOccurrence, LinkTarget, ResolvedRelation, ResolutionStatus, ResourceAddress, TextSpan,
};
pub use rule::{InspectResult, Rule, RuleEngine, RuleKind, RuleTrace};

pub use query::{Projection, ProjectionStore, QueryPage, Selector};
pub use resource::{
    Resource, ResourceKind, ResourceRef, ResourceRefError, ResourceRelation, SegmentRecord,
    derived_id,
};
