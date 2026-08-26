//! Handlers for resource-facing commands: `resolve`, `query`, `read`,
//! `recent`, `resource …`, and `inspect …`.
//!
//! Every handler translates the parsed CLI syntax into a protocol
//! `Request`, dispatches it through the [`ApplicationDispatcher`], and
//! renders the unwrapped response payload. Ref/kind validation lives in
//! the dispatcher; malformed input surfaces as
//! `ApplicationError::InvalidRequest` (exit code 2).

use serde_json::json;
use std::process::exit;

use super::{Service, exit_code_for};
use crate::commands::{QueryArgs, ResourceCommands, ResourceSubcommand};
use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_core::application::ResolveResult;
use notez_core::domain::{Resource, ResourceKind, ResourceRef};
use notez_protocol::request::{
    DeleteResourceRequest, InspectRulesRequest, ListBySourceRequest, ListRecentRequest,
    ReadResourceRequest, Request, ResolveRequest, ResourcePayload, UpsertResourceRequest,
};

pub fn run_resolve(json: bool, service: &mut Service, query: &str) {
    let dispatched = ApplicationDispatcher::new(service).dispatch(Request::Resolve(ResolveRequest {
        query: query.to_string(),
    }));
    match dispatched {
        Ok(Response::Resolve(result)) => match result {
            ResolveResult::Found(r_ref) => {
                if json {
                    println!("{}", json!({ "ref": r_ref.to_string() }));
                } else {
                    println!("{r_ref}");
                }
            }
            ResolveResult::NotFound => {
                eprintln!("Resource not found: {query}");
                exit(3);
            }
            ResolveResult::Ambiguous(refs) => {
                eprintln!("Ambiguous resolve for '{query}'");
                if json {
                    let str_refs: Vec<String> = refs.iter().map(|r| r.to_string()).collect();
                    println!("{}", json!({ "ambiguous": str_refs }));
                }
                exit(4);
            }
        },
        Err(e) => {
            eprintln!("Resolve error: {e}");
            exit(exit_code_for(&e));
        }
        other => unreachable!("unexpected dispatcher response: {other:?}"),
    }
}

