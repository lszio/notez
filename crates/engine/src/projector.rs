use crate::{ports, PortBundle};

pub struct Projector<'a> { pub ports: &'a PortBundle }
impl<'a> Projector<'a> {
    pub fn new(ports: &'a PortBundle) -> Self { Self { ports } }
    pub fn append(&self, payload: &[u8]) -> Result<(), ports::JournalError> {
        if let Some(journal) = &self.ports.journal { journal.append(payload).map(|_| ()) } else { Ok(()) }
    }
}
