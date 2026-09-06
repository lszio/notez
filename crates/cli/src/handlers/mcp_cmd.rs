//! Handler for `Commands::Mcp` (`mcp serve`).
//!
//! The MCP server (notez_mcp) owns the composed service for the lifetime of the
//! process, so this handler takes it by value.

use std::process::exit;

use super::Service;

pub fn run_mcp(service: Service) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Tokio runtime error: {e}");
            exit(5);
        }
    };
    if let Err(e) = runtime.block_on(notez_mcp::serve(service)) {
        eprintln!("MCP server error: {e}");
        exit(5);
    }
}
