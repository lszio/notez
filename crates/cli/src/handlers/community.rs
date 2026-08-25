//! Handlers for the derived-artifact command family:
//! `Commands::Community`, `Commands::Derive`, `Commands::Skill`, and
//! `Commands::Artifact`.

use serde_json::json;
use std::path::Path;
use std::process::exit;

use super::{Service, exit_code_for};
use crate::commands::{CommunityCommands, CommunitySubcommand};
use notez_core::application::use_cases::{ArtifactUseCase, CommunityUseCase, InspectUseCase};
use notez_core::domain::{ResourceKind, Selector};

pub fn run_community(json: bool, service: &Service, sub: CommunitySubcommand) {
    match sub.command {
        CommunityCommands::Create {
            id,
            name,
            kind,
            title_contains,
        } => {
            let mut selector = Selector::new();
            if let Some(k) = kind {
                let r_kind: ResourceKind = k.into();
                selector.kind = Some(r_kind);
            }
            if let Some(t) = title_contains {
                selector.title_contains = Some(t);
            }

            let comm = notez_core::domain::community::Community {
                id,
                name,
                selector,
                pinned_members: vec![],
                excluded_members: vec![],
            };

            match CommunityUseCase::create_community(service, comm) {
                Ok(_) => {
                    if json {
                        println!("{}", json!({ "created": true }));
                    } else {
                        println!("Community created successfully.");
                    }
                }
                Err(e) => {
                    eprintln!("Create community error: {e}");
                    exit(exit_code_for(&e));
                }
            }
        }

        CommunityCommands::List => match CommunityUseCase::list_communities(service) {
            Ok(communities) => {
                if json {
                    println!("{}", serde_json::to_string(&communities).unwrap());
                } else {
                    for c in communities {
                        println!("{} ({})", c.id, c.name);
                    }
                }
            }
            Err(e) => {
                eprintln!("List communities error: {e}");
                exit(exit_code_for(&e));
            }
        },
    }
}

pub fn run_derive(json: bool, service: &mut Service, community: &str, recipe: &str) {
    match ArtifactUseCase::derive_artifact(service, community, recipe) {
        Ok(derived) => {
            if json {
                println!("{}", serde_json::to_string(&derived).unwrap());
            } else {
                println!("{}", derived.content);
            }
        }
        Err(e) => {
            eprintln!("Derive artifact error: {e}");
            exit(exit_code_for(&e));
        }
    }
}

pub fn run_skill_export(
    json: bool,
    service: &mut Service,
    community: &str,
    description: &str,
    out: &Path,
) {
    match ArtifactUseCase::export_skill(service, community, description, out) {
        Ok(pkg) => {
            if json {
                println!("{}", serde_json::to_string(&pkg).unwrap());
            } else {
                println!("Exported skill package to {:?}", pkg.package_path);
            }
        }
        Err(e) => {
            eprintln!("Skill export error: {e}");
            exit(exit_code_for(&e));
        }
    }
}

pub fn run_artifact_stale(json: bool, service: &Service) {
    match InspectUseCase::check_artifact_freshness(service) {
        Ok(stale_report) => {
            if json {
                println!("{}", serde_json::to_string(&stale_report).unwrap());
            } else {
                println!("Artifact Freshness: {}", stale_report.status);
            }
        }
        Err(e) => {
            eprintln!("Artifact freshness check error: {e}");
            exit(exit_code_for(&e));
        }
    }
}
