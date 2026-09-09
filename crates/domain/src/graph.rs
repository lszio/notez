use serde::{Deserialize, Serialize};
use crate::{ObjectAddress, ObjectId, SpaceId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationScope { Global, Space(SpaceId) }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationKind { LinksTo, References, Contains }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation { pub from: ObjectId, pub to: ObjectId, pub kind: RelationKind, pub scope: RelationScope }

impl Relation {
    pub fn global(from: ObjectId, to: ObjectId, kind: RelationKind) -> Self { Self { from, to, kind, scope: RelationScope::Global } }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphScope { Space(SpaceId), Source(crate::SourceId), Object(ObjectId), CrossSpace(Vec<SpaceId>) }

impl GraphScope { pub fn is_cross_space(&self) -> bool { matches!(self, Self::CrossSpace(_)) } }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphQuery { Neighbors { object: ObjectAddress, scope: GraphScope, depth: u32 } }
