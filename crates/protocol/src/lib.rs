//! # notez-protocol
//!
//! The single wire vocabulary for every Notez surface: CLI, MCP,
//! HTTP API, and web clients all translate their native syntax into
//! the [`request::Request`] values defined here, and engines
//! (`core::application::dispatcher`) translate them into use-case
//! calls.
//!
//! Design constraints (docs/architecture-v2.org §4.3):
//!
//! * **Dependency-light** — only `serde` + `schemars`. No domain
//!   types, no I/O, no engine. Compilable for wasm32 clients.
//! * **One operation, one struct** — a new query filter or flag is a
//!   field on exactly one request struct, never a parallel parameter
//!   list on three surfaces.
//! * **Stringly identifiers** — refs, kinds, paths arrive as strings;
//!   the engine parses and rejects them with structured errors so
//!   every surface shares one validation behavior.
//!
//! Response vocabulary is fully typed: see [`response::Response`] —
//! one variant per operation, payload structs mirroring the engine's
//! domain DTO shapes with identical JSON field names.

pub mod request;
pub mod response;
pub mod schema;

pub use request::Request;
pub use response::{DocumentUpdateReport, Response};
