//! Handlers for the derived-artifact command family:
//! `Commands::Community`, `Commands::Derive`, `Commands::Skill`, and
//! `Commands::Artifact`.
//!
//! Every arm translates the parsed CLI syntax into a protocol
//! `Request`, dispatches it through the [`ApplicationDispatcher`], and
//! renders the unwrapped response payload.

use serde_json::json;
use std::path::Path;
use std::process::exit;

use super::{Service, exit_code_for};
use crate::commands::{CommunityCommands, CommunitySubcommand};
use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_core::domain::ResourceKind;
use notez_protocol::request::{
    ArtifactFreshnessRequest, CreateCommunityRequest, DeriveArtifactRequest,
    ExportSkillRequest, ListCommunitiesRequest, Request,
};

pub fn run_community(json: bool, service: &mut Service, sub: CommunitySubcommand) {
    let mut dispatcher = ApplicationDispatcher::new(service);
    match sub.command {
        CommunityCommands::Create {
            id,
            name,
            kind,
            title_contains,
        } => {
            match dispatcher.dispatch(Request::CreateCommunity(CreateCommunityRequest {
                id,
                name,
                kind: kind.map(|k| ResourceKind::from(k).to_string()),
                title_contains,
            })) {
                Ok(Response::Done) => {
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
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }

        CommunityCommands::List => {
            match dispatcher.dispatch(Request::ListCommunities(ListCommunitiesRequest {})) {
                Ok(Response::Communities(communities)) => {
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
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
    }
}

pub fn run_derive(json: bool, service: &mut Service, community: &str, recipe: &str) {
    match ApplicationDispatcher::new(service).dispatch(Request::DeriveArtifact(
        DeriveArtifactRequest {
            community_id: community.to_string(),
            recipe_name: recipe.to_string(),
        },
    )) {
        Ok(Response::Derived(derived)) => {
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
        other => unreachable!("unexpected dispatcher response: {other:?}"),
    }
}

pub fn run_skill_export(
    json: bool,
    service: &mut Service,
    community: &str,
    description: &str,
    out: &Path,
) {
    match ApplicationDispatcher::new(service).dispatch(Request::ExportSkill(ExportSkillRequest {
        community_id: community.to_string(),
        description: Some(description.to_string()),
        out_path: out.display().to_string(),
    })) {
        Ok(Response::Skill(pkg)) => {
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
        other => unreachable!("unexpected dispatcher response: {other:?}"),
    }
}

pub fn run_artifact_stale(json: bool, service: &mut Service) {
    match ApplicationDispatcher::new(service)
        .dispatch(Request::ArtifactFreshness(ArtifactFreshnessRequest {}))
    {
        Ok(Response::Freshness(stale_report)) => {
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
        other => unreachable!("unexpected dispatcher response: {other:?}"),
    }
}