pub fn run_query(json: bool, service: &mut Service, args: QueryArgs) {
    let request = Request::QueryResources(notez_protocol::request::QueryResourcesRequest {
        kind: args.kind.map(|k| ResourceKind::from(k).to_string()),
        title_contains: args.title_contains,
        exact_ref: args.exact_ref,
        source_id: args.source,
        limit: Some(args.limit as u32),
    });
    match ApplicationDispatcher::new(service).dispatch(request) {
        Ok(Response::ResourcePage(page)) => {
            // The engine returns the full page; the CLI applies its own
            // `--limit` window at the render layer.
            let mut items = page.items;
            if items.len() > args.limit {
                items.truncate(args.limit);
            }
            if json {
                println!("{}", serde_json::to_string(&items).unwrap());
            } else {
                for res in items {
                    println!("{} {}", res.r#ref, res.title);
                }
            }
        }
        Err(e) => {
            eprintln!("Query error: {e}");
            exit(exit_code_for(&e));
        }
        other => unreachable!("unexpected dispatcher response: {other:?}"),
    }
}

pub fn run_read(json: bool, service: &mut Service, r_ref: &str) {
    let dispatched = ApplicationDispatcher::new(service)
        .dispatch(Request::ReadResource(ReadResourceRequest {
            r_ref: r_ref.to_string(),
        }));
    match dispatched {
        Ok(Response::Resource(Some(res))) => {
            if json {
                println!("{}", serde_json::to_string(&res).unwrap());
            } else {
                println!("Ref:      {}", res.r#ref);
                println!("Title:    {}", res.title);
                println!("Kind:     {}", res.kind);
                println!("Locator:  {}", res.locator);
                println!("Revision: {}", res.revision);
            }
        }
        Ok(Response::Resource(None)) => {
            eprintln!("Resource not found: {r_ref}");
            exit(3);
        }
        Err(e) => {
            eprintln!("Read error: {e}");
            exit(exit_code_for(&e));
        }
        other => unreachable!("unexpected dispatcher response: {other:?}"),
    }
}

pub fn run_recent(json: bool, service: &mut Service, limit: usize) {
    let dispatched = ApplicationDispatcher::new(service)
        .dispatch(Request::ListRecent(ListRecentRequest {
            limit: Some(limit as u32),
        }));
    match dispatched {
        Ok(Response::Resources(items)) => {
            if json {
                println!("{}", serde_json::to_string(&items).unwrap());
            } else {
                for res in items {
                    println!("{} {} ({})", res.r#ref, res.title, res.source_id);
                }
            }
        }
        Err(e) => {
            eprintln!("Recent error: {e}");
            exit(exit_code_for(&e));
        }
        other => unreachable!("unexpected dispatcher response: {other:?}"),
    }
}

pub fn run_resource(json: bool, service: &mut Service, sub: ResourceSubcommand) {
    let mut dispatcher = ApplicationDispatcher::new(service);
    match sub.command {
        ResourceCommands::Upsert { from } => {
            let bytes = match std::fs::read(&from) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("Failed to read {}: {e}", from.display());
                    exit(2);
                }
            };
            let res: Resource = match serde_json::from_slice(&bytes) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Invalid resource JSON: {e}");
                    exit(2);
                }
            };
            let payload = ResourcePayload {
                ref_: res.r#ref.to_string(),
                kind: res.kind.to_string(),
                title: res.title.clone(),
                revision: res.revision.clone(),
                source_id: res.source_id.clone(),
                locator: res.locator.clone(),
                properties: res.properties.clone(),
                object_id: Some(res.object_id.to_string()),
                primary_source_id: res.primary_source_id.clone(),
            };
            match dispatcher.dispatch(Request::UpsertResource(UpsertResourceRequest {
                resource: payload,
            })) {
                Ok(Response::Done) => {
                    if json {
                        println!("{}", serde_json::to_string(&res).unwrap());
                    } else {
                        println!("Upserted {}", res.r#ref);
                    }
                }
                Err(e) => {
                    eprintln!("Upsert error: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
        ResourceCommands::Delete { r_ref } => {
            match dispatcher.dispatch(Request::DeleteResource(DeleteResourceRequest {
                r_ref: r_ref.clone(),
            })) {
                Ok(Response::Done) => {
                    // The engine validated and deleted the ref; recover
                    // its normalized form purely for rendering.
                    let parsed =
                        ResourceRef::parse(&r_ref).expect("engine deleted an unparseable ref");
                    if json {
                        println!("{}", serde_json::to_string(&parsed).unwrap());
                    } else {
                        println!("Deleted {}", parsed);
                    }
                }
                Err(e) => {
                    eprintln!("Delete error: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
        ResourceCommands::Ls { source, limit } => {
            match dispatcher.dispatch(Request::ListBySource(ListBySourceRequest {
                source_id: source,
                limit: Some(limit as u32),
            })) {
                Ok(Response::Resources(items)) => {
                    if json {
                        println!("{}", serde_json::to_string(&items).unwrap());
                    } else {
                        for res in items {
                            println!("{} {}", res.r#ref, res.title);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("List error: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
    }
}

pub fn run_inspect(json: bool, service: &mut Service, r_ref: &str, rules: bool) {
    if rules {
        let dispatched = ApplicationDispatcher::new(service)
            .dispatch(Request::InspectRules(InspectRulesRequest {
                source_ref: r_ref.to_string(),
            }));
        match dispatched {
            Ok(Response::InspectRules(Some(inspect_res))) => {
                if json {
                    println!("{}", serde_json::to_string(&inspect_res).unwrap());
                } else {
                    println!("Ref: {}", inspect_res.r_ref);
                    println!("Type: {:?}", inspect_res.classified_type);
                    println!("Derived: {:?}", inspect_res.derived_properties);
                }
            }
            Ok(Response::InspectRules(None)) => {
                eprintln!("Resource not found: {r_ref}");
                exit(3);
            }
            Err(e) => {
                eprintln!("Inspect error: {e}");
                exit(exit_code_for(&e));
            }
            other => unreachable!("unexpected dispatcher response: {other:?}"),
        }
    } else {
        // `inspect` without --rules keeps its legacy Debug rendering,
        // distinct from `read`'s field table.
        let dispatched = ApplicationDispatcher::new(service)
            .dispatch(Request::ReadResource(ReadResourceRequest {
                r_ref: r_ref.to_string(),
            }));
        match dispatched {
            Ok(Response::Resource(Some(res))) => {
                if json {
                    println!("{}", serde_json::to_string(&res).unwrap());
                } else {
                    println!("{res:?}");
                }
            }
            Ok(Response::Resource(None)) => {
                eprintln!("Resource not found: {r_ref}");
                exit(3);
            }
            Err(e) => {
                eprintln!("Read error: {e}");
                exit(exit_code_for(&e));
            }
            other => unreachable!("unexpected dispatcher response: {other:?}"),
        }
    }
}
