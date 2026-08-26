pub mod adapter;
pub mod writer;
pub mod protocol;
pub mod registry;
pub use protocol::{
    FormatParser, ParsedEntity, ParserError, RawEntity, SourceTransport, TransportError,
};
pub mod apple_calendar;
pub mod apple_notes;
pub use apple_calendar::AppleCalendarSourceAdapter;
pub use apple_notes::AppleNotesSourceAdapter;
pub mod anytype;
pub use adapter::{ComposedSourceAdapter, PreparedWrite, SourceCapabilities, WriteResult};
pub use anytype::AnytypeSourceAdapter;
pub mod git;
pub mod native;
pub mod obsidian;
pub use adapter::{ScannedSource, SourceAdapter, SourceConfig, SourceError, SourceKind};
pub use git::GitSourceAdapter;
pub use native::NativeSourceAdapter;
pub use obsidian::ObsidianSourceAdapter;
pub use registry::{SourceAdapterFactory, SourceRegistry};
