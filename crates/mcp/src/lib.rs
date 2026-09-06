//! `mcp` — the notez MCP server.
//!
//! Transport-agnostic rmcp handler: every tool deserializes its raw
//! JSON arguments into the matching [`notez_protocol::Request`] struct
//! and hands it to the engine dispatcher. Input schemas advertised via
//! `tools/list` are generated from the protocol request types.
//!
//! The handler itself does not know about any transport. Surfaces pick
//! one:
//!
//! * [`serve`] — MCP over stdin/stdout (local CLI / agent subprocess);
//! * the web host mounts the same handler over MCP streamable HTTP so a
//!   remote agent can reach a running notez server (see
//!   `packages/web` host assembly).
//!
//! Library name: `notez_mcp`.

pub mod server;
pub use server::{NotezMcpServer, serve};
