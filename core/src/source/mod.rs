pub mod adapter;
pub mod protocol;
pub use protocol::{FormatParser, ParsedEntity, RawEntity, SourceTransport, TransportError, ParserError};
pub mod apple_notes;
pub mod apple_calendar;
pub use apple_notes::AppleNotesSourceAdapter;
pub use apple_calendar::AppleCalendarSourceAdapter;
pub mod anytype;
pub use adapter::{PreparedWrite, SourceCapabilities, ComposedSourceAdapter, WriteResult};
pub use anytype::AnytypeSourceAdapter;
pub mod git;
pub mod native;
pub mod obsidian;

pub use adapter::{ScannedSource, SourceAdapter, SourceConfig, SourceError, SourceKind};
pub use git::GitSourceAdapter;
pub use native::NativeSourceAdapter;
pub use obsidian::ObsidianSourceAdapter;
