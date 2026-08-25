//! Handlers for resource-facing commands: `resolve`, `query`, `read`,
//! `recent`, `resource …`, and `inspect …`.

use serde_json::json;
use std::process::exit;

use super::{Service, exit_code_for};
use crate::commands::{QueryArgs, ResourceCommands, ResourceSubcommand};
use notez_core::application::{
    ResolveResult,
    use_cases::{InspectUseCase, ResourceUseCase},
};
use notez_core::domain::{Resource, ResourceKind, ResourceRef, Selector};

pub fn run_resolve(json: bool, service: &Service, query: &str) {
    match ResourceUseCase::resolve(service, query) {
        Ok(ResolveResult::Found(r_ref)) => {
            if json {
                println!("{}", json!({ "ref": r_ref.to_string() }));
            } else {
                println!("{r_ref}");
            }
        }
        Ok(ResolveResult::NotFound) => {
            eprintln!("Resource not found: {query}");
            exit(3);
        }
        Ok(ResolveResult::Ambiguous(refs)) => {
            eprintln!("Ambiguous resolve for '{query}'");
            if json {
                let str_refs: Vec<String> = refs.iter().map(|r| r.to_string()).collect();
                println!("{}", json!({ "ambiguous": str_refs }));
            }
            exit(4);
        }
        Err(e) => {
            eprintln!("Resolve error: {e}");
            exit(exit_code_for(&e));
        }
    }
}

pub fn run_query(json: bool, service: &Service, args: QueryArgs) {
    let mut selector = Selector::new();
    if let Some(kind) = args.kind {
        let r_kind: ResourceKind = kind.into();
        selector.kind = Some(r_kind);
    }
    if let Some(title_sub) = args.title_contains {
        selector.title_contains = Some(title_sub);
    }
    if let Some(exact_ref_str) = args.exact_ref {
        match ResourceRef::parse(&exact_ref_str) {
            Ok(r_ref) => {
                selector.exact_refs.push(r_ref);
            }
            Err(e) => {
                eprintln!("Invalid exact-ref parameter: {e}");
                exit(2);
            }
        }
    }
    if let Some(src) = args.source {
        selector.source_id = Some(src);
    }

    match ResourceUseCase::query(service, &selector) {
        Ok(page) => {
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
    }
}

pub fn run_read(json: bool, service: &Service, r_ref: &str) {
    let parsed_ref = match ResourceRef::parse(r_ref) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Invalid resource ref format '{r_ref}': {e}");
            exit(2);
        }
    };

    match ResourceUseCase::read(service, &parsed_ref) {
        Ok(Some(res)) => {
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
        Ok(None) => {
            eprintln!("Resource not found: {r_ref}");
            exit(3);
        }
        Err(e) => {
            eprintln!("Read error: {e}");
            exit(exit_code_for(&e));
        }
    }
}

pub fn run_recent(json: bool, service: &Service, limit: usize) {
    match ResourceUseCase::list_recent(service, limit) {
        Ok(items) => {
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
    }
}

pub fn run_resource(json: bool, service: &mut Service, sub: ResourceSubcommand) {
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
            match ResourceUseCase::upsert_resource(service, res.clone()) {
                Ok(()) => {
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
            }
        }
        ResourceCommands::Delete { r_ref } => {
            let parsed = match ResourceRef::parse(&r_ref) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Invalid resource ref '{r_ref}': {e}");
                    exit(2);
                }
            };
            match ResourceUseCase::delete_resource(service, &parsed) {
                Ok(()) => {
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
            }
        }
        ResourceCommands::Ls { source, limit } => {
            match ResourceUseCase::list_by_source(service, &source, limit) {
                Ok(items) => {
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
            }
        }
    }
}

pub fn run_inspect(json: bool, service: &Service, r_ref: &str, rules: bool) {
    let parsed_ref = match ResourceRef::parse(r_ref) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Invalid resource ref format '{r_ref}': {e}");
            exit(2);
        }
    };

    if rules {
        match InspectUseCase::inspect_rules(service, &parsed_ref) {
            Ok(Some(inspect_res)) => {
                if json {
                    println!("{}", serde_json::to_string(&inspect_res).unwrap());
                } else {
                    println!("Ref: {}", inspect_res.r_ref);
                    println!("Type: {:?}", inspect_res.classified_type);
                    println!("Derived: {:?}", inspect_res.derived_properties);
                }
            }
            Ok(None) => {
                eprintln!("Resource not found: {r_ref}");
                exit(3);
            }
            Err(e) => {
                eprintln!("Inspect error: {e}");
                exit(exit_code_for(&e));
            }
        }
    } else {
        match ResourceUseCase::read(service, &parsed_ref) {
            Ok(Some(res)) => {
                if json {
                    println!("{}", serde_json::to_string(&res).unwrap());
                } else {
                    println!("{res:?}");
                }
            }
            Ok(None) => {
                eprintln!("Resource not found: {r_ref}");
                exit(3);
            }
            Err(e) => {
                eprintln!("Read error: {e}");
                exit(exit_code_for(&e));
            }
        }
    }
}
