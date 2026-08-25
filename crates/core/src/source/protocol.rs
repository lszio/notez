use crate::domain::{LinkOccurrence, Resource, ResourceRelation};
use std::fmt;

/// An abstract physical block of data, agnostic to whether it came from a
/// file system, HTTP request, or AppleScript RPC.
#[derive(Debug, Clone)]
pub struct RawEntity {
    pub locator: String,   // E.g. relative file path, x-coredata ID, URL
    pub mime_type: String, // E.g. text/org, text/markdown, application/json
    pub payload: Vec<u8>,  // The raw bytes of the entity
}

#[derive(Debug, Clone)]
pub struct ParsedEntity {
    pub resources: Vec<Resource>,
    pub relations: Vec<ResourceRelation>,
    pub link_occurrences: Vec<LinkOccurrence>,
}

#[derive(Debug)]
pub enum TransportError {
    Io(std::io::Error),
    Other(String),
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportError::Io(e) => write!(f, "Transport I/O error: {}", e),
            TransportError::Other(msg) => write!(f, "Transport error: {}", msg),
        }
    }
}

impl std::error::Error for TransportError {}

#[derive(Debug)]
pub enum ParserError {
    Format(String),
    Other(String),
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParserError::Format(msg) => write!(f, "Format parse error: {}", msg),
            ParserError::Other(msg) => write!(f, "Parser error: {}", msg),
        }
    }
}

impl std::error::Error for ParserError {}

/// Resolves "where to get data" and "how to mutate it".
pub trait SourceTransport: Send + Sync {
    /// Iterates the source and produces a stream of raw physical entities.
    fn fetch_raw(&self) -> Result<Vec<RawEntity>, TransportError>;

    /// Optional mutation capability.
    fn mutate(&self, locator: &str, payload: &str) -> Result<(), TransportError> {
        let _ = (locator, payload);
        Err(TransportError::Other(
            "Mutation not supported by this transport".into(),
        ))
    }
}

/// Resolves "how to understand data".
pub trait FormatParser: Send + Sync {
    /// Checks if this parser can handle the given mime type.
    fn supports(&self, mime_type: &str) -> bool;

    /// Parses a single raw entity into the standardized semantic model.
    fn parse(&self, entity: &RawEntity, source_id: &str) -> Result<ParsedEntity, ParserError>;
}
