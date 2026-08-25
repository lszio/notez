//! Handler for `Commands::Attachment` (`AttachmentCommands::{Add,
//! Extract, Segments}`).

use serde_json::json;
use std::process::exit;

use super::{Service, exit_code_for};
use crate::commands::{AttachmentCommands, AttachmentSubcommand};
use notez_core::application::use_cases::AttachmentUseCase;
use notez_core::domain::ResourceRef;

pub fn run_attachment(json: bool, service: &mut Service, sub: AttachmentSubcommand) {
    match sub.command {
        AttachmentCommands::Add { path, mime } => {
            match AttachmentUseCase::add_attachment(service, &path, &mime) {
                Ok(att_ref) => {
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
            }
        }

        AttachmentCommands::Extract { r_ref } => {
            let parsed_ref = match ResourceRef::parse(&r_ref) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Invalid resource ref format '{r_ref}': {e}");
                    exit(2);
                }
            };

            match AttachmentUseCase::run_extraction(service, &parsed_ref) {
                Ok(segments) => {
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
            }
        }

        AttachmentCommands::Segments { r_ref } => {
            let parsed_ref = match ResourceRef::parse(&r_ref) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Invalid resource ref format '{r_ref}': {e}");
                    exit(2);
                }
            };

            match AttachmentUseCase::query_segments(service, &parsed_ref) {
                Ok(segments) => {
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
            }
        }
    }
}
