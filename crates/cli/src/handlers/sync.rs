//! Handlers for `Commands::Sync` (`SyncCommands::{Push, Pull,
//! Conflicts, Relay}`).

use serde_json::json;
use std::process::exit;

use super::{Service, exit_code_for};
use crate::commands::{SyncCommands, SyncSubcommand};
use notez_core::application::use_cases::SyncUseCase;

pub fn run_sync(json: bool, service: &mut Service, sub: SyncSubcommand) {
    match sub.command {
        SyncCommands::Push { actor, folder } => {
            match SyncUseCase::sync_push(service, &actor, &folder) {
                Ok(report) => {
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
            }
        }

        SyncCommands::Pull { actor, folder } => {
            match SyncUseCase::sync_pull(service, &actor, &folder) {
                Ok(report) => {
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
            }
        }

        SyncCommands::Conflicts => match SyncUseCase::list_conflicts(service) {
            Ok(conflicts) => {
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
        },
        SyncCommands::Relay { id } => match SyncUseCase::relay_sync(service) {
            Ok(report) => {
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
        },
    }
}
