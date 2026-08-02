use notez_cli::commands;
use notez_core::application::{
    use_cases::{
        ArtifactUseCase, AttachmentUseCase, CommunityUseCase, InspectUseCase, LinkUseCase,
        ResourceUseCase, ScanUseCase, SyncUseCase, TaskUseCase,
    },
    ApplicationService, ResolveResult, SpaceContext,
};
use notez_core::domain::{Resource, ResourceKind, ResourceRef, Selector};
use notez_core::storage::SqliteProjection;
use clap::Parser;
use commands::LinkCommands;
use commands::{Cli, Commands, McpCommands, McpSubcommand, SpaceCommands, SpaceSubcommand};
use serde_json::json;
use std::fs;
use std::process::exit;

/// Map an `ApplicationError` to a stable process exit code.
///
/// The mapping is part of the CLI contract and must not change without
/// a coordinated release (see the error design spec).
fn exit_code_for(err: &notez_core::application::ApplicationError) -> i32 {
    use notez_core::application::ApplicationError;
    match err {
        ApplicationError::NotFound { .. } => 3,
        ApplicationError::Storage { .. } => 5,
        ApplicationError::Document { .. } => 5,
        ApplicationError::Io { .. } => 5,
        ApplicationError::UnsupportedCapability { .. } => 6,
        ApplicationError::ReadOnlySource { .. } => 7,
        ApplicationError::SourceNotFound { .. } => 8,
        ApplicationError::RevisionConflict { .. } => 9,
    }
}

