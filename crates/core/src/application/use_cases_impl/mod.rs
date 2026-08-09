//! Per-trait impl blocks for `ApplicationFacade`.
//!
//! Each mod file in this directory implements one of the nine
//! `core::application::use_cases::*UseCase` traits for
//! `ApplicationFacade<S>`. Method bodies were previously inlined in
//! `service.rs`; moving them here is the structural split promised by
//! the 0.5.x-A1+A3 spec
//! (`docs/superpowers/specs/2026-08-09-0.5x-a1-a3-usecase-impl-split-and-write-checks-design.org`).
//!
//! `service.rs` keeps the 44 `pub fn` forwarders and the
//! `ApplicationFacade` struct + constructors. Each forwarder is a
//! one-line `<Self as TraitUseCase>::fn(self, ...)` dispatch.
pub mod scan;
pub mod artifact;
pub mod attachment;
pub mod community;
pub mod inspect;
pub mod link;
pub mod resource;
pub mod sync;
pub mod task;
