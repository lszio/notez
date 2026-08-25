//! Command-family handlers for the `notez` binary.
//!
//! Each submodule owns one family of `Commands` variants, including its
//! rendering (JSON vs. human table). `main.rs` stays a thin
//! initialise-and-dispatch wrapper. Known debt: the render layer is not
//! yet factored out; JSON-vs-table logic lives inline in each handler.

pub mod attachment;
pub mod community;
pub mod link;
pub mod mcp_cmd;
pub mod resource;
pub mod scan;
pub mod source;
pub mod sync;
pub mod task;
pub mod watch;
pub mod workspace;

/// The composed application service handed to every command handler.
pub type Service =
    notez_core::application::ApplicationFacade<notez_core::storage::SqliteProjection>;

/// Map an `ApplicationError` to a stable process exit code.
///
/// The mapping is part of the CLI contract and must not change without
/// a coordinated release (see the error design spec).
pub fn exit_code_for(err: &notez_core::application::ApplicationError) -> i32 {
    use notez_core::application::ApplicationError;
    match err {
        ApplicationError::NotFound { .. } => 3,
        ApplicationError::Storage { .. } => 5,
        ApplicationError::Document { .. } => 5,
        ApplicationError::Io { .. } => 5,
        ApplicationError::UnsupportedCapability { .. } => 6,
        ApplicationError::ReadOnlySource { .. } => 7,
        ApplicationError::SourceNotFound { .. } => 8,
        ApplicationError::RevisionConflict { .. } => 9,
        ApplicationError::AddressUniqueness { .. } => 10,
    }
}
