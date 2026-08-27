//! Handlers for `Commands::Agenda` (legacy alias) and `Commands::Task`
//! (`TaskCommands::{Transition, List, Detail, Agenda, Para, Jobs}`).
//!
//! Every arm translates the parsed CLI syntax into a protocol
//! `Request`, dispatches it through the [`ApplicationDispatcher`], and
//! renders the unwrapped response payload.

use serde_json::json;
use std::process::exit;

use super::{Service, exit_code_for};
use crate::commands;
use notez_core::application::dispatcher::{ApplicationDispatcher, Response};
use notez_protocol::response::{AgendaItem, ParaNode, ResourceKind};
use notez_protocol::request::{
    AgendaRequest, ListJobsRequest, ParaOverviewRequest, ReadResourceRequest,
    Request, TransitionTaskRequest,
};

pub fn run_agenda(json: bool, service: &mut Service) {
    match ApplicationDispatcher::new(service).dispatch(Request::Agenda(AgendaRequest {})) {
        Ok(Response::Agenda(agenda)) => {
            if json {
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
        other => unreachable!("unexpected dispatcher response: {other:?}"),
    }
}


pub fn run_task(
    json: bool,
    service: &mut Service,
    sub: commands::TaskSubcommand,
    source_root: &std::path::Path,
) {
    let mut dispatcher = ApplicationDispatcher::new(service);
    let cmd = sub.command.unwrap_or(commands::TaskCommands::Agenda);
    match cmd {
        commands::TaskCommands::Transition {
            r_ref,
            to,
            timestamp,
        } => {
            let dispatched = dispatcher.dispatch(Request::TransitionTask(TransitionTaskRequest {
                r_ref: r_ref.clone(),
                to_state: to,
                timestamp,
                expected_revision: None,
            }));
            match dispatched {
                Ok(Response::Transition(transition)) => {
                    if json {
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
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
        commands::TaskCommands::Detail { r_ref } => {
            let dispatched = dispatcher.dispatch(Request::ReadResource(ReadResourceRequest {
                r_ref: r_ref.clone(),
            }));
            match dispatched {
                Ok(Response::Resource(Some(res))) => {
                    if json {
                        println!("{}", serde_json::to_string(&res).unwrap());
                    } else {
                        println!("=== Task Record ===");
                        println!("Ref:      {}", res.ref_);
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
                        let file_path = if direct_path.exists() {
                            direct_path
                        } else {
                            source_root.join(&res.locator)
                        };
                        if let Ok(content) = std::fs::read_to_string(&file_path) {
                            if res.kind == ResourceKind::Document {
                                println!("{content}");
                            } else if res.kind == ResourceKind::Heading {
                                let level_str = res
                                    .properties
                                    .get("LEVEL")
                                    .cloned()
                                    .unwrap_or_else(|| "1".to_string());
                                let target_level: usize = level_str.parse().unwrap_or(1);
                                let mut printing = false;
                                let is_org = res.locator.ends_with(".org");

                                for line in content.lines() {
                                    let trimmed = line.trim_start();
                                    let (is_heading, level, text) = if is_org
                                        && trimmed.starts_with('*')
                                        && trimmed.contains(' ')
                                    {
                                        let stars =
                                            trimmed.chars().take_while(|c| *c == '*').count();
                                        (true, stars, trimmed[stars..].trim())
                                    } else if !is_org
                                        && trimmed.starts_with('#')
                                        && trimmed.contains(' ')
                                    {
                                        let hashes =
                                            trimmed.chars().take_while(|c| *c == '#').count();
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
                                println!(
                                    "(Content extraction for {:?} is not supported in CLI detail view)",
                                    res.kind
                                );
                            }
                        } else {
                            println!("(Unable to read source file at {:?})", file_path);
                        }
                    }
                }
                Ok(Response::Resource(None)) => {
                    eprintln!("Task not found: {r_ref}");
                    exit(3);
                }
                Err(e) => {
                    eprintln!("Detail error: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
        commands::TaskCommands::List => {
            let dispatched = dispatcher.dispatch(Request::Agenda(AgendaRequest {}));
            match dispatched {
                Ok(Response::Agenda(agenda)) => {
                    if json {
                        println!("{}", serde_json::to_string(&agenda).unwrap());
                    } else {
                        let mut by_state: std::collections::BTreeMap<
                            String,
                            Vec<&AgendaItem>,
                        > = std::collections::BTreeMap::new();
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
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
        commands::TaskCommands::Agenda => {
            let dispatched = dispatcher.dispatch(Request::Agenda(AgendaRequest {}));
            match dispatched {
                Ok(Response::Agenda(agenda)) => {
                    if json {
                        println!("{}", serde_json::to_string(&agenda).unwrap());
                    } else {
                        let mut by_date: std::collections::BTreeMap<
                            String,
                            Vec<(&AgendaItem, String)>,
                        > = std::collections::BTreeMap::new();
                        let mut unscheduled = Vec::new();
                        let mut completed = Vec::new();

                        let extract_date = |s: &str| -> Option<String> {
                            // Look for YYYY-MM-DD inside < > or [ ]
                            let s =
                                s.trim_matches(|c| c == '<' || c == '>' || c == '[' || c == ']');
                            if s.len() >= 10 {
                                Some(s[0..10].to_string())
                            } else {
                                None
                            }
                        };

                        for item in &agenda.items {
                            let is_done = item.todo.as_deref() == Some("DONE")
                                || item.todo.as_deref() == Some("QUIT");

                            if is_done {
                                let date_str = item
                                    .closed
                                    .as_deref()
                                    .or(item.deadline.as_deref())
                                    .or(item.scheduled.as_deref())
                                    .unwrap_or("");
                                completed.push((item, date_str.to_string()));
                                continue;
                            }

                            let mut has_date = false;
                            if let Some(ref d) = item.deadline {
                                if let Some(date) = extract_date(d) {
                                    by_date
                                        .entry(date)
                                        .or_default()
                                        .push((item, format!("Deadline: {}", d)));
                                    has_date = true;
                                }
                            }
                            if let Some(ref s) = item.scheduled {
                                if let Some(date) = extract_date(s) {
                                    by_date
                                        .entry(date)
                                        .or_default()
                                        .push((item, format!("Scheduled: {}", s)));
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
                                let state_str = if todo.is_empty() {
                                    String::new()
                                } else {
                                    format!("[{}] ", todo)
                                };
                                println!("  {time_info:<25} | {state_str}{}", item.title);
                            }
                        }
                        if !unscheduled.is_empty() {
                            println!("\n=== Unscheduled Active Tasks ===");
                            for item in unscheduled {
                                let todo = item.todo.as_deref().unwrap_or("");
                                let state_str = if todo.is_empty() {
                                    String::new()
                                } else {
                                    format!("[{}] ", todo)
                                };
                                println!("  {state_str}{}", item.title);
                            }
                        }

                        if !completed.is_empty() {
                            println!("\n=== Completed ===");
                            // Sort completed roughly by closed date
                            completed.sort_by(|a, b| b.1.cmp(&a.1));
                            for (item, date_str) in completed {
                                let state_str =
                                    format!("[{}] ", item.todo.as_deref().unwrap_or("DONE"));
                                let time_info = if date_str.is_empty() {
                                    String::new()
                                } else {
                                    format!(" Closed: {:<20} |", date_str)
                                };
                                println!(" {time_info} {state_str}{}", item.title);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Agenda error: {e}");
                    exit(exit_code_for(&e));
                }
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
        commands::TaskCommands::Para => {
            let dispatched = dispatcher.dispatch(Request::ParaOverview(ParaOverviewRequest {}));
            match dispatched {
                Ok(Response::Para(para)) => {
                    if json {
                        println!("{}", serde_json::to_string(&para).unwrap());
                    } else {
                        let print_nodes =
                            |nodes: Vec<ParaNode>| {
                                for node in nodes {
                                    let todo_str = node
                                        .resource
                                        .properties
                                        .get("TODO")
                                        .map(|s| format!("[{}] ", s))
                                        .unwrap_or_default();
                                    println!(
                                        "- {}{} ({})",
                                        todo_str, node.resource.title, node.resource.ref_
                                    );
                                    for task in node.tasks {
                                        let t_todo = task
                                            .todo
                                            .map(|s| format!("[{}] ", s))
                                            .unwrap_or_default();
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
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
        commands::TaskCommands::Jobs => {
            let dispatched = dispatcher.dispatch(Request::ListJobs(ListJobsRequest {}));
            match dispatched {
                Ok(Response::Jobs(jobs)) => {
                    if json {
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
                other => unreachable!("unexpected dispatcher response: {other:?}"),
            }
        }
    }
}
