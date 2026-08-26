//! Org-mode format adapter.
//!
//! The `OrgParser` is a [`FormatParser`] implementation that interprets
//! in-memory Org bytes carried by a [`RawEntity`]. It never reads the
//! `locator` from disk; that responsibility belongs to a `SourceTransport`.
//!
//! Re-exports the `core::document::OrgScanner` API so the parser logic can
//! still be invoked with a filesystem `Path` when needed (legacy callers),
//! but the canonical path is `parse_bytes` for transports.

pub use notez_core::document::{OrgDocumentError as DocumentError, OrgScanner};

use notez_core::source::{FormatParser, ParsedEntity, ParserError, RawEntity};

/// Adapter parser that maps an Org `RawEntity` to the canonical
/// `ParsedEntity` model used by `core::source`.
pub struct OrgParser;

impl OrgParser {
    /// Construct a new parser instance. Stateless; provided for symmetry
    /// with future stateful adapters.
    pub fn new() -> Self {
        Self
    }
}
impl OrgParser {
    /// Read `path` and parse its bytes. Adapters are the only place
    /// that owns filesystem IO so that the core scanners stay pure
    /// and the application layer can run under wasm.
    pub fn parse_path(
        &self,
        path: &std::path::Path,
        source_id: &str,
    ) -> Result<notez_core::document::ScannedDocument, notez_core::document::OrgDocumentError>
    {
        let path_str = path.to_string_lossy().to_string();
        let bytes = std::fs::read(path).map_err(|e| {
            notez_core::document::OrgDocumentError::Io {
                path: path_str.clone(),
                kind: e.kind(),
            }
        })?;
        notez_core::document::OrgScanner::parse_bytes(&bytes, source_id, &path_str)
    }
}

impl Default for OrgParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParser for OrgParser {
    fn supports(&self, mime_type: &str) -> bool {
        mime_type == "text/org"
    }

    fn parse(
        &self,
        entity: &RawEntity,
        source_id: &str,
    ) -> Result<ParsedEntity, ParserError> {
        let doc = OrgScanner::parse_bytes(&entity.payload, source_id, &entity.locator)
            .map_err(|e| ParserError::Format(e.to_string()))?;
        Ok(ParsedEntity {
            resources: doc.resources,
            relations: doc.links,
            link_occurrences: doc.link_occurrences,
        })
    }
}
