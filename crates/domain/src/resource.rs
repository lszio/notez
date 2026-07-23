use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use ulid::Ulid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Document,
    Heading,
    Attachment,
    Block,
}

impl ResourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResourceKind::Document => "document",
            ResourceKind::Heading => "heading",
            ResourceKind::Attachment => "attachment",
            ResourceKind::Block => "block",
        }
    }
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceRef {
    kind: ResourceKind,
    id: Ulid,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ResourceRefError {
    #[error("invalid format: expected '<kind>:<ulid>'")]
    InvalidFormat,
    #[error("unknown resource kind: {0}")]
    UnknownKind(String),
    #[error("invalid ULID: {0}")]
    InvalidUlid(String),
}

impl ResourceRef {
    pub fn new(kind: ResourceKind, id: Ulid) -> Self {
        Self { kind, id }
    }

    pub fn parse(s: &str) -> Result<Self, ResourceRefError> {
        let (kind_str, ulid_str) = s.split_once(':').ok_or(ResourceRefError::InvalidFormat)?;
        let kind = match kind_str {
            "document" => ResourceKind::Document,
            "heading" => ResourceKind::Heading,
            "attachment" => ResourceKind::Attachment,
            "block" => ResourceKind::Block,
            _ => return Err(ResourceRefError::UnknownKind(kind_str.to_string())),
        };
        let id =
            Ulid::from_str(ulid_str).map_err(|e| ResourceRefError::InvalidUlid(e.to_string()))?;
        Ok(Self { kind, id })
    }

    pub fn kind(&self) -> ResourceKind {
        self.kind
    }

    pub fn id(&self) -> Ulid {
        self.id
    }
}

impl fmt::Display for ResourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind.as_str(), self.id)
    }
}

impl FromStr for ResourceRef {
    type Err = ResourceRefError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for ResourceRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ResourceRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ResourceRef::parse(&s).map_err(de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resource {
    #[serde(rename = "ref")]
    pub r#ref: ResourceRef,
    pub kind: ResourceKind,
    pub title: String,
    pub revision: String,
    pub source_id: String,
    pub locator: String,
    pub properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceRelation {
    pub source_ref: ResourceRef,
    pub relation: String,
    pub target_ref: ResourceRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentRecord {
    pub id: String,
    pub attachment_ref: String,
    pub text: String,
    pub offset_start: usize,
    pub offset_end: usize,
}