fn main() {
    let cli = Cli::parse();

    // Match on the subcommand first. The `ListCapabilities` variant
    // short-circuits before any space or store wiring; it just prints
    // the catalog as JSON and exits 0. All other variants fall through
    // to the existing space-selection flow.
    if let Commands::ListCapabilities = cli.command {
        let stub_db = std::path::Path::new("/tmp/notez-capabilities-stub.sqlite");
        let store = match SqliteProjection::open(stub_db) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to open stub database: {e}");
                std::process::exit(5);
            }
        };
        let facade = notez_core::application::ApplicationFacade::new(store);
        let entries: Vec<serde_json::Value> = facade
            .capability_catalog()
            .list()
            .iter()
            .map(|d| {
                serde_json::json!({
                    "id": d.id,
                    "description": d.description,
                    "mutability": match d.mutability {
                        notez_core::capability::Mutability::Read => "read",
                        notez_core::capability::Mutability::Write => "write",
                    },
                })
            })
            .collect();
        match serde_json::to_string_pretty(&entries) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("Failed to serialise capabilities: {e}");
                std::process::exit(5);
            }
        }
        std::process::exit(0);
    }

    let env_vars: std::collections::BTreeMap<String, std::ffi::OsString> =
        std::env::vars_os().map(|(k, v)| (k.to_string_lossy().to_string(), v)).collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let paths = match notez_core::config::ConfigPaths::discover(&env_vars, &cwd) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Configuration discovery error: {e}");
            exit(2);
        }
    };

    // 2. Select space
    let resolved_path: Option<std::path::PathBuf>;
    let selector = match &cli.space {
        Some(s) => {
            if std::path::Path::new(s).is_absolute() || s.starts_with('.') || s.starts_with("~/") || s.contains('/') {
                resolved_path = Some(std::path::PathBuf::from(s));
                notez_core::config::SpaceSelector::Path(resolved_path.as_ref().unwrap())
            } else {
                notez_core::config::SpaceSelector::Name(s)
            }
        }
        None => {
            if paths.space_config.is_some() {
                notez_core::config::SpaceSelector::Upward
            } else {
                notez_core::config::SpaceSelector::Default
            }
        }
    };

    let selected = match notez_core::config::select_space(&paths, selector) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Space selection error: {e}");
            exit(2);
        }
    };

    // 3. Load runtime config
    let r_config = match notez_core::config::load_runtime_config(&paths, &selected, &env_vars, None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Configuration load error: {e}");
            exit(2);
        }
    };

    let db_path = match cli.db {
        Some(path) => path,
        None => r_config.database.clone(),
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
    let mut runtime = r_config.clone();
    // Merge legacy `.notez/sources.json` (used by `notez source add`) into
    // the runtime so that mutations see sources registered in prior
    // invocations. Sources already declared in `notez.toml` are kept.
    let json_sources = notez_core::application::federation::SpaceSourcesConfig::load(&r_config.space_root)
        .ok()
        .map(|c| c.sources)
        .unwrap_or_default();
    for src in json_sources {
        if !runtime.sources.iter().any(|s| s.id == src.id) {
            runtime.sources.push(src);
        }
    }
    let space = SpaceContext::new(runtime.space_name.clone(), runtime.space_root.clone(), runtime);
    let mut service = ApplicationService::with_space(store, space);
    service.register_format_parser(Box::new(orgmode::OrgParser::new()));
    service.register_format_parser(Box::new(markdown::MarkdownParser::new()));
    match cli.command {
        Commands::Scan => match ScanUseCase::scan_native(&mut service, &r_config.space_root) {
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
                exit(exit_code_for(&e));
            }
        },

        Commands::Resolve { query } => match ResourceUseCase::resolve(&service, &query) {
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
                exit(exit_code_for(&e));
            }
        },

        Commands::Link(sub) => match sub.command {
            LinkCommands::List { r_ref } => match ResourceRef::parse(&r_ref) {
                Ok(parsed_ref) => match LinkUseCase::query_link_occurrences(&service, &parsed_ref) {
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
                        exit(exit_code_for(&e));
                    }
                },
                Err(e) => {
                    eprintln!("Invalid ref parameter: {e}");
                    exit(2);
                }
            },
            LinkCommands::Resolved { r_ref } => match ResourceRef::parse(&r_ref) {
                Ok(parsed_ref) => match LinkUseCase::query_resolved_relations(&service, &parsed_ref) {
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
                        exit(exit_code_for(&e));
                    }
                },
                Err(e) => {
                    eprintln!("Invalid ref parameter: {e}");
                    exit(2);
                }
            },
            LinkCommands::Diagnose { r_ref } => match ResourceRef::parse(&r_ref) {
                Ok(parsed_ref) => match LinkUseCase::diagnose_link(&service, &parsed_ref) {
                    Ok(diags) => {
                        if cli.json {
                            println!("{}", json!(diags));
                        } else {
                            for d in diags {
                                println!(
                                    "L{} {} -> {:?} (candidates={})",
                                    d.occurrence.span.line,
                                    d.occurrence.target,
                                    d.status,
                                    d.candidates.len()
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error diagnosing links: {e}");
                        exit(exit_code_for(&e));
                    }
                },
                Err(e) => {
                    eprintln!("Invalid ref parameter: {e}");
                    exit(2);
                }
            },
            LinkCommands::Reindex { space_root } => match LinkUseCase::reindex_links(&mut service, &space_root) {
                Ok(report) => {
                    if cli.json {
                        println!("{}", json!(report));
                    } else {
                        println!(
                            "scanned={} resolved={} unresolved={} ambiguous={} external={} invalid={}",
                            report.scanned,
                            report.resolved,
                            report.unresolved,
                            report.ambiguous,
                            report.external,
                            report.invalid
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Error reindexing links: {e}");
                    exit(exit_code_for(&e));
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
            if let Some(src) = args.source {
                selector.source_id = Some(src);
            }

            match ResourceUseCase::query(&service, &selector) {
                Ok(page) => {
                    let mut items = page.items;
                    if items.len() > args.limit {
                        items.truncate(args.limit);
                    }
                    if cli.json {
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

        Commands::Recent { limit } => match ResourceUseCase::list_recent(&service, limit) {
            Ok(items) => {
                if cli.json {
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
        },

        Commands::Resource(sub) => match sub.command {
            commands::ResourceCommands::Upsert { from } => {
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
                match ResourceUseCase::upsert_resource(&mut service, res.clone()) {
                    Ok(()) => {
                        if cli.json {
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
            commands::ResourceCommands::Delete { r_ref } => {
                let parsed = match ResourceRef::parse(&r_ref) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Invalid resource ref '{r_ref}': {e}");
                        exit(2);
                    }
                };
                match ResourceUseCase::delete_resource(&mut service, &parsed) {
                    Ok(()) => {
                        if cli.json {
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
            commands::ResourceCommands::Ls { source, limit } => {
                match ResourceUseCase::list_by_source(&service, &source, limit) {
                    Ok(items) => {
                        if cli.json {
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
        },

        Commands::Read { r_ref } => {
            let parsed_ref = match ResourceRef::parse(&r_ref) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Invalid resource ref format '{r_ref}': {e}");
                    exit(2);
                }
            };

            match ResourceUseCase::read(&service, &parsed_ref) {
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
                    exit(exit_code_for(&e));
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
                match InspectUseCase::inspect_rules(&service, &parsed_ref) {
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
                        exit(exit_code_for(&e));
                    }
                }
            } else {
                match ResourceUseCase::read(&service, &parsed_ref) {
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
                        exit(exit_code_for(&e));
                    }
                }
            }
        }

        Commands::Agenda => match TaskUseCase::agenda(&service) {
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
                exit(exit_code_for(&e));
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

                    match TaskUseCase::transition_task(&mut service, &parsed_ref, &to, &timestamp) {
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
                            exit(exit_code_for(&e));
                        }
                    }
                }
                commands::TaskCommands::Detail { r_ref } => {
                    let parsed_ref = match ResourceRef::parse(&r_ref) {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("Invalid resource ref format '{r_ref}': {e}");
                            exit(2);
                        }
                    };
                    match ResourceUseCase::read(&service, &parsed_ref) {
                        Ok(Some(res)) => {
                            if cli.json {
                                println!("{}", serde_json::to_string(&res).unwrap());
                            } else {
                                println!("=== Task Record ===");
                                println!("Ref:      {}", res.r#ref);
                                println!("Title:    {}", res.title);
                                println!("Locator:  {}", res.locator);
                                println!("Revision: {}", res.revision);
                                if !res.properties.is_empty() {
                                    println!("Properties:");
                                    for (k, v) in &res.properties {
                                        println!("  {k}: {v}");
                                    }
                                }
                                
                                println!("\n=== Content ===");
                                let direct_path = std::path::PathBuf::from(&res.locator);
                                let file_path = if direct_path.exists() { direct_path } else { r_config.space_root.join(&res.locator) };
                                if let Ok(content) = std::fs::read_to_string(&file_path) {
                                    if res.kind == notez_core::domain::ResourceKind::Document {
                                        println!("{content}");
                                    } else if res.kind == notez_core::domain::ResourceKind::Heading {
                                        let level_str = res.properties.get("LEVEL").cloned().unwrap_or_else(|| "1".to_string());
                                        let target_level: usize = level_str.parse().unwrap_or(1);
                                        let mut printing = false;
                                        let is_org = res.locator.ends_with(".org");
                                        
                                        for line in content.lines() {
                                            let trimmed = line.trim_start();
                                            let (is_heading, level, text) = if is_org && trimmed.starts_with('*') && trimmed.contains(' ') {
                                                let stars = trimmed.chars().take_while(|c| *c == '*').count();
                                                (true, stars, trimmed[stars..].trim())
                                            } else if !is_org && trimmed.starts_with('#') && trimmed.contains(' ') {
                                                let hashes = trimmed.chars().take_while(|c| *c == '#').count();
                                                (true, hashes, trimmed[hashes..].trim())
                                            } else {
                                                (false, 0, "")
                                            };

                                            if is_heading {
                                                if !printing && text.contains(&res.title) {
                                                    printing = true;
                                                } else if printing && level <= target_level {
                                                    // Reached next heading of same or higher level
                                                    break;
                                                }
                                            }
                                            
                                            if printing {
                                                println!("{line}");
                                            }
                                        }
                                        
                                        if !printing {
                                            println!("(Could not locate heading content in file)");
                                        }
                                    } else {
                                        println!("(Content extraction for {} is not supported in CLI detail view)", res.kind);
                                    }
                                } else {
                                    println!("(Unable to read source file at {:?})", file_path);
                                }
                            }
                        }
                        Ok(None) => {
                            eprintln!("Task not found: {r_ref}");
                            exit(3);
                        }
                        Err(e) => {
                            eprintln!("Detail error: {e}");
                            exit(exit_code_for(&e));
                        }
                    }
                }
                commands::TaskCommands::List => match TaskUseCase::agenda(&service) {
                    Ok(agenda) => {
                        if cli.json {
                            println!("{}", serde_json::to_string(&agenda).unwrap());
                        } else {
                            // Group by TODO state
                            let mut by_state: std::collections::BTreeMap<String, Vec<&notez_core::application::task_para::AgendaItem>> = std::collections::BTreeMap::new();
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
                        exit(exit_code_for(&e));
                    }
                },
                commands::TaskCommands::Agenda => match TaskUseCase::agenda(&service) {
                    Ok(agenda) => {
                        if cli.json {
                            println!("{}", serde_json::to_string(&agenda).unwrap());
                        } else {
                            // Extract YYYY-MM-DD and group
                            let mut by_date: std::collections::BTreeMap<String, Vec<(&notez_core::application::task_para::AgendaItem, String)>> = std::collections::BTreeMap::new();
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
                        exit(exit_code_for(&e));
                    }
                },
                commands::TaskCommands::Para => match TaskUseCase::para_overview(&service) {
                    Ok(para) => {
                        if cli.json {
                            println!("{}", serde_json::to_string(&para).unwrap());
                        } else {
                            let print_nodes = |nodes: Vec<notez_core::application::task_para::ParaNode>| {
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
                        exit(exit_code_for(&e));
                    }
                },
                commands::TaskCommands::Jobs => match InspectUseCase::list_jobs(&service) {
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
                        exit(exit_code_for(&e));
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
                include,
                exclude,
            } => {
                let config = notez_core::source::SourceConfig {
                    id,
                    kind: kind.into(),
                    path,
                    read_only,
                    include_paths: include,
                    exclude_paths: exclude,
                };
                match service.add_source(&r_config.space_root, config) {
                    Ok(_) => {
                        if cli.json {
                            println!("{}", json!({ "added": true }));
                        } else {
                            println!("Source added successfully.");
                        }
                    }
                    Err(e) => {
                        eprintln!("Add source error: {e}");
                        exit(exit_code_for(&e));
                    }
                }
            }

            commands::SourceCommands::List => match service.list_sources(&r_config.space_root) {
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
                    exit(exit_code_for(&e));
                }
            },

            commands::SourceCommands::Sync => match ScanUseCase::scan_federation(&mut service, &r_config.space_root) {
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
                    exit(exit_code_for(&e));
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
                        exit(exit_code_for(&e));
                    }
                }
            }
        },
        Commands::Attachment(commands::AttachmentSubcommand { command }) => match command {
            commands::AttachmentCommands::Add { path, mime } => {
                match AttachmentUseCase::add_attachment(&mut service, &r_config.space_root, &path, &mime) {
                    Ok(att_ref) => {
                        if cli.json {
                            println!("{}", json!({ "ref": att_ref.to_string() }));
                        } else {
                            println!("{att_ref}");
                        }
                    }
                    Err(e) => {
                        eprintln!("Add attachment error: {e}");
                        exit(exit_code_for(&e));
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

                match AttachmentUseCase::run_extraction(&mut service, &r_config.space_root, &parsed_ref) {
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
                        exit(exit_code_for(&e));
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

                match AttachmentUseCase::query_segments(&service, &parsed_ref) {
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
                        exit(exit_code_for(&e));
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

                let comm = notez_core::domain::community::Community {
                    id,
                    name,
                    selector,
                    pinned_members: vec![],
                    excluded_members: vec![],
                };

                match CommunityUseCase::create_community(&service, &r_config.space_root, comm) {
                    Ok(_) => {
                        if cli.json {
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

            commands::CommunityCommands::List => match CommunityUseCase::list_communities(&service, &r_config.space_root) {
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
                    exit(exit_code_for(&e));
                }
            },
        },

        Commands::Derive(commands::DeriveArgs { community, recipe }) => {
            match ArtifactUseCase::derive_artifact(&mut service, &r_config.space_root, &community, &recipe) {
                Ok(derived) => {
                    if cli.json {
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

        Commands::Skill(commands::SkillSubcommand {
            command:
                commands::SkillCommands::Export {
                    community,
                    description,
                    out,
                },
        }) => match ArtifactUseCase::export_skill(&mut service, &r_config.space_root, &community, &description, &out) {
            Ok(pkg) => {
                if cli.json {
                    println!("{}", serde_json::to_string(&pkg).unwrap());
                } else {
                    println!("Exported skill package to {:?}", pkg.package_path);
                }
            }
            Err(e) => {
                eprintln!("Skill export error: {e}");
                exit(exit_code_for(&e));
            }
        },
        Commands::Sync(commands::SyncSubcommand { command }) => match command {
            commands::SyncCommands::Push { actor, folder } => {
                match SyncUseCase::sync_push(&mut service, &actor, &r_config.space_root, &folder) {
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
                        exit(exit_code_for(&e));
                    }
                }
            }

            commands::SyncCommands::Pull { actor, folder } => {
                match SyncUseCase::sync_pull(&mut service, &actor, &r_config.space_root, &folder) {
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
                        exit(exit_code_for(&e));
                    }
                }
            }

            commands::SyncCommands::Conflicts => match SyncUseCase::list_conflicts(&service) {
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
                    exit(exit_code_for(&e));
                }
            },
            commands::SyncCommands::Relay { id } => match SyncUseCase::relay_sync(&service, &id, &r_config.space_root) {
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
                    exit(exit_code_for(&e));
                }
            },
        },

        Commands::Artifact(commands::ArtifactSubcommand {
            command: commands::ArtifactCommands::Stale,
        }) => match InspectUseCase::check_artifact_freshness(&service, &r_config.space_root) {
            Ok(stale_report) => {
                if cli.json {
                    println!("{}", serde_json::to_string(&stale_report).unwrap());
                } else {
                    println!("Artifact Freshness: {}", stale_report.status);
                }
            }
            Err(e) => {
                eprintln!("Artifact freshness check error: {e}");
                exit(exit_code_for(&e));
            }
        },
        Commands::Mcp(McpSubcommand {
            command: McpCommands::Serve,
        }) => {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("Tokio runtime error: {e}");
                    exit(5);
                }
            };
            if let Err(e) = runtime.block_on(notez_cli::mcp::serve(service)) {
                eprintln!("MCP server error: {e}");
                exit(5);
            }
        }

        Commands::Space(SpaceSubcommand { command }) => match command {
            SpaceCommands::Rebuild => match service.rebuild(&r_config.space_root) {
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
                    exit(exit_code_for(&e));
                }
            },

            SpaceCommands::Doctor => match InspectUseCase::space_doctor(&service, &r_config.space_root) {
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
                    exit(exit_code_for(&e));
                }
            },
            SpaceCommands::List => {
                let mut spaces = Vec::new();
                if let Some(global) = &paths.global_config {
                    for (name, reg) in &global.spaces {
                        spaces.push(json!({ "name": name, "path": reg.path }));
                    }
                }
                if cli.json {
                    println!("{}", json!({ "spaces": spaces }));
                } else {
                    for s in spaces {
                        println!("{}: {}", s["name"].as_str().unwrap(), s["path"].as_str().unwrap());
                    }
                }
            }
            SpaceCommands::Register { name, path } => {
                let mut global = paths.global_config.clone().unwrap_or_else(|| {
                    notez_core::config::GlobalConfig {
                        version: 1,
                        default_space: None,
                        spaces: std::collections::BTreeMap::new(),
                        preferences: Default::default(),
                    }
                });
                let root = path.unwrap_or_else(|| cwd.clone());
                let reg = notez_core::config::SpaceRegistration {
                    path: root,
                    config: None,
                };
                global.spaces.insert(name.clone(), reg);
                if global.default_space.is_none() {
                    global.default_space = Some(name.clone());
                }
                let text = toml::to_string(&global).unwrap();
                let cfg_path = paths.global.clone();
                if let Some(p) = cfg_path.parent() {
                    fs::create_dir_all(p).unwrap();
                }
                // atomic write
                let tmp = cfg_path.with_extension("tmp");
                fs::write(&tmp, text).unwrap();
                fs::rename(tmp, cfg_path).unwrap();
                if cli.json {
                    println!("{}", json!({ "registered": name }));
                } else {
                    println!("Registered space '{}'", name);
                }
            }
            SpaceCommands::Unregister { name } => {
                if let Some(mut global) = paths.global_config.clone() {
                    global.spaces.remove(&name);
                    if global.default_space.as_deref() == Some(&name) {
                        global.default_space = None;
                    }
                    let text = toml::to_string(&global).unwrap();
                    let tmp = paths.global.with_extension("tmp");
                    fs::write(&tmp, text).unwrap();
                    fs::rename(tmp, &paths.global).unwrap();
                }
                if cli.json {
                    println!("{}", json!({ "unregistered": name }));
                } else {
                    println!("Unregistered space '{}'", name);
                }
            }
        },
        Commands::Config(commands::ConfigSubcommand { command }) => match command {
            commands::ConfigCommands::Show => {
                if cli.json {
                    println!("{}", serde_json::to_string(&r_config).unwrap());
                } else {
                    println!("Space Name: {}", r_config.space_name);
                    println!("Space Root: {}", r_config.space_root.display());
                    println!("Database:   {}", r_config.database.display());
                    println!("Sources:    {}", r_config.sources.len());
                }
            }
            commands::ConfigCommands::Validate => {
                if cli.json {
                    println!("{}", json!({ "valid": true }));
                } else {
                    println!("Configuration valid.");
                }
            }
            commands::ConfigCommands::Migrate { apply } => {
                match notez_core::config::migrate::plan_legacy_migration(&r_config.space_root, &selected.space_config) {
                    Ok(plan) => {
                        if apply {
                            if let Err(e) = notez_core::config::migrate::apply_legacy_migration(
                                &r_config.space_root,
                                selected.space_config.clone(),
                                plan,
                            ) {
                                eprintln!("Migration error: {e}");
                                exit(5);
                            }
                            println!("Migration applied.");
                        } else {
                            if cli.json {
                                println!(
                                    "{}",
                                    json!({
                                        "sources_to_add": plan.sources_to_add.len(),
                                        "communities_to_extract": plan.communities_to_extract.len(),
                                    })
                                );
                            } else {
                                println!("Would merge {} source(s).", plan.sources_to_add.len());
                                println!(
                                    "Would extract {} community(s).",
                                    plan.communities_to_extract.len()
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Migration error: {e}");
                        exit(5);
                    }
                }
            }
        },
        #[cfg(feature = "web")]
        Commands::Web(args) => {
            let state = match web::state::WebState::from_space(&r_config.space_root) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("WebState error: {e}");
                    exit(5);
                }
            };
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("Tokio runtime error: {e}");
                    exit(5);
                }
            };
            if let Err(e) = runtime.block_on(web::server::serve(state, &args.bind, args.open)) {
                eprintln!("Web server error: {e}");
                exit(5);
            }
        }
        Commands::ListCapabilities => {
            // Handled by the early-return block above; reaching here is
            // unreachable unless the flag was somehow lost. Defensive
            // exit with a clear message.
            eprintln!("list-capabilities was consumed before this point");
            std::process::exit(1);
        }
    }
}
