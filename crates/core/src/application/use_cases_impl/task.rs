//! TaskUseCase impl for `ApplicationFacade`.
//!
//! Method bodies were previously inlined in `service.rs`; this file
//! is part of the 0.5.x-A1+A3 use-case impl split.

use crate::application::service::{ApplicationError, ApplicationFacade, DocumentErrorKind, StorageErrorKind};
use crate::application::write_check;
use crate::application::task_para::{AgendaItem, AgendaView, ParaNode, ParaOverview};
use crate::application::use_cases::TaskUseCase;
use crate::domain::{ProjectionStore, ResourceRef, Selector};
use crate::document::StateTransition;
use std::path::Path;

impl<S: ProjectionStore> TaskUseCase for ApplicationFacade<S> {
    fn agenda(&self) -> Result<crate::application::task_para::AgendaView, ApplicationError> {
        let page = <Self as crate::application::use_cases::ResourceUseCase>::query(self, &Selector::new())?;
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
            .map_err(|e| {
                ApplicationError::Document {
                    source: DocumentErrorKind::Org(
                        crate::document::OrgDocumentError::Other(e.to_string()),
                    ),
                }
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
        let payload = serde_json::json!({
            "action": "UpdateTaskStatus",
            "to_state": transition.to_state,
            "closed_timestamp": transition.closed_timestamp
        }).to_string();

        self.writeback_resource(&source_id, &res.locator, &payload)?;

        // Then update the local projection
        self.store
            .replace_source(&source_id, vec![res], vec![], vec![])
            .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?;

        Ok(transition)
    }

    fn para_overview(&self) -> Result<crate::application::task_para::ParaOverview, ApplicationError> {
        let page = <Self as crate::application::use_cases::ResourceUseCase>::query(self, &Selector::new())?;
        let mut projects = Vec::new();
        let mut areas = Vec::new();
        let mut resources = Vec::new();
        let mut archives = Vec::new();

        let mut parent_to_tasks: std::collections::BTreeMap<String, Vec<crate::application::task_para::AgendaItem>> = std::collections::BTreeMap::new();

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
                    parent_to_tasks.entry(parent_ref.to_string()).or_default().push(item);
                }
            }
        }

        for res in page.items {
            let inspect_res = <Self as crate::application::use_cases::InspectUseCase>::inspect_rules(self, &res.r#ref)?;
            let para_val = inspect_res
                .as_ref()
                .and_then(|i| i.derived_properties.get("para").map(|s| s.to_string()))
                .or_else(|| res.properties.get("para").map(|s| s.to_string()))
                .or_else(|| res.properties.get("TYPE").map(|s| s.to_string()));

            if let Some(pv) = para_val {
                let tasks = parent_to_tasks.remove(&res.r#ref.to_string()).unwrap_or_default();
                let node = crate::application::task_para::ParaNode { resource: res, tasks };
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
