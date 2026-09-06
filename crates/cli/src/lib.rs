//! `cli` — notez command-line interface.
//!
//! The binary at `src/main.rs` is a thin wrapper that exercises the
//! `commands` and `handlers` modules defined here. Exposing them as a
//! library lets integration tests in `tests/` drive CLI behaviours
//! in-process.
//!
//! Library name: `notez_cli`.

pub mod commands;
pub mod handlers;
pub mod host;
