use crate::source::adapter::{
    ComposedSourceAdapter, ScannedSource, SourceAdapter, SourceCapabilities, SourceConfig,
    SourceError,
};
use crate::source::protocol::{
    FormatParser, ParsedEntity, ParserError, RawEntity, SourceTransport, TransportError,
};

pub struct AppleNotesSourceAdapter {
    inner: ComposedSourceAdapter,
}

pub struct JxaNotesTransport;

impl SourceTransport for JxaNotesTransport {
    fn fetch_raw(&self) -> Result<Vec<RawEntity>, TransportError> {
        // A complete implementation executes `osascript` (JXA or AppleScript)
        // to query the Notes.app SQLite database or CloudKit API. Until that
        // lands, fail loudly instead of silently scanning to an empty source.
        Err(TransportError::Other(
            "apple_notes transport is not implemented yet; osascript integration pending".into(),
        ))
    }

    fn mutate(&self, _locator: &str, _payload: &str) -> Result<(), TransportError> {
        // The JXA/osascript integration is not wired yet. Refuse instead of
        // reporting a fake successful mutation.
        Err(TransportError::Other(
            "apple_notes transport cannot mutate yet; osascript integration pending".into(),
        ))
    }
}

pub struct AppleNotesParser;

impl FormatParser for AppleNotesParser {
    fn supports(&self, mime_type: &str) -> bool {
        mime_type == "application/vnd.apple.notes+json"
    }

    fn parse(&self, _entity: &RawEntity, _source_id: &str) -> Result<ParsedEntity, ParserError> {
        // Parse the JSON payload from JxaNotesTransport into checklist
        // Resources. Unreachable until the transport above is implemented.
        Ok(ParsedEntity {
            resources: vec![],
            relations: vec![],
            link_occurrences: vec![],
        })
    }
}

impl AppleNotesSourceAdapter {
    pub fn new(config: SourceConfig) -> Self {
        let transport = JxaNotesTransport;
        let parsers: Vec<Box<dyn FormatParser>> = vec![Box::new(AppleNotesParser)];

        let inner = ComposedSourceAdapter::new(config, Box::new(transport), parsers);
        Self { inner }
    }
}

impl SourceAdapter for AppleNotesSourceAdapter {
    fn config(&self) -> &SourceConfig {
        self.inner.config()
    }

    /// The composed transport is a skeleton; report honestly instead of
    /// inheriting `ComposedSourceAdapter`'s optimistic read capability.
    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities {
            can_read: false,
            can_write: false,
            can_import: false,
            can_watch: false,
        }
    }

    fn scan(&self) -> Result<ScannedSource, SourceError> {
        self.inner.scan()
    }
}
