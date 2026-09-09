//! Structured errors shared by every Notez transport.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Error {
    NotFound {
        target: String,
    },
    Unavailable {
        source: String,
        message: String,
    },
    Unauthorized {
        message: String,
    },
    UnsupportedCapability {
        capability: String,
    },
    StaleRevision {
        expected: String,
        actual: String,
    },
    /// The source or adapter is temporarily unavailable and may be retried.
    SourceUnavailable {
        source: String,
        message: String,
    },
    /// The caller lacks permission for the requested operation.
    Forbidden {
        message: String,
    },
    /// A transient operation failure that is safe to retry.
    Retryable {
        message: String,
    },
    InvalidPatch {
        message: String,
    },
    Conflict {
        target: String,
        message: String,
    },
    ScriptSyntax {
        message: String,
    },
    ScriptRuntime {
        message: String,
    },
    ScriptTimeout,
    ResultTooLarge {
        limit: usize,
    },
    InvalidRequest {
        message: String,
    },
    Internal {
        diagnostic_id: String,
    },
}
