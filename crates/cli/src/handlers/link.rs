//! Handlers for `Commands::Link` (`LinkCommands::{List, Resolved,
//! Diagnose, Reindex}`).

use serde_json::json;
use std::process::exit;

use super::{Service, exit_code_for};
use crate::commands::{LinkCommands, LinkSubcommand};
use notez_core::application::use_cases::LinkUseCase;
use notez_core::domain::ResourceRef;

pub fn run_link(json: bool, service: &mut Service, sub: LinkSubcommand) {
    match sub.command {
        LinkCommands::List { r_ref } => match ResourceRef::parse(&r_ref) {
            Ok(parsed_ref) => match LinkUseCase::query_link_occurrences(service, &parsed_ref) {
                Ok(occs) => {
                    if json {
                        println!("{}", json!(occs));
                    } else {
                        for occ in occs {
                            println!(
                                "{}:{}-{} -> {}",
                                occ.span.line, occ.span.col_start, occ.span.col_end, occ.target
                            );
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error querying occurrences: {e}");
                    exit(exit_code_for(&e));
                }
            },
            Err(e) => {
                eprintln!("Invalid ref parameter: {e}");
                exit(2);
            }
        },
        LinkCommands::Resolved { r_ref } => match ResourceRef::parse(&r_ref) {
            Ok(parsed_ref) => match LinkUseCase::query_resolved_relations(service, &parsed_ref) {
                Ok(rels) => {
                    if json {
                        println!("{}", json!(rels));
                    } else {
                        for rel in rels {
                            println!("{} -> {} ({:?})", rel.target, rel.target_ref, rel.status);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error querying resolved relations: {e}");
                    exit(exit_code_for(&e));
                }
            },
            Err(e) => {
                eprintln!("Invalid ref parameter: {e}");
                exit(2);
            }
        },
        LinkCommands::Diagnose { r_ref } => match ResourceRef::parse(&r_ref) {
            Ok(parsed_ref) => match LinkUseCase::diagnose_link(service, &parsed_ref) {
                Ok(diags) => {
                    if json {
                        println!("{}", json!(diags));
                    } else {
                        for d in diags {
                            println!(
                                "L{} {} -> {:?} (candidates={})",
                                d.occurrence.span.line,
                                d.occurrence.target,
                                d.status,
                                d.candidates.len()
                            );
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error diagnosing links: {e}");
                    exit(exit_code_for(&e));
                }
            },
            Err(e) => {
                eprintln!("Invalid ref parameter: {e}");
                exit(2);
            }
        },
        LinkCommands::Reindex => match LinkUseCase::reindex_links(service) {
            Ok(report) => {
                if json {
                    println!("{}", json!(report));
                } else {
                    println!(
                        "scanned={} resolved={} unresolved={} ambiguous={} external={} invalid={}",
                        report.scanned,
                        report.resolved,
                        report.unresolved,
                        report.ambiguous,
                        report.external,
                        report.invalid
                    );
                }
            }
            Err(e) => {
                eprintln!("Error reindexing links: {e}");
                exit(exit_code_for(&e));
            }
        },
    }
}
