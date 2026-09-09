use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self { Self(value.into()) }
            pub fn as_str(&self) -> &str { &self.0 }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self { Self(value) }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self { Self::new(value) }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
        }
    };
}

id_type!(SpaceId);
id_type!(SourceId);
id_type!(DocumentId);
id_type!(ObjectId);
id_type!(Revision);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentFingerprint(String);

impl ContentFingerprint {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        // FNV-1a keeps the domain crate dependency-light; this is a matching
        // fingerprint, not a cryptographic integrity claim.
        let mut hash = 0xcbf29ce484222325u64;
        for byte in bytes { hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3); }
        Self(format!("fnv1a:{hash:016x}"))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePath(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourcePathError {
    #[error("source path must be relative")]
    Absolute,
    #[error("source path escapes its root")]
    Escape,
    #[error("source path is empty")]
    Empty,
}

impl SourcePath {
    pub fn parse(path: &str) -> Result<Self, SourcePathError> {
        if path.is_empty() { return Err(SourcePathError::Empty); }
        let normalized = path.replace('\\', "/");
        if normalized.starts_with('/') || normalized.starts_with("//") ||
            (normalized.len() >= 3 && normalized.as_bytes()[1] == b':' && normalized.as_bytes()[2] == b'/')
        {
            return Err(SourcePathError::Absolute);
        }
        let mut parts = Vec::new();
        for part in normalized.split('/') {
            match part { "" | "." => {}, ".." => return Err(SourcePathError::Escape), value => parts.push(value) }
        }
        if parts.is_empty() { return Err(SourcePathError::Empty); }
        Ok(Self(parts.join("/")))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockLocator { pub start_line: u32, pub end_line: u32 }

impl BlockLocator {
    pub fn line_span(start_line: u32, end_line: u32) -> Self { Self { start_line, end_line } }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionedRef {
    pub space_id: SpaceId,
    pub source_id: SourceId,
    pub document_path: SourcePath,
    pub locator: BlockLocator,
    pub fingerprint: ContentFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectAddress { Stable(ObjectId), Positioned(PositionedRef) }

impl ObjectAddress {
    pub fn source_id(&self) -> Option<&SourceId> {
        match self { Self::Stable(_) => None, Self::Positioned(value) => Some(&value.source_id) }
    }
    pub fn document_path(&self) -> Option<&SourcePath> {
        match self { Self::Stable(_) => None, Self::Positioned(value) => Some(&value.document_path) }
    }
}
