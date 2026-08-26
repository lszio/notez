//! Handlers for `Commands::Sync` (`SyncCommands::{Push, Pull,
//! Conflicts, Relay}`).
//!
//! Every arm translates the parsed CLI syntax into a protocol
//! `Request`, dispatches it through the [`ApplicationDispatcher`], and
//! renders the unwrapped response payload.

use serde_json::json;
use std::process::exit;

use super::{Service, exit_code_for};
use crate::commands::{SyncCommands, SyncSubcommand};
use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_protocol::request::{ListConflictsRequest, RelaySyncRequest, Request, SyncPullRequest, SyncPushRequest};

pub fn run_sync(json: bool, service: &mut Service, sub: SyncSubcommand) {
    // Resolve needs &mut service directly; every other arm goes
    // through the protocol dispatcher.
    if let SyncCommands::Resolve { path, keep, folder } = sub.command {
        let keep_mine = matches!(keep.as_str(), "mine");
        match <Service as notez_core::application::use_cases::SyncUseCase>::resolve_conflict(
            service,
            folder.as_path(),
            &path,
            keep_mine,
        ) {
            Ok(report) => {
                if json {
                    println!(
                        "{}",
                        json!({"resolved": report.target_ref, "committed": report.committed, "kept": keep})
                    );
                } else {
                    println!("Resolved {}: kept {}", report.target_ref, keep);
                }
            }
            Err(e) => {
                eprintln!("Resolve error: {e}");
                exit(exit_code_for(&e));
            }
        }
        return;
    }

    let mut dispatcher = ApplicationDispatcher::new(service);
    match sub.command {
        SyncCommands::Push { actor, folder } => {
            match dispatcher.dispatch(Request::SyncPush(SyncPushRequest {
                actor_id: actor,
                folder: folder.display().to_string(),
            })) {
                Ok(Response::Pushed(report)) => {
                    if json {
                        println!(
                            "{}",
                            json!({
                                "pushed_files": report.pushed_files,
                                "pushed_objects": report.pushed_objects
                            })
                        );
                    } else {
                        println!("Pushed {} files to shared folder.", report.pushed_files);
                    }
                }
                Err(e) => {
                    eprintln!("Sync push error: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }

        SyncCommands::Pull { actor, folder } => {
            match dispatcher.dispatch(Request::SyncPull(SyncPullRequest {
                actor_id: actor,
                folder: folder.display().to_string(),
            })) {
                Ok(Response::Pulled(report)) => {
                    if json {
                        println!(
                            "{}",
                            json!({
                                "pulled_files": report.pulled_files,
                                "merged_files": report.merged_files,
                                "conflicts_count": report.conflicts.len()
                            })
                        );
                    } else {
                        println!(
                            "Pulled {} files, merged {}.",
                            report.pulled_files, report.merged_files
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Sync pull error: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }

        SyncCommands::Conflicts => {
            match dispatcher.dispatch(Request::ListConflicts(ListConflictsRequest {})) {
                Ok(Response::Conflicts(conflicts)) => {
                    if json {
                        println!("{}", serde_json::to_string(&conflicts).unwrap());
                    } else {
                        for c in conflicts {
                            println!("Conflict in {}: {}", c.logical_path, c.conflict_text);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Sync conflicts error: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
        SyncCommands::Resolve { .. } => unreachable!("handled before dispatcher"),
        SyncCommands::Relay { id: _ } => {
            match dispatcher.dispatch(Request::RelaySync(RelaySyncRequest {})) {
                Ok(Response::Relay(report)) => {
                    if json {
                        println!(
                            "{}",
                            json!({
                                "source_id": report.source_id,
                                "synced_via_relay": report.synced_via_relay
                            })
                        );
                    } else {
                        println!(
                            "Relay sync complete for {} (synced: {})",
                            report.source_id, report.synced_via_relay
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Sync relay error: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
    }
}
