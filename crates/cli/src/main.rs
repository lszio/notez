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
        Commands::Inspect { r_ref, rules } => {
            let parsed_ref = match ResourceRef::parse(&r_ref) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Invalid resource ref format '{r_ref}': {e}");
                    exit(2);
                }
            };

            if rules {
                match service.inspect_rules(&parsed_ref) {
                    Ok(Some(inspect_res)) => {
                        if cli.json {
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
                        exit(5);
                    }
                }
            } else {
                match service.read(&parsed_ref) {
                    Ok(Some(res)) => {
                        if cli.json {
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
                        exit(5);
                    }
                }
            }
        }

        Commands::Agenda => match service.agenda() {
            Ok(agenda) => {
                if cli.json {
                    println!("{}", serde_json::to_string(&agenda).unwrap());
                } else {
                    for item in agenda.items {
                        println!("{} {}", item.r_ref, item.title);
                    }
                }
            }
            Err(e) => {
                eprintln!("Agenda error: {e}");
                exit(5);
            }
        },

        Commands::Task(commands::TaskSubcommand {
            command:
                commands::TaskCommands::Transition {
                    r_ref,
                    to,
                    timestamp,
                },
        }) => {
            let parsed_ref = match ResourceRef::parse(&r_ref) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Invalid resource ref format '{r_ref}': {e}");
                    exit(2);
                }
            };

            match service.transition_task(&parsed_ref, &to, &timestamp) {
                Ok(transition) => {
                    if cli.json {
                        println!("{}", serde_json::to_string(&transition).unwrap());
                    } else {
                        println!(
                            "Transitioned {} from {} to {}",
                            r_ref, transition.from_state, transition.to_state
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Task transition error: {e}");
                    exit(5);
                }
            }
        }
        Commands::Source(commands::SourceSubcommand { command }) => match command {
            commands::SourceCommands::Add {
                id,
                kind,
                path,
                read_only,
            } => {
                let config = source::SourceConfig {
                    id,
                    kind: kind.into(),
                    path,
                    read_only,
                    exclude_paths: vec![],
                };
                match service.add_source(&cli.space, config) {
                    Ok(_) => {
                        if cli.json {
                            println!("{}", json!({ "added": true }));
                        } else {
                            println!("Source added successfully.");
                        }
                    }
                    Err(e) => {
                        eprintln!("Add source error: {e}");
                        exit(5);
                    }
                }
            }

            commands::SourceCommands::List => match service.list_sources(&cli.space) {
                Ok(sources) => {
                    if cli.json {
                        println!("{}", serde_json::to_string(&sources).unwrap());
                    } else {
                        for src in sources {
                            println!("{} ({:?}): {:?}", src.id, src.kind, src.path);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("List sources error: {e}");
                    exit(5);
                }
            },

            commands::SourceCommands::Sync => match service.scan_federation(&cli.space) {
                Ok(report) => {
                    if cli.json {
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
                    exit(5);
                }
            },
        },

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
