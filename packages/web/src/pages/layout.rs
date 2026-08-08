//! `use_space_layout` — hook each per-space page calls to validate
//! the active space and populate the `space_ctx` signal.
//!
//! The Dioxus 0.7 router does not support a router-level layout for
//! dynamic-segment routes without `#[nest]` (which has its own
//! macro-grammar quirks), so each per-space page calls this hook as
//! its first line. The hook:
//! 1. Decodes the `encoded` path segment into a filesystem path.
//! 2. Calls `selected_space` server fn to validate and fetch the DTO.
//! 3. Populates the `space_ctx` context the layout reads for the
//!    picker and the error banner.
//!
//! After the hook returns, `use_context::<Signal<Option<SpaceState>>>()`
//! is reactive — pages can read it for their data fetch and re-render
//! when the user navigates to a new space.

use dioxus::prelude::*;

use crate::server::{resolve_space_path, selected_space, WebServerError};
use crate::space_ctx::{SpaceState, SpaceStatus};

fn decode_segment(encoded: &str) -> String {
    crate::router::decode_space(encoded)
}
/// Run validation. Idempotent within a single render pass; the
/// `use_future` re-runs whenever `encoded` changes (the captured
/// `String` is the reactive dep).
pub fn use_space_layout(encoded: &str) {
    let space_ctx = use_context::<Signal<Option<SpaceState>>>();

    let enc = encoded.to_string();
    use_future(move || {
        let enc = enc.clone();
        let mut space_ctx = space_ctx.clone();
        async move {
            let path = decode_segment(&enc);
            space_ctx.set(Some(SpaceState::resolving(enc.clone(), path.clone())));

            match selected_space(path.clone()).await {
                Ok(dto) => {
                    space_ctx.set(Some(SpaceState {
                        encoded: enc.clone(),
                        path: path.clone(),
                        status: SpaceStatus::Ready(dto),
                    }));
                }
                Err(_) => match resolve_space_path(path.clone()).await {
                    Ok(_) => {
                        space_ctx.set(Some(SpaceState {
                            encoded: enc.clone(),
                            path: path.clone(),
                            status: SpaceStatus::Ready(
                                crate::server::SelectedSpaceDto {
                                    name: "(unnamed)".into(),
                                    path: path.clone(),
                                    db_path: String::new(),
                                },
                            ),
                        }));
                    }
                    Err(e) => {
                        let err = WebServerError::Internal { message: e.to_string() };
                        space_ctx.set(Some(SpaceState {
                            encoded: enc.clone(),
                            path: path.clone(),
                            status: SpaceStatus::Error(err),
                        }));
                    }
                },
            }
        }
    });
}
