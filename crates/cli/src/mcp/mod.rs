//! notez MCP stdio server (rmcp-based, MCP spec 2025-11-25).
//!
//! Built into the `cli` crate behind the `mcp` cargo feature so the
//! binary can serve `notez mcp serve` over stdin/stdout while the
//! rest of the CLI stays free of the rmcp dependency.

pub mod server;
pub use server::{NotezMcpServer, serve};
