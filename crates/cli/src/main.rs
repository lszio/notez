pub mod commands;

use application::{ApplicationService, ResolveResult};
use clap::Parser;
use commands::{Cli, Commands, McpCommands, McpSubcommand, SpaceCommands, SpaceSubcommand};
use domain::{ResourceKind, ResourceRef, Selector};
use serde_json::json;
use std::fs;
use std::process::exit;
use storage::SqliteProjection;

fn main() {
    let cli = Cli::parse();

    let db_path = match cli.db {
        Some(path) => path,
        None => cli.space.join(".notez/index.sqlite"),
    };

    if let Some(parent) = db_path.parent()
        && !parent.exists()
    {
        let _ = fs::create_dir_all(parent);
    }

    let store = match SqliteProjection::open(&db_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error opening database: {e}");
            exit(5);
        }
    };

    let mut service = ApplicationService::new(store);

    match cli.command {
        Commands::Scan => match service.scan_native(&cli.space) {
            Ok(report) => {
                if cli.json {
                    println!(
                        "{}",
                        json!({
                            "scanned_files": report.scanned_files,
                            "scanned_resources": report.scanned_resources,
                            "scanned_relations": report.scanned_relations,
                        })
                    );
                } else {
                    println!(
                        "Scanned {} files, {} resources, {} relations.",
                        report.scanned_files, report.scanned_resources, report.scanned_relations
                    );
                }
            }
            Err(e) => {
                eprintln!("Scan error: {e}");
                exit(5);
            }
        },

        Commands::Resolve { query } => match service.resolve(&query) {
            Ok(ResolveResult::Found(r_ref)) => {
                if cli.json {
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
                if cli.json {
                    let str_refs: Vec<String> = refs.iter().map(|r| r.to_string()).collect();
                    println!("{}", json!({ "ambiguous": str_refs }));
                }
                exit(4);
            }
            Err(e) => {
                eprintln!("Resolve error: {e}");
                exit(5);
            }
        },

        Commands::Query(args) => {
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

            match service.query(&selector) {
                Ok(page) => {
                    if cli.json {
                        println!("{}", serde_json::to_string(&page).unwrap());
                    } else {
                        for res in page.items {
                            println!("{} {}", res.r#ref, res.title);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Query error: {e}");
                    exit(5);
                }
            }
        }

        Commands::Read { r_ref } => {
            let parsed_ref = match ResourceRef::parse(&r_ref) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Invalid resource ref format '{r_ref}': {e}");
                    exit(2);
                }
            };

            match service.read(&parsed_ref) {
                Ok(Some(res)) => {
                    if cli.json {
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
                    exit(5);
                }
            }
        }

        Commands::Mcp(McpSubcommand {
            command: McpCommands::Serve,
        }) => {
            let stdin = std::io::stdin().lock();
            let stdout = std::io::stdout().lock();
            if let Err(e) = mcp::McpServer::serve(stdin, stdout, &mut service) {
                eprintln!("MCP server error: {e}");
                exit(5);
            }
        }

        Commands::Space(SpaceSubcommand {
            command: SpaceCommands::Rebuild,
        }) => match service.rebuild(&cli.space) {
            Ok(report) => {
                if cli.json {
                    println!(
                        "{}",
                        json!({
                            "rebuilt": true,
                            "scanned_files": report.scanned_files,
                            "scanned_resources": report.scanned_resources,
                            "scanned_relations": report.scanned_relations,
                        })
                    );
                } else {
                    println!(
                        "Rebuilt index: {} files, {} resources, {} relations.",
                        report.scanned_files, report.scanned_resources, report.scanned_relations
                    );
                }
            }
            Err(e) => {
                eprintln!("Rebuild error: {e}");
                exit(5);
            }
        },
    }
}
