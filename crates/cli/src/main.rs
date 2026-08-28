//! `notez` binary entry point: argument parsing, capability listing,
//! composition-root wiring, and dispatch to the `handlers` modules.

use clap::Parser;
use notez_cli::commands;
use notez_cli::commands::{Cli, Commands};
use notez_cli::handlers;
use notez_composition::native::{OpenSpaceError, open_space};
use std::process::exit;

fn main() {
    let cli = Cli::parse();

    // Match on the subcommand first. The `ListCapabilities` variant
    // short-circuits before any space or store wiring; it just prints
    // the catalog as JSON and exits 0. All other variants fall through
    // to the existing space-selection flow.
    if let Commands::ListCapabilities = cli.command {
        // The catalog is purely in-memory; no SQLite needed. Skip the
        // space / store wiring entirely.
        let store = notez_core::storage::SqliteProjection::in_memory().unwrap_or_else(|e| {
            eprintln!("Failed to open in-memory store: {e}");
            std::process::exit(5);
        });
        let facade = notez_core::application::Engine::new(store);
        let payload = facade.capabilities_json();
        match serde_json::to_string_pretty(&payload) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("Failed to serialise capabilities: {e}");
                std::process::exit(5);
            }
        }
        std::process::exit(0);
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let selector = match &cli.space {
        Some(s) => {
            if std::path::Path::new(s).is_absolute()
                || s.starts_with('.')
                || s.starts_with("~/")
                || s.contains('/')
            {
                notez_core::config::SourceSelector::Path(std::path::Path::new(s))
            } else {
                notez_core::config::SourceSelector::Name(s)
            }
        }
        None => {
            // Upward search only makes sense when we are sitting inside a
            // space that has its own notez.toml.
            let env: std::collections::BTreeMap<String, std::ffi::OsString> = std::env::vars_os()
                .map(|(k, v)| (k.to_string_lossy().to_string(), v))
                .collect();
            let has_local_config = notez_core::config::ConfigPaths::discover(&env, &cwd)
                .map(|p| p.source_config.is_some())
                .unwrap_or(false);
            if has_local_config {
                notez_core::config::SourceSelector::Upward
            } else {
                notez_core::config::SourceSelector::Default
            }
        }
    };

    // Single composition root: selection, runtime config, legacy source
    // cache merge, database placement and parser registration all happen
    // in `composition` so every transport wires identically.
    let handle = match open_space(selector, cli.db.as_deref()) {
        Ok(h) => h,
        Err(OpenSpaceError::Config(e)) => {
            eprintln!("Configuration error: {e}");
            exit(2);
        }
        Err(OpenSpaceError::Store(e)) => {
            eprintln!("Error opening database: {e}");
            exit(5);
        }
    };
    let source_root = handle.ctx.root.clone();
    let r_config = handle.ctx.config.clone();
    let selected = handle.selected;
    let mut engine = handle.engine;
    match cli.command {
        Commands::Scan => handlers::scan::run_scan(cli.json, &mut engine),
        Commands::Resolve { query } => handlers::resource::run_resolve(cli.json, &mut engine, &query),
        Commands::Link(sub) => handlers::link::run_link(cli.json, &mut engine, sub),
        Commands::Query(args) => handlers::resource::run_query(cli.json, &mut engine, args),
        Commands::Recent { limit } => handlers::resource::run_recent(cli.json, &mut engine, limit),
        Commands::Read { r_ref } => handlers::resource::run_read(cli.json, &mut engine, &r_ref),
        Commands::Inspect { r_ref, rules } => {
            handlers::resource::run_inspect(cli.json, &mut engine, &r_ref, rules)
        }
        Commands::Resource(sub) => handlers::resource::run_resource(cli.json, &mut engine, sub),
        Commands::Agenda => handlers::task::run_agenda(cli.json, &mut engine),
        Commands::Task(sub) => handlers::task::run_task(cli.json, &mut engine, sub, &source_root),
        Commands::Source(sub) => handlers::source::run_source(cli.json, &mut engine, sub),
        Commands::Attachment(sub) => {
            handlers::attachment::run_attachment(cli.json, &mut engine, sub)
        }
        Commands::Community(sub) => handlers::community::run_community(cli.json, &mut engine, sub),
        Commands::Derive(commands::DeriveArgs { community, recipe }) => {
            handlers::community::run_derive(cli.json, &mut engine, &community, &recipe)
        }
        Commands::Skill(commands::SkillSubcommand {
            command:
                commands::SkillCommands::Export {
                    community,
                    description,
                    out,
                },
        }) => handlers::community::run_skill_export(
            cli.json,
            &mut engine,
            &community,
            &description,
            &out,
        ),
        Commands::Sync(sub) => handlers::sync::run_sync(cli.json, &mut engine, sub),
        Commands::Artifact(commands::ArtifactSubcommand {
            command: commands::ArtifactCommands::Stale,
        }) => handlers::community::run_artifact_stale(cli.json, &mut engine),
        Commands::Janet {
            script,
            source_id,
            document_ref,
            timeout_ms,
            result_limit,
        } => handlers::janet::run_janet(
            cli.json,
            &mut engine,
            script,
            source_id,
            document_ref,
            timeout_ms,
            result_limit,
        ),
        Commands::Mcp(commands::McpSubcommand {
            command: commands::McpCommands::Serve,
        }) => handlers::mcp_cmd::run_mcp(engine),
        Commands::Workspace(sub) => {
            handlers::workspace::run_workspace(cli.json, &mut engine, sub, &cwd)
        }
        Commands::Config(sub) => {
            handlers::workspace::run_config(cli.json, &r_config, &source_root, &selected, sub)
        }
        Commands::ListCapabilities => {
            // Handled by the early-return block above; reaching here is
            // unreachable unless the flag was somehow lost. Defensive
            // exit with a clear message.
            eprintln!("list-capabilities was consumed before this point");
            std::process::exit(1);
        }
        Commands::Watch(args) => handlers::watch::run_watch(args, &source_root),
    }
}
