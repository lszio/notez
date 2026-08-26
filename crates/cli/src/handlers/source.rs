//! Handler for `Commands::Source` (`SourceCommands::{Add, List, Sync,
//! Writeback}`).
//!
//! `Add`/`List` are pure source-config operations with no protocol
//! counterpart, so they keep calling the facade directly. `Sync` and
//! `Writeback` are domain operations and route through the
//! [`ApplicationDispatcher`].

use serde_json::json;
use std::process::exit;

use super::{Service, exit_code_for};
use crate::commands::{SourceCommands, SourceSubcommand};
use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_protocol::request::{Request, ScanFederationRequest, WritebackResourceRequest};

pub fn run_source(json: bool, service: &mut Service, sub: SourceSubcommand) {
    let mut dispatcher = ApplicationDispatcher::new(service);
    match sub.command {
        SourceCommands::Add {
            id,
            kind,
            path,
            read_only,
            include,
            exclude,
        } => {
            let config = notez_core::source::SourceConfig {
                id,
                kind: kind.into(),
                path,
                read_only,
                include_paths: include,
                exclude_paths: exclude,
            };
            match service.add_source(config) {
                Ok(_) => {
                    if json {
                        println!("{}", json!({ "added": true }));
                    } else {
                        println!("Source added successfully.");
                    }
                }
                Err(e) => {
                    eprintln!("Add source error: {e}");
                    exit(exit_code_for(&e));
                }
            }
        }

        SourceCommands::List => match service.list_sources() {
            Ok(sources) => {
                if json {
                    println!("{}", serde_json::to_string(&sources).unwrap());
                } else {
                    for src in sources {
                        println!("{} ({:?}): {:?}", src.id, src.kind, src.path);
                    }
                }
            }
            Err(e) => {
                eprintln!("List sources error: {e}");
                exit(exit_code_for(&e));
            }
        },

        SourceCommands::Sync => {
            match dispatcher.dispatch(Request::ScanFederation(ScanFederationRequest {})) {
                Ok(Response::Scan(report)) => {
                    if json {
                        println!(
                            "{}",
                            json!({
                                "synced": true,
                                "scanned_files": report.scanned_files,
                                "scanned_resources": report.scanned_resources,
                                "scanned_relations": report.scanned_relations,
                            })
                        );
                    } else {
                        println!(
                            "Synced sources: {} resources, {} relations.",
                            report.scanned_resources, report.scanned_relations
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Source sync error: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }

        SourceCommands::Writeback { id, r_ref, payload } => {
            match dispatcher.dispatch(Request::WritebackResource(WritebackResourceRequest {
                source_id: id,
                r_ref,
                payload,
            })) {
                Ok(Response::Writeback(report)) => {
                    if json {
                        println!(
                            "{}",
                            json!({
                                "target_ref": report.target_ref,
                                "committed": report.committed
                            })
                        );
                    } else {
                        println!("Writeback committed: {}", report.committed);
                    }
                }
                Err(e) => {
                    eprintln!("Source writeback error: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
    }
}
