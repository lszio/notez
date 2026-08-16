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
//! We use `use_server_future` (not `use_future`) so the SSR pass
//! actually waits for `selected_space` to resolve. With `use_future`
//! the SSR rendered before the future returned, leaving the layout
//! in `Resolving` state, which in turn left every downstream
//! `use_server_future` in the page seeing `space().active_path` as
//! `None` and returning an empty list.

use dioxus::prelude::*;

use crate::server::{resolve_space_path, selected_space, SelectedSpaceDto, WebServerError};
use crate::space_ctx::{SpaceState, SpaceStatus};

fn decode_segment(encoded: &str) -> String {
    crate::router::decode_space(encoded)
}

pub fn use_space_layout(encoded: &str) {
    let mut space_ctx = use_context::<Signal<Option<SpaceState>>>();

    let enc = encoded.to_string();
    let resolved = match use_server_future(move || {
        let enc = enc.clone();
        async move {
            let path = decode_segment(&enc);
            match selected_space(path.clone()).await {
                Ok(dto) => SpaceState {
                    encoded: enc,
                    path: path.clone(),
                    status: SpaceStatus::Ready(dto),
                },
                Err(_) => match resolve_space_path(path.clone()).await {
                    Ok(_) => SpaceState {
                        encoded: enc,
                        path: path.clone(),
                        status: SpaceStatus::Ready(SelectedSpaceDto {
                            name: "(unnamed)".into(),
                            path: path.clone(),
                            db_path: String::new(),
                        }),
                    },
                    Err(e) => SpaceState {
                        encoded: enc,
                        path: path.clone(),
                        status: SpaceStatus::Error(WebServerError::Internal {
                            message: e.to_string(),
                        }),
                    },
                },
            }
        }
    }) {
        Ok(r) => r,
        Err(e) => panic!("server future returned RenderError: {e:?}"),
    };
    if let Some(state) = resolved.cloned() {
        space_ctx.set(Some(state));
    }
}
