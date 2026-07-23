pub mod adapter;
pub mod anytype;
pub use adapter::{PreparedWrite, SourceCapabilities, WriteResult};
pub use anytype::AnytypeSourceAdapter;
pub mod git;
pub mod native;
pub mod obsidian;

pub use adapter::{ScannedSource, SourceAdapter, SourceConfig, SourceError, SourceKind};
pub use git::GitSourceAdapter;
pub use native::NativeSourceAdapter;
pub use obsidian::ObsidianSourceAdapter;
