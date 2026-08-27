//! TaskUseCase impl for `ApplicationFacade`.
//!
//! Method bodies were previously inlined in `service.rs`; this file
//! is part of the 0.5.x-A1+A3 use-case impl split.

use crate::application::service::{
    ApplicationError, ApplicationFacade, DocumentErrorKind, StorageErrorKind,
};
use crate::application::task_para::{AgendaItem, AgendaView, ParaNode, ParaOverview};
use crate::domain::{ProjectionReader, ProjectionWrite};
use crate::application::use_cases::TaskUseCase;
use crate::application::write_check;
use crate::document::StateTransition;
use crate::domain::{ProjectionStore, ResourceRef, Selector};
use std::path::Path;

impl<S> TaskUseCase for ApplicationFacade<S>
where
    S: ProjectionStore,
    S: ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>, {
    fn agenda(&self) -> Result<crate::application::task_para::AgendaView, ApplicationError> {
        let page = <Self as crate::application::use_cases::ResourceUseCase>::query(
            self,
            &Selector::new(),
        )?;
        let mut items = Vec::new();

        for res in page.items {
            let scheduled = res.properties.get("SCHEDULED").cloned();
            let deadline = res.properties.get("DEADLINE").cloned();
            let closed = res.properties.get("CLOSED").cloned();
            let todo = res.properties.get("TODO").cloned();

            if scheduled.is_some() || deadline.is_some() || todo.is_some() {
                items.push(crate::application::task_para::AgendaItem {
                    r_ref: res.r#ref.to_string(),
                    title: res.title,
                    todo,
                    scheduled,
                    deadline,
                    closed,
                    locator: res.locator,
                });
            }
        }

        Ok(crate::application::task_para::AgendaView { items })
    }

    fn transition_task(
        &mut self,
        r_ref: &ResourceRef,
        to_state: &str,
        timestamp: &str,
    ) -> Result<crate::document::StateTransition, ApplicationError> {
        write_check::check_capability(self, "task")?;
        write_check::check_address_uniqueness(
            self,
            &crate::domain::ResourceAddress::Ref { r#ref: *r_ref },
            &r_ref,
        )?;
        let mut res = <Self as crate::application::use_cases::ResourceUseCase>::read(self, r_ref)?
            .ok_or_else(|| ApplicationError::NotFound {
                kind: r_ref.kind(),
                r_ref: r_ref.clone(),
            })?;

        let current_todo = res
            .properties
            .get("TODO")
            .cloned()
            .unwrap_or_else(|| "TODO".to_string());

        let profile = crate::document::WorkflowProfile::default();
        let transition = profile
            .transition(&current_todo, to_state, timestamp)
            .map_err(|e| ApplicationError::Document {
                source: DocumentErrorKind::Org(crate::document::OrgDocumentError::Other(
                    e.to_string(),
                )),
            })?;

        res.properties
            .insert("TODO".to_string(), transition.to_state.clone());
        if let Some(ref closed_ts) = transition.closed_timestamp {
            res.properties
                .insert("CLOSED".to_string(), closed_ts.clone());
        }

        let source_id = res.source_id.clone();

        // First write back to the authoritative source
        // We serialize the state change into a JSON payload for the adapter's mutate interface
        if source_id == "native" {
            // M4 surgical write-back: patch the heading line in place
            // instead of replacing the whole file payload.
            let file_path = self.require_space_root()?.join(&res.locator);
            let content = std::fs::read_to_string(&file_path).map_err(|e| {
                ApplicationError::Io {
                    path: Some(file_path.clone()),
                    source: e.kind(),
                }
            })?;
            let file_lines: Vec<String> = content.lines().map(str::to_string).collect();
            let found = crate::source::writer::find_heading_line(&file_lines, &res.title, 1)
                .ok_or_else(|| ApplicationError::NotFound {
                    kind: crate::domain::ResourceKind::Heading,
                    r_ref: r_ref.clone(),
                })?;
            let done = matches!(transition.to_state.as_str(), "DONE" | "QUIT");
            let patch = if res.locator.ends_with(".org") {
                crate::source::writer::org_heading_state_patch(
                    found.0,
                    &found.1,
                    &transition.to_state,
                )
            } else {
                crate::source::writer::markdown_heading_state_patch(found.0, &found.1, done)
            };
            let patch = patch.ok_or_else(|| ApplicationError::UnsupportedCapability {
                capability: "task_transition_format",
            })?;
            crate::source::writer::FsSpanWriter::apply(&file_path, &[patch])
                .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?;
        } else {
            let payload = serde_json::json!({
                "action": "UpdateTaskStatus",
                "to_state": transition.to_state,
                "closed_timestamp": transition.closed_timestamp
            })
            .to_string();
            self.writeback_resource(&source_id, &res.locator, &payload)?;
        }

        // Then update the local projection through the event spine: one
        // ChangeOp::TransitionTask Change, then the source-slice swap.
        let journaling = crate::application::projector::Journaling::from_parts(
            self.journal.as_ref(),
            self.audit.as_ref(),
            self.actor_principal(),
            self.now_unix_millis(),
        );
        let mut projector =
            crate::application::projector::Projector::new(&mut self.store, &journaling);
        projector
            .replace_source_op(
                crate::domain::change::ChangeOp::TransitionTask {
                    from_state: current_todo,
                    to_state: transition.to_state.clone(),
                    timestamp: timestamp.to_string(),
                    closed_timestamp: transition.closed_timestamp.clone(),
                    logbook_entry: transition.logbook_entry.clone(),
                },
                &source_id,
                vec![res],
                vec![],
                vec![],
                serde_json::Value::Null,
            )
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            })?;

        Ok(transition)
    }

    fn para_overview(
        &self,
    ) -> Result<crate::application::task_para::ParaOverview, ApplicationError> {
        let page = <Self as crate::application::use_cases::ResourceUseCase>::query(
            self,
            &Selector::new(),
        )?;
        let mut projects = Vec::new();
        let mut areas = Vec::new();
        let mut resources = Vec::new();
        let mut archives = Vec::new();

        let mut parent_to_tasks: std::collections::BTreeMap<
            String,
            Vec<crate::application::task_para::AgendaItem>,
        > = std::collections::BTreeMap::new();

        // First pass: collect all tasks and map them to their parents
        for res in &page.items {
            let todo = res.properties.get("TODO").cloned();
            if todo.is_some() {
                if let Some(parent_ref) = res.properties.get("PARENT_REF") {
                    let item = crate::application::task_para::AgendaItem {
                        r_ref: res.r#ref.to_string(),
                        title: res.title.clone(),
                        todo,
                        scheduled: res.properties.get("SCHEDULED").cloned(),
                        deadline: res.properties.get("DEADLINE").cloned(),
                        closed: res.properties.get("CLOSED").cloned(),
                        locator: res.locator.clone(),
                    };
                    parent_to_tasks
                        .entry(parent_ref.to_string())
                        .or_default()
                        .push(item);
                }
            }
        }

        for res in page.items {
            let inspect_res =
                <Self as crate::application::use_cases::InspectUseCase>::inspect_rules(
                    self, &res.r#ref,
                )?;
            let para_val = inspect_res
                .as_ref()
                .and_then(|i| i.derived_properties.get("para").map(|s| s.to_string()))
                .or_else(|| res.properties.get("para").map(|s| s.to_string()))
                .or_else(|| res.properties.get("TYPE").map(|s| s.to_string()));

            if let Some(pv) = para_val {
                let tasks = parent_to_tasks
                    .remove(&res.r#ref.to_string())
                    .unwrap_or_default();
                let node = crate::application::task_para::ParaNode {
                    resource: res,
                    tasks,
                };
                match pv.as_str() {
                    "projects" | "project" => projects.push(node),
                    "areas" | "area" => areas.push(node),
                    "resources" | "resource" => resources.push(node),
                    "archives" | "archive" => archives.push(node),
                    _ => {}
                }
            }
        }

        Ok(crate::application::task_para::ParaOverview {
            projects,
            areas,
            resources,
            archives,
        })
    }
}
