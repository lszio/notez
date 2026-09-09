use notez_ports as ports;

mod authorization;
mod commands;
mod dispatcher;
mod ingestion;

pub use authorization::{Action, AllowList, Authorization, AuthorizationError, Principal, Scope, Space, Source};
pub use commands::Command;
pub use dispatcher::{ApplicationError, Dispatcher, EnginePorts};
pub use ingestion::IngestionReceipt;
pub use notez_protocol::Query;
pub use notez_protocol::response::{CommandResult, Response};

/// A source adapter used by the write spine. The combined trait keeps the
/// public dispatcher independent from filesystem and adapter implementations.
pub trait SourcePort: ports::SourceReader + ports::SourceWriter {}
impl<T: ports::SourceReader + ports::SourceWriter> SourcePort for T {}

/// Shared port bundle accepted by the application dispatcher.
pub struct PortBundle {
    pub source: Option<std::sync::Arc<dyn SourcePort>>,
    pub journal: Option<std::sync::Arc<dyn ports::ChangeJournal>>,
    pub audit: Option<std::sync::Arc<dyn ports::AuditLog>>,
    pub projection: Option<std::sync::Arc<dyn ports::ProjectionReader>>,
}

impl Default for PortBundle {
    fn default() -> Self { Self { source: None, journal: None, audit: None, projection: None } }
}

pub use PortBundle as Ports;
