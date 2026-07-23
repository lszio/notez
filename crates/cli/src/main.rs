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

            commands::SourceCommands::Writeback { id, r_ref, payload } => {
                match service.writeback_resource(&id, &r_ref, &payload) {
                    Ok(report) => {
                        if cli.json {
                            println!(
                                "{}",
                                json!({
                                    "target_ref": report.target_ref,
                                    "committed": report.committed
                                })
                            );
                        } else {
                            println!("Writeback committed: {}", report.committed);
                        }
                    }
                    Err(e) => {
                        eprintln!("Source writeback error: {e}");
                        exit(5);
                    }
                }
            }
        },
        Commands::Attachment(commands::AttachmentSubcommand { command }) => match command {
            commands::AttachmentCommands::Add { path, mime } => {
                match service.add_attachment(&cli.space, &path, &mime) {
                    Ok(att_ref) => {
                        if cli.json {
                            println!("{}", json!({ "ref": att_ref.to_string() }));
                        } else {
                            println!("{att_ref}");
                        }
                    }
                    Err(e) => {
                        eprintln!("Add attachment error: {e}");
                        exit(5);
                    }
                }
            }

            commands::AttachmentCommands::Extract { r_ref } => {
                let parsed_ref = match ResourceRef::parse(&r_ref) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Invalid resource ref format '{r_ref}': {e}");
                        exit(2);
                    }
                };

                match service.run_extraction(&cli.space, &parsed_ref) {
                    Ok(segments) => {
                        if cli.json {
                            println!(
                                "{}",
                                json!({
                                    "attachment_ref": r_ref,
                                    "segments_count": segments.len()
                                })
                            );
                        } else {
                            println!("Extracted {} segments for {r_ref}.", segments.len());
                        }
                    }
                    Err(e) => {
                        eprintln!("Extract attachment error: {e}");
                        exit(5);
                    }
                }
            }

            commands::AttachmentCommands::Segments { r_ref } => {
                let parsed_ref = match ResourceRef::parse(&r_ref) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Invalid resource ref format '{r_ref}': {e}");
                        exit(2);
                    }
                };

                match service.query_segments(&parsed_ref) {
                    Ok(segments) => {
                        if cli.json {
                            println!("{}", serde_json::to_string(&segments).unwrap());
                        } else {
                            for seg in segments {
                                println!("[{}-{}] {}", seg.offset_start, seg.offset_end, seg.text);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Query segments error: {e}");
                        exit(5);
                    }
                }
            }
        },
        Commands::Community(commands::CommunitySubcommand { command }) => match command {
            commands::CommunityCommands::Create {
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

                let comm = domain::community::Community {
                    id,
                    name,
                    selector,
                    pinned_members: vec![],
                    excluded_members: vec![],
                };

                match service.create_community(&cli.space, comm) {
                    Ok(_) => {
                        if cli.json {
                            println!("{}", json!({ "created": true }));
                        } else {
                            println!("Community created successfully.");
                        }
                    }
                    Err(e) => {
                        eprintln!("Create community error: {e}");
                        exit(5);
                    }
                }
            }

            commands::CommunityCommands::List => match service.list_communities(&cli.space) {
                Ok(communities) => {
                    if cli.json {
                        println!("{}", serde_json::to_string(&communities).unwrap());
                    } else {
                        for c in communities {
                            println!("{} ({})", c.id, c.name);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("List communities error: {e}");
                    exit(5);
                }
            },
        },

        Commands::Derive(commands::DeriveArgs { community, recipe }) => {
            match service.derive_artifact(&cli.space, &community, &recipe) {
                Ok(derived) => {
                    if cli.json {
                        println!("{}", serde_json::to_string(&derived).unwrap());
                    } else {
                        println!("{}", derived.content);
                    }
                }
                Err(e) => {
                    eprintln!("Derive artifact error: {e}");
                    exit(5);
                }
            }
        }

        Commands::Skill(commands::SkillSubcommand {
            command:
                commands::SkillCommands::Export {
                    community,
                    description,
                    out,
                },
        }) => match service.export_skill(&cli.space, &community, &description, &out) {
            Ok(pkg) => {
                if cli.json {
                    println!("{}", serde_json::to_string(&pkg).unwrap());
                } else {
                    println!("Exported skill package to {:?}", pkg.package_path);
                }
            }
            Err(e) => {
                eprintln!("Skill export error: {e}");
                exit(5);
            }
        },
        Commands::Sync(commands::SyncSubcommand { command }) => match command {
            commands::SyncCommands::Push { actor, folder } => {
                match service.sync_push(&actor, &cli.space, &folder) {
                    Ok(report) => {
                        if cli.json {
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
                        exit(5);
                    }
                }
            }

            commands::SyncCommands::Pull { actor, folder } => {
                match service.sync_pull(&actor, &cli.space, &folder) {
                    Ok(report) => {
                        if cli.json {
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
                        exit(5);
                    }
                }
            }

            commands::SyncCommands::Conflicts => match service.list_conflicts() {
                Ok(conflicts) => {
                    if cli.json {
                        println!("{}", serde_json::to_string(&conflicts).unwrap());
                    } else {
                        for c in conflicts {
                            println!("Conflict in {}: {}", c.logical_path, c.conflict_text);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Sync conflicts error: {e}");
                    exit(5);
                }
            },
            commands::SyncCommands::Relay { id } => match service.relay_sync(&id, &cli.space) {
                Ok(report) => {
                    if cli.json {
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
                    exit(5);
                }
            },
        },
        Commands::Job(commands::JobSubcommand {
            command: commands::JobCommands::List,
        }) => match service.list_jobs() {
            Ok(jobs) => {
                if cli.json {
                    println!("{}", serde_json::to_string(&jobs).unwrap());
                } else {
                    for j in jobs {
                        println!("Job {} ({}) -> {}", j.job_id, j.job_type, j.status);
                    }
                }
            }
            Err(e) => {
                eprintln!("List jobs error: {e}");
                exit(5);
            }
        },

        Commands::Artifact(commands::ArtifactSubcommand {
            command: commands::ArtifactCommands::Stale,
        }) => match service.check_artifact_freshness(&cli.space) {
            Ok(stale_report) => {
                if cli.json {
                    println!("{}", serde_json::to_string(&stale_report).unwrap());
                } else {
                    println!("Artifact Freshness: {}", stale_report.status);
                }
            }
            Err(e) => {
                eprintln!("Artifact freshness check error: {e}");
                exit(5);
            }
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

        Commands::Space(SpaceSubcommand { command }) => match command {
            SpaceCommands::Rebuild => match service.rebuild(&cli.space) {
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
                            report.scanned_files,
                            report.scanned_resources,
                            report.scanned_relations
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Rebuild error: {e}");
                    exit(5);
                }
            },

            SpaceCommands::Doctor => match service.space_doctor(&cli.space) {
                Ok(report) => {
                    if cli.json {
                        println!("{}", serde_json::to_string(&report).unwrap());
                    } else {
                        println!("Space Doctor Status: {}", report.status);
                        for issue in report.issues {
                            println!("[{}] {}: {}", issue.severity, issue.code, issue.message);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Space doctor error: {e}");
                    exit(5);
                }
            },
        },
    }
}
