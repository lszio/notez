//! Handler for `Commands::Attachment` (`AttachmentCommands::{Add,
//! Extract, Segments}`).
//!
//! Every arm translates the parsed CLI syntax into a protocol
//! `Request`, dispatches it through the [`ApplicationDispatcher`], and
//! renders the unwrapped response payload.

use serde_json::json;
use std::process::exit;

use super::{Service, exit_code_for};
use crate::commands::{AttachmentCommands, AttachmentSubcommand};
use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_protocol::request::{
    AddAttachmentRequest, ExtractAttachmentRequest, QuerySegmentsRequest, Request,
};

pub fn run_attachment(json: bool, service: &mut Service, sub: AttachmentSubcommand) {
    let mut dispatcher = ApplicationDispatcher::new(service);
    match sub.command {
        AttachmentCommands::Add { path, mime } => {
            match dispatcher.dispatch(Request::AddAttachment(AddAttachmentRequest {
                file_path: path.display().to_string(),
                mime: Some(mime),
            })) {
                Ok(Response::AttachmentRef(att_ref)) => {
                    if json {
                        println!("{}", json!({ "ref": att_ref.to_string() }));
                    } else {
                        println!("{att_ref}");
                    }
                }
                Err(e) => {
                    eprintln!("Add attachment error: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }

        AttachmentCommands::Extract { r_ref } => {
            match dispatcher.dispatch(Request::ExtractAttachment(ExtractAttachmentRequest {
                source_ref: r_ref.clone(),
            })) {
                Ok(Response::Segments(segments)) => {
                    if json {
                        println!(
                            "{}",
                            json!({
                                "attachment_ref": r_ref,
                                "segments_count": segments.len()
                            })
                        );
                    } else {
                        println!("Extracted {} segments for {r_ref}.", segments.len());
                    }
                }
                Err(e) => {
                    eprintln!("Extract attachment error: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }

        AttachmentCommands::Segments { r_ref } => {
            match dispatcher.dispatch(Request::QuerySegments(QuerySegmentsRequest {
                source_ref: r_ref,
            })) {
                Ok(Response::Segments(segments)) => {
                    if json {
                        println!("{}", serde_json::to_string(&segments).unwrap());
                    } else {
                        for seg in segments {
                            println!("[{}-{}] {}", seg.offset_start, seg.offset_end, seg.text);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Query segments error: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
    }
}
