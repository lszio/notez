//! Structured errors shared by every Notez transport.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Error {
    NotFound { target: String },
    Unavailable { source: String, message: String },
    Unauthorized { message: String },
    UnsupportedCapability { capability: String },
    StaleRevision { expected: String, actual: String },
    InvalidPatch { message: String },
    Conflict { target: String, message: String },
    ScriptSyntax { message: String },
    ScriptRuntime { message: String },
    ScriptTimeout,
    ResultTooLarge { limit: usize },
    InvalidRequest { message: String },
    Internal { diagnostic_id: String },
}
