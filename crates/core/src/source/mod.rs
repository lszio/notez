pub mod adapter;
pub mod writer;
pub mod protocol;
pub mod policy;
pub use policy::{Decision, IgnoreCounts, IgnoreReason, SourcePolicy};
pub use protocol::{
    ChangeObserver, FormatParser, ParsedEntity, ParserError, RawEntity, SourceReader,
    SourceTransport, SourceWriter, TransportError,
};
pub mod registry;
pub mod apple_calendar;
pub mod apple_notes;
pub use apple_calendar::AppleCalendarSourceAdapter;
pub use apple_notes::AppleNotesSourceAdapter;
pub mod anytype;
pub mod notez_rest;
pub use notez_rest::NotezRestSourceAdapter;
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
