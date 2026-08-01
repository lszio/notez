//! `cli` — notez command-line interface and MCP stdio server.
//!
//! The binary at `src/main.rs` is a thin wrapper that exercises the
//! `commands` and (when the `mcp` feature is enabled) `mcp` modules
//! defined here. Exposing the modules as a library lets the
//! integration tests in `tests/` exercise the MCP server in-process
//! without re-spawning the binary.
//!
//! Library name: `notez_cli`.

pub mod commands;
pub mod mcp;
