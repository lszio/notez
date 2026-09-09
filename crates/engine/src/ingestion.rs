use crate::PortBundle;
use notez_ports::{WatchBatch, WatchIngestion};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub struct IngestionService {
    state: Mutex<State>,
    ports: Arc<PortBundle>,
}

struct State {
    seen: HashSet<String>,
    changes: usize,
    last_batch: Option<String>,
}

impl IngestionService {
    pub fn new(ports: Arc<PortBundle>) -> Self {
        Self { state: Mutex::new(State { seen: HashSet::new(), changes: 0, last_batch: None }), ports }
    }

    pub fn accept_with_revision(&self, batch: WatchBatch, expected_revision: Option<&str>) -> IngestionReceipt {
        if batch.batch_id.is_empty() || batch.source_id.is_empty() || batch.events.is_empty() || batch.events.iter().any(|event| {
            event.locator.is_empty() || event.observed_revision.is_empty() || event.observed_at.is_empty()
        }) {
            return IngestionReceipt::Rejected;
        }
        let mut state = self.state.lock().unwrap();
        if state.seen.contains(&batch.batch_id) { return IngestionReceipt::Duplicate; }
        if let Some(source) = &self.ports.source {
            let snapshot = match source.read(&batch.source_id) {
                Ok(snapshot) => snapshot,
                Err(_) => return IngestionReceipt::Retryable,
            };
            if let Some(expected) = expected_revision {
                if snapshot.revision != expected || batch.events.iter().any(|event| event.observed_revision != expected) {
                    return IngestionReceipt::Retryable;
                }
            }
        }
        state.seen.insert(batch.batch_id.clone());
        state.changes += 1;
        state.last_batch = Some(batch.batch_id);
        IngestionReceipt::Accepted
    }

    pub fn change_count(&self) -> usize { self.state.lock().unwrap().changes }
    #[allow(dead_code)]
    pub fn last_batch_id(&self) -> Option<String> { self.state.lock().unwrap().last_batch.clone() }
}

impl WatchIngestion for IngestionService {
    fn ingest(&self, batch: WatchBatch) -> Result<(), notez_ports::WatchIngestionError> {
        match self.accept_with_revision(batch, None) {
            IngestionReceipt::Accepted => Ok(()),
            IngestionReceipt::Duplicate => Err(notez_ports::WatchIngestionError::Duplicate),
            IngestionReceipt::Rejected => Err(notez_ports::WatchIngestionError::Rejected { message: "invalid batch".into() }),
            IngestionReceipt::Retryable => Err(notez_ports::WatchIngestionError::Retryable { message: "source reread failed or revision changed".into() }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestionReceipt { Accepted, Duplicate, Rejected, Retryable }
