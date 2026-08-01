use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TodoStateKind {
    Active,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoState {
    pub name: String,
    pub key: Option<char>,
    pub kind: TodoStateKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateTransition {
    pub from_state: String,
    pub to_state: String,
    pub closed_timestamp: Option<String>,
    pub logbook_entry: String,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum WorkflowError {
    #[error("invalid workflow spec: {0}")]
    InvalidSpec(String),
    #[error("unknown TODO state: {0}")]
    UnknownState(String),
    #[error("invalid transition from '{from}' to '{to}'")]
    InvalidTransition { from: String, to: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowProfile {
    pub active_states: Vec<TodoState>,
    pub done_states: Vec<TodoState>,
}

impl Default for WorkflowProfile {
    fn default() -> Self {
        Self::parse("TODO NEXT PEND WAIT | DONE QUIT").unwrap()
    }
}

impl WorkflowProfile {
    pub fn parse(spec: &str) -> Result<Self, WorkflowError> {
        let mut active_states = Vec::new();
        let mut done_states = Vec::new();
        let mut is_done = false;

        for token in spec.split_whitespace() {
            if token == "|" {
                is_done = true;
                continue;
            }

            let (name, key) = if let Some(open_idx) = token.find('(') {
                let name = &token[..open_idx];
                let close_idx = token.find(')').unwrap_or(token.len());
                let inner = &token[open_idx + 1..close_idx];
                let key_char = inner.chars().next();
                (name.to_string(), key_char)
            } else {
                (token.to_string(), None)
            };

            let state = TodoState {
                name,
                key,
                kind: if is_done {
                    TodoStateKind::Done
                } else {
                    TodoStateKind::Active
                },
            };

            if is_done {
                done_states.push(state);
            } else {
                active_states.push(state);
            }
        }

        if active_states.is_empty() && done_states.is_empty() {
            return Err(WorkflowError::InvalidSpec("empty spec".to_string()));
        }

        Ok(Self {
            active_states,
            done_states,
        })
    }

    pub fn find_state(&self, name: &str) -> Option<&TodoState> {
        self.active_states
            .iter()
            .chain(self.done_states.iter())
            .find(|s| s.name.eq_ignore_ascii_case(name))
    }

    pub fn transition(
        &self,
        from_state: &str,
        to_state: &str,
        timestamp: &str,
    ) -> Result<StateTransition, WorkflowError> {
        let target_state = self
            .find_state(to_state)
            .ok_or_else(|| WorkflowError::UnknownState(to_state.to_string()))?;

        let closed_timestamp = if target_state.kind == TodoStateKind::Done {
            Some(timestamp.to_string())
        } else {
            None
        };

        let logbook_entry = format!(
            "- State \"{}\"       from \"{}\"       [{}]",
            target_state.name, from_state, timestamp
        );

        Ok(StateTransition {
            from_state: from_state.to_string(),
            to_state: target_state.name.clone(),
            closed_timestamp,
            logbook_entry,
        })
    }
}
