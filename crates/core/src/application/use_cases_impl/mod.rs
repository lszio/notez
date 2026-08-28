//! Per-trait implementations for `Engine`.
//!
//! Each module implements one application use-case trait for `Engine<S>`.
//! The service module contains the engine state and constructors; behavior
//! remains split by capability for independent evolution.
pub mod artifact;
pub mod attachment;
pub mod community;
pub mod inspect;
pub mod link;
pub mod resource;
pub mod scan;
pub mod sync;
pub mod task;
