use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use ulid::Ulid;

/// Identity of a configured source within the current Notez instance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId(pub String);

impl SourceId {
    pub fn new(value: impl Into<String>) -> Self { Self(value.into()) }
}

impl std::fmt::Display for SourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
}

/// Stable document identity within a source.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentKey {
    pub source_id: SourceId,
    pub source_local_id: String,
}

/// Content/source revision used for optimistic concurrency.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Revision(pub String);

impl Revision {
    pub fn new(value: impl Into<String>) -> Self { Self(value.into()) }
}

impl std::fmt::Display for Revision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
}
/// Immutable snapshot of a source-backed document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSnapshot {
    pub key: DocumentKey,
    pub title: String,
    pub revision: Revision,
    pub locator: String,
    pub raw_content: String,
}

/// Cross-source object snapshot used by relations and views.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectSnapshot {
    pub id: ObjectIdentity,
    pub title: String,
    pub revision: Revision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState { Inbox, Next, Waiting, Scheduled, Done, Cancelled }

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
    /// 跨 source 稳定的对象身份（spec §2.1）。`authority` 是发 ID 的命名
    /// 权威（即拥有正文的主源命名空间），`native_id` 是 source 原生 ID
    /// 或派生指纹。同一正文 + 同一 locator + 同一 position 在不同 source
    /// 下得同一身份。
    #[serde(default)]
    pub object_id: ObjectIdentity,
    /// 拥有该对象正文的主源 id。空串表示本资源自身的 `source_id` 即主源；
    /// 其余出现（跨 Space、镜像、远程投影）为引用，不复制正文。
    #[serde(default)]
    pub primary_source_id: String,
}

impl Resource {
    /// 返回拥有该对象正文的主源 id。`primary_source_id` 为空时回退到
    /// 资源自身的 `source_id`（「扫描到即主源」的默认）。
    pub fn primary_source(&self) -> &str {
        if self.primary_source_id.is_empty() {
            &self.source_id
        } else {
            &self.primary_source_id
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceRelation {
    pub source_ref: ResourceRef,
    pub relation: String,
    pub target_ref: ResourceRef,
    #[serde(default)]
    pub relation_type: crate::domain::link::RelationType,
    #[serde(default)]
    pub direction: crate::domain::link::RelationDirection,
    #[serde(default = "default_evidence_json")]
    pub evidence_json: serde_json::Value,
    #[serde(default = "default_created_at")]
    pub created_at: String,
    #[serde(default = "default_creator")]
    pub creator: String,
}

fn default_evidence_json() -> serde_json::Value {
    serde_json::json!({})
}
fn default_created_at() -> String {
    String::new()
}
fn default_creator() -> String {
    "legacy".to_string()
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectIdentity {
    pub authority: String,
    pub native_id: String,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ObjectIdentityError {
    #[error("invalid object identity: expected `authority::native_id`, got `{0}`")]
    InvalidFormat(String),
}

impl Default for ObjectIdentity {
    fn default() -> Self {
        Self {
            authority: String::new(),
            native_id: String::new(),
        }
    }
}

impl ObjectIdentity {
    pub fn new(authority: impl Into<String>, native_id: impl Into<String>) -> Self {
        Self {
            authority: authority.into(),
            native_id: native_id.into(),
        }
    }

    /// True when this identity carries both an authority and a native id.
    pub fn is_known(&self) -> bool {
        !self.authority.is_empty() && !self.native_id.is_empty()
    }

    /// Parse the canonical `authority::native_id` form.
    pub fn parse(s: &str) -> Result<Self, ObjectIdentityError> {
        let (authority, native_id) = s
            .split_once("::")
            .ok_or_else(|| ObjectIdentityError::InvalidFormat(s.to_string()))?;
        if authority.is_empty() || native_id.is_empty() {
            return Err(ObjectIdentityError::InvalidFormat(s.to_string()));
        }
        Ok(Self::new(authority, native_id))
    }

    /// The public `notez://object/…` URL form.
    pub fn to_notez_uri(&self) -> String {
        if self.is_known() {
            format!("notez://object/{self}")
        } else {
            String::new()
        }
    }

    /// Parse a `notez://object/authority::native_id` URL.
    pub fn parse_notez_uri(s: &str) -> Result<Self, ObjectIdentityError> {
        let rest = s
            .strip_prefix("notez://object/")
            .ok_or_else(|| ObjectIdentityError::InvalidFormat(s.to_string()))?;
        Self::parse(rest)
    }
}

impl fmt::Display for ObjectIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_known() {
            write!(f, "{}::{}", self.authority, self.native_id)
        } else {
            f.write_str("")
        }
    }
}

impl Serialize for ObjectIdentity {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ObjectIdentity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        if s.is_empty() {
            return Ok(ObjectIdentity::default());
        }
        ObjectIdentity::parse(&s).map_err(de::Error::custom)
    }
}

/// v1 派生：SHA-256( "notez-derived-object-id-v1" || content_hash || locator || position )。
///
/// `source_id` 不参与 — 同一文件被多 Space 扫描时 `content_hash` 来自同一正文，
/// 输出同一身份。`authority` 固定为 `local`（本地文件无 source 原生全局 ID，
/// 用派生指纹兜底）。
pub fn derived_object_id(content_hash: &str, locator: &str, position: &str) -> ObjectIdentity {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(b"notez-derived-object-id-v1\0");
    hasher.update(content_hash.as_bytes());
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
    ObjectIdentity::new("local", id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_id_is_stable_for_same_inputs() {
        let a = derived_object_id("hash-a", "notes/x.md", "h:0");
        let b = derived_object_id("hash-a", "notes/x.md", "h:0");
        assert_eq!(a, b);
        assert_eq!(a.authority, "local");
        assert!(a.is_known());
    }

    #[test]
    fn object_id_changes_when_content_hash_changes() {
        let a = derived_object_id("hash-a", "notes/x.md", "h:0");
        let b = derived_object_id("hash-b", "notes/x.md", "h:0");
        assert_ne!(a, b);
    }

    #[test]
    fn object_id_changes_when_position_changes() {
        let a = derived_object_id("hash-a", "notes/x.md", "h:0");
        let b = derived_object_id("hash-a", "notes/x.md", "h:1");
        assert_ne!(a, b);
    }

    #[test]
    fn object_id_parse_and_display_roundtrip() {
        let id = derived_object_id("hash", "loc", "pos");
        let s = id.to_string();
        let parsed = ObjectIdentity::parse(&s).expect("parse");
        assert_eq!(id, parsed);
    }

    #[test]
    fn object_identity_notez_uri_roundtrip() {
        let id = ObjectIdentity::new("notion", "8f3a2b");
        assert_eq!(id.to_string(), "notion::8f3a2b");
        assert_eq!(id.to_notez_uri(), "notez://object/notion::8f3a2b");
        let parsed = ObjectIdentity::parse_notez_uri("notez://object/notion::8f3a2b").unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn object_identity_default_is_unknown() {
        let id = ObjectIdentity::default();
        assert!(!id.is_known());
        assert_eq!(id.to_string(), "");
    }
}
