pub mod community;
pub mod conflict;
pub mod query;
pub use community::{Community, CommunityCandidate, CommunitySelector};
pub mod link;
pub mod resource;
pub mod rule;
pub mod schema;
pub use link::{
    LinkOccurrence, LinkTarget, RelationDirection, RelationType, ResolutionStatus,
    ResolvedRelation, ResourceAddress, TextSpan,
};
pub use rule::{InspectResult, Rule, RuleEngine, RuleKind, RuleTrace};

pub use conflict::ConflictRecord;
pub use query::{LinkDiagnostic, Projection, ProjectionStore, QueryPage, Selector};
pub use resource::{
    ObjectIdentity, ObjectIdentityError, Resource, ResourceKind, ResourceRef, ResourceRefError,
    ResourceRelation, SegmentRecord, derived_id, derived_object_id,
};
