use crate::source::adapter::{
    ComposedSourceAdapter, ScannedSource, SourceAdapter, SourceCapabilities, SourceConfig,
    SourceError,
};
use crate::source::protocol::{
    FormatParser, ParsedEntity, ParserError, RawEntity, SourceTransport, TransportError,
};

pub struct AppleCalendarSourceAdapter {
    inner: ComposedSourceAdapter,
}

pub struct EventKitTransport;

impl SourceTransport for EventKitTransport {
    fn fetch_raw(&self) -> Result<Vec<RawEntity>, TransportError> {
        // A complete implementation calls a Swift binary or OSAScript to
        // query EventKit for calendar events. Until that lands, fail loudly
        // instead of silently scanning to an empty source.
        Err(TransportError::Other(
            "apple_calendar transport is not implemented yet; EventKit integration pending".into(),
        ))
    }

    fn mutate(&self, _locator: &str, _payload: &str) -> Result<(), TransportError> {
        // The EventKit integration is not wired yet. Refuse instead of
        // reporting a fake successful mutation.
        Err(TransportError::Other(
            "apple_calendar transport cannot mutate yet; EventKit integration pending".into(),
        ))
    }
}

pub struct CalendarParser;

impl FormatParser for CalendarParser {
    fn supports(&self, mime_type: &str) -> bool {
        mime_type == "application/vnd.apple.calendar+json"
    }

    fn parse(&self, _entity: &RawEntity, _source_id: &str) -> Result<ParsedEntity, ParserError> {
        // Parse the JSON payload from EventKitTransport into scheduled /
        // deadline Resources. Unreachable until the transport above is
        // implemented.
        Ok(ParsedEntity {
            resources: vec![],
            relations: vec![],
            link_occurrences: vec![],
        })
    }
}

impl AppleCalendarSourceAdapter {
    pub fn new(config: SourceConfig) -> Self {
        let transport = EventKitTransport;
        let parsers: Vec<Box<dyn FormatParser>> = vec![Box::new(CalendarParser)];

        let inner = ComposedSourceAdapter::new(config, Box::new(transport), parsers);
        Self { inner }
    }
}

impl SourceAdapter for AppleCalendarSourceAdapter {
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
