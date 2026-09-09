use serde::{Deserialize, Serialize};
use crate::{ObjectId, SourceId, SpaceId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Space { pub id: SpaceId, pub name: String }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source { pub id: SourceId, pub kind: SourceKind }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind { Local, Git, Remote }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceSourceMembership { pub space_id: SpaceId, pub source_id: SourceId, pub role: SourceRole }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceRole { Primary, Reference, Archive, Derived }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document { pub id: crate::DocumentId, pub source_id: SourceId, pub path: crate::SourcePath, pub revision: crate::Revision }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectProjection { pub object_id: ObjectId, pub document_id: crate::DocumentId, pub source_id: SourceId, pub locator: crate::BlockLocator }
