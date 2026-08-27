//! Handler for `Commands::Workspace` (`rebuild`, `doctor`, `list`,
//! `register`, `unregister`) and `Commands::Config` (`show`,
//! `validate`, `migrate`). Both are admin surfaces operating on the
//! global registry / runtime configuration rather than resources.

use serde_json::json;
use std::fs;
use std::path::Path;
use std::process::exit;

use super::{exit_code_for, Service};
use notez_core::config::{SelectedSource, SourceConfig};
use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_protocol::request::{Request, ScanNativeRequest, SourceDoctorRequest};
use crate::commands::{ConfigCommands, ConfigSubcommand, WorkspaceCommands, WorkspaceSubcommand};
pub fn run_workspace(json: bool, service: &mut Service, sub: WorkspaceSubcommand, cwd: &Path) {
    let WorkspaceSubcommand { command } = sub;
    // Admin commands operate on the global registry; re-discover
    // the config paths locally instead of threading them through
    // every command arm.
    let env: std::collections::BTreeMap<String, std::ffi::OsString> = std::env::vars_os()
        .map(|(k, v)| (k.to_string_lossy().to_string(), v))
        .collect();
    let paths = notez_core::config::ConfigPaths::discover(&env, cwd).unwrap_or_else(|e| {
        eprintln!("Configuration error: {e}");
        exit(2);
    });
    match command {
        WorkspaceCommands::Rebuild => match ApplicationDispatcher::new(service)
            .dispatch(Request::ScanNative(ScanNativeRequest {}))
        {
            Ok(Response::Scan(report)) => {
                if json {
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
                exit(exit_code_for(&e));
            }
            other => unreachable!("unexpected dispatcher response: {other:?}"),
        },

        WorkspaceCommands::Doctor => match ApplicationDispatcher::new(service)
            .dispatch(Request::SourceDoctor(SourceDoctorRequest {}))
        {
            Ok(Response::Doctor(report)) => {
                if json {
                    println!("{}", serde_json::to_string(&report).unwrap());
                } else {
                    println!("Source Doctor Status: {}", report.status);
                    for issue in report.issues {
                        println!("[{}] {}: {}", issue.severity, issue.code, issue.message);
                    }
                }
            }
            Err(e) => {
                eprintln!("Source doctor error: {e}");
                exit(exit_code_for(&e));
            }
            other => unreachable!("unexpected dispatcher response: {other:?}"),
        },

        WorkspaceCommands::List => {
            let mut sources = Vec::new();
            if let Some(global) = &paths.global_config {
                for (name, reg) in &global.sources {
                    sources.push(json!({ "name": name, "path": reg.path }));
                }
            }
            if json {
                println!("{}", json!({ "sources": sources }));
            } else {
                for s in sources {
                    println!(
                        "{}: {}",
                        s["name"].as_str().unwrap(),
                        s["path"].as_str().unwrap()
                    );
                }
            }
        }
        WorkspaceCommands::Register { name, path } => {
            let mut global =
                paths
                    .global_config
                    .clone()
                    .unwrap_or_else(|| notez_core::config::GlobalConfig {
                        version: notez_core::config::CURRENT_VERSION,
                        default_source: None,
                        sources: std::collections::BTreeMap::new(),
                        preferences: Default::default(),
                    });
            let root = path.unwrap_or_else(|| cwd.to_path_buf());
            let reg = notez_core::config::SourceRegistration {
                path: root,
                config: None,
            };
            global.sources.insert(name.clone(), reg);
            if global.default_source.is_none() {
                global.default_source = Some(name.clone());
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
            if json {
                println!("{}", json!({ "registered": name }));
            } else {
                println!("Registered source '{}'", name);
            }
        }
        WorkspaceCommands::Unregister { name } => {
            if let Some(mut global) = paths.global_config.clone() {
                global.sources.remove(&name);
                if global.default_source.as_deref() == Some(&name) {
                    global.default_source = None;
                }
                let text = toml::to_string(&global).unwrap();
                let tmp = paths.global.with_extension("tmp");
                fs::write(&tmp, text).unwrap();
                fs::rename(tmp, &paths.global).unwrap();
            }
            if json {
                println!("{}", json!({ "unregistered": name }));
            } else {
                println!("Unregistered source '{}'", name);
            }
        }
    }
}

pub fn run_config(
    json: bool,
    r_config: &SourceConfig,
    source_root: &Path,
    selected: &SelectedSource,
    sub: ConfigSubcommand,
) {
    match sub.command {
        ConfigCommands::Show => {
            if json {
                println!("{}", serde_json::to_string(r_config).unwrap());
            } else {
                println!("Source Name: {}", r_config.source.name);
                println!("Source Root: {}", source_root.display());
                println!("Database:   {}", r_config.source.database.display());
                println!("Sources:    {}", r_config.sources.len());
            }
        }
        ConfigCommands::Validate => {
            if json {
                println!("{}", json!({ "valid": true }));
            } else {
                println!("Configuration valid.");
            }
        }
        ConfigCommands::Migrate { apply } => {
            match notez_core::config::migrate::plan_legacy_migration(
                source_root,
                &selected.source_config,
            ) {
                Ok(plan) => {
                    if apply {
                        if let Err(e) = notez_core::config::migrate::apply_legacy_migration(
                            source_root,
                            selected.source_config.clone(),
                            plan,
                        ) {
                            eprintln!("Migration error: {e}");
                            exit(5);
                        }
                        println!("Migration applied.");
                    } else {
                        if json {
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
    }
}
