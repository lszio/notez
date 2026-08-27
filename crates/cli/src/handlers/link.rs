//! Handlers for `Commands::Link` (`LinkCommands::{List, Resolved,
//! Diagnose, Reindex}`).
//!
//! Every arm translates the parsed CLI syntax into a protocol
//! `Request`, dispatches it through the [`ApplicationDispatcher`], and
//! renders the unwrapped response payload.

use serde_json::json;
use std::process::exit;

use super::{Service, exit_code_for};
use crate::commands::{LinkCommands, LinkSubcommand};
use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_protocol::request::{
    DiagnoseLinkRequest, LinkOccurrencesRequest, ReindexLinksRequest, Request,
    ResolvedRelationsRequest,
};

pub fn run_link(json: bool, service: &mut Service, sub: LinkSubcommand) {
    let mut dispatcher = ApplicationDispatcher::new(service);
    match sub.command {
        LinkCommands::List { r_ref } => {
            match dispatcher.dispatch(Request::LinkOccurrences(LinkOccurrencesRequest {
                source_ref: r_ref,
            })) {
                Ok(Response::Occurrences(occs)) => {
                    if json {
                        println!("{}", json!(occs));
                    } else {
                        for occ in occs {
                            println!(
                                "{}:{}-{} -> {:?}",
                                occ.span.line, occ.span.col_start, occ.span.col_end, occ.target
                            );
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error querying occurrences: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
        LinkCommands::Resolved { r_ref } => {
            match dispatcher.dispatch(Request::ResolvedRelations(ResolvedRelationsRequest {
                source_ref: r_ref,
            })) {
                Ok(Response::Relations(rels)) => {
                    if json {
                        println!("{}", json!(rels));
                    } else {
                        for rel in rels {
                            println!("{:?} -> {} ({:?})", rel.target, rel.target_ref, rel.status);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error querying resolved relations: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
        LinkCommands::Diagnose { r_ref } => {
            match dispatcher.dispatch(Request::DiagnoseLink(DiagnoseLinkRequest {
                source_ref: r_ref,
            })) {
                Ok(Response::Diagnostics(diags)) => {
                    if json {
                        println!("{}", json!(diags));
                    } else {
                        for d in diags {
                            println!(
                                "L{} {:?} (candidates={})",
                                d.occurrence.span.line,
                                d.occurrence.target,
                                d.candidates.len()
                            );
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error diagnosing links: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
        LinkCommands::Reindex => {
            match dispatcher.dispatch(Request::ReindexLinks(ReindexLinksRequest {})) {
                Ok(Response::Reindex(report)) => {
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
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
    }
}
