pub mod commands;

use application::{ApplicationService, ResolveResult};
use clap::Parser;
use commands::LinkCommands;
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

        Commands::Link(sub) => match sub.command {
            LinkCommands::List { r_ref } => match ResourceRef::parse(&r_ref) {
                Ok(parsed_ref) => match service.query_link_occurrences(&parsed_ref) {
                    Ok(occs) => {
                        if cli.json {
                            println!("{}", json!(occs));
                        } else {
                            for occ in occs {
                                println!(
                                    "{}:{}-{} -> {}",
                                    occ.span.line,
                                    occ.span.col_start,
                                    occ.span.col_end,
                                    occ.target
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error querying occurrences: {e}");
                        exit(5);
                    }
                },
                Err(e) => {
                    eprintln!("Invalid ref parameter: {e}");
                    exit(2);
                }
            },
            LinkCommands::Resolved { r_ref } => match ResourceRef::parse(&r_ref) {
                Ok(parsed_ref) => match service.query_resolved_relations(&parsed_ref) {
                    Ok(rels) => {
                        if cli.json {
                            println!("{}", json!(rels));
                        } else {
                            for rel in rels {
                                println!(
                                    "{} -> {} ({:?})",
                                    rel.target, rel.target_ref, rel.status
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error querying resolved relations: {e}");
                        exit(5);
                    }
                },
                Err(e) => {
                    eprintln!("Invalid ref parameter: {e}");
                    exit(2);
                }
            },
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

        Commands::Task(commands::TaskSubcommand { command }) => {
            let cmd = command.unwrap_or(commands::TaskCommands::Agenda);
            match cmd {
                commands::TaskCommands::Transition { r_ref, to, timestamp } => {
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
                            eprintln!("Transition error: {e}");
                            exit(5);
                        }
                    }
                }
                commands::TaskCommands::List => match service.agenda() {
                    Ok(agenda) => {
                        if cli.json {
                            println!("{}", serde_json::to_string(&agenda).unwrap());
                        } else {
                            // Group by TODO state
                            let mut by_state: std::collections::BTreeMap<String, Vec<&application::task_para::AgendaItem>> = std::collections::BTreeMap::new();
                            for item in &agenda.items {
                                let state = item.todo.clone().unwrap_or_else(|| "NONE".to_string());
                                by_state.entry(state).or_default().push(item);
                            }
                            for (state, items) in by_state {
                                println!("=== {} ===", state);
                                for item in items {
                                    let date_info = if let Some(ref d) = item.deadline {
                                        format!(" (DEADLINE: {d})")
                                    } else if let Some(ref s) = item.scheduled {
                                        format!(" (SCHEDULED: {s})")
                                    } else {
                                        String::new()
                                    };
                                    println!("- {}{} [{}]", item.title, date_info, item.r_ref);
                                }
                                println!();
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("List error: {e}");
                        exit(5);
                    }
                },
                commands::TaskCommands::Agenda => match service.agenda() {
                    Ok(agenda) => {
                        if cli.json {
                            println!("{}", serde_json::to_string(&agenda).unwrap());
                        } else {
                            // Extract YYYY-MM-DD and group
                            let mut by_date: std::collections::BTreeMap<String, Vec<(&application::task_para::AgendaItem, String)>> = std::collections::BTreeMap::new();
                            let mut unscheduled = Vec::new();
                            let mut completed = Vec::new();

                            let extract_date = |s: &str| -> Option<String> {
                                // Look for YYYY-MM-DD inside < > or [ ]
                                let s = s.trim_matches(|c| c == '<' || c == '>' || c == '[' || c == ']');
                                if s.len() >= 10 {
                                    Some(s[0..10].to_string())
                                } else {
                                    None
                                }
                            };

                            for item in &agenda.items {
                                let is_done = item.todo.as_deref() == Some("DONE") || item.todo.as_deref() == Some("QUIT");
                                
                                if is_done {
                                    let date_str = item.closed.as_deref()
                                        .or(item.deadline.as_deref())
                                        .or(item.scheduled.as_deref())
                                        .unwrap_or("");
                                    completed.push((item, date_str.to_string()));
                                    continue;
                                }

                                let mut has_date = false;
                                if let Some(ref d) = item.deadline {
                                    if let Some(date) = extract_date(d) {
                                        by_date.entry(date).or_default().push((item, format!("Deadline: {}", d)));
                                        has_date = true;
                                    }
                                }
                                if let Some(ref s) = item.scheduled {
                                    if let Some(date) = extract_date(s) {
                                        by_date.entry(date).or_default().push((item, format!("Scheduled: {}", s)));
                                        has_date = true;
                                    }
                                }
                                if !has_date {
                                    unscheduled.push(item);
                                }
                            }

                            println!("=== Agenda View ===");
                            for (date, items) in by_date {
                                println!("\n{}", date);
                                println!("{:-<30}", "");
                                for (item, time_info) in items {
                                    let todo = item.todo.as_deref().unwrap_or("");
                                    let state_str = if todo.is_empty() { String::new() } else { format!("[{}] ", todo) };
                                    println!("  {time_info:<25} | {state_str}{}", item.title);
                                }
                            }
                            if !unscheduled.is_empty() {
                                println!("\n=== Unscheduled Active Tasks ===");
                                for item in unscheduled {
                                    let todo = item.todo.as_deref().unwrap_or("");
                                    let state_str = if todo.is_empty() { String::new() } else { format!("[{}] ", todo) };
                                    println!("  {state_str}{}", item.title);
                                }
                            }

                            if !completed.is_empty() {
                                println!("\n=== Completed ===");
                                // Sort completed roughly by closed date
                                completed.sort_by(|a, b| b.1.cmp(&a.1));
                                for (item, date_str) in completed {
                                    let state_str = format!("[{}] ", item.todo.as_deref().unwrap_or("DONE"));
                                    let time_info = if date_str.is_empty() { String::new() } else { format!(" Closed: {:<20} |", date_str) };
                                    println!(" {time_info} {state_str}{}", item.title);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Agenda error: {e}");
                        exit(5);
                    }
                },
                commands::TaskCommands::Para => match service.para_overview() {
                    Ok(para) => {
                        if cli.json {
                            println!("{}", serde_json::to_string(&para).unwrap());
                        } else {
                            let print_nodes = |nodes: Vec<application::task_para::ParaNode>| {
                                for node in nodes {
                                    let todo_str = node.resource.properties.get("TODO").map(|s| format!("[{}] ", s)).unwrap_or_default();
                                    println!("- {}{} ({})", todo_str, node.resource.title, node.resource.r#ref);
                                    for task in node.tasks {
                                        let t_todo = task.todo.map(|s| format!("[{}] ", s)).unwrap_or_default();
                                        println!("    * {}{} ({})", t_todo, task.title, task.r_ref);
                                    }
                                }
                            };
                            println!("=== Projects ===");
                            print_nodes(para.projects);
                            println!("\n=== Areas ===");
                            print_nodes(para.areas);
                            println!("\n=== Resources ===");
                            print_nodes(para.resources);
                            println!("\n=== Archives ===");
                            print_nodes(para.archives);
                        }
                    }
                    Err(e) => {
                        eprintln!("Para overview error: {e}");
                        exit(5);
                    }
                },
                commands::TaskCommands::Jobs => match service.list_jobs() {
                    Ok(jobs) => {
                        if cli.json {
                            println!("{}", json!(jobs));
                        } else {
                            println!("=== Background System Jobs ===");
                            for j in jobs {
                                println!("Job {} ({}) -> {}", j.job_id, j.job_type, j.status);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Job list error: {e}");
                        exit(5);
                    }
                },
            }
        },
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
