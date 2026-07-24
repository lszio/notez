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

/// Deterministic derived identity for resources without explicit IDs.
///
/// Generates a stable `ResourceRef` from source identity + normalized locator +
/// structural position. The algorithm is versioned (v1) so future changes can
/// migrate.
///
/// Inputs:
/// - `kind`: the resource kind
/// - `source_id`: the source adapter's identity
/// - `locator`: normalized source-relative path
/// - `position`: structural position within the document (e.g. heading index)
///
/// The output ULID encodes a zero timestamp (derived, not temporal) and the
/// lower 80 bits of a SHA-256 hash of the inputs.
pub fn derived_id(
    kind: ResourceKind,
    source_id: &str,
    locator: &str,
    position: &str,
) -> ResourceRef {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(b"notez-derived-id-v1\0");
    hasher.update(kind.as_str().as_bytes());
    hasher.update(b"\0");
    hasher.update(source_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(locator.as_bytes());
    hasher.update(b"\0");
    hasher.update(position.as_bytes());
    let hash = hasher.finalize();

    // Pack lower 80 bits of hash into a ULID with timestamp=0
    let mut bytes = [0u8; 16];
    // timestamp bytes [0..6] = 0 (derived, non-temporal)
    // random bytes [6..16] = hash[0..10]
    bytes[6..16].copy_from_slice(&hash[0..10]);

    let id = Ulid::from_bytes(bytes);
    ResourceRef::new(kind, id)
}
