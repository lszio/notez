//! Task use case: agenda, PARA overview, and state transitions.

use crate::application::ApplicationError;
use crate::application::task_para::{AgendaView, ParaOverview};
use crate::document::StateTransition;
use crate::domain::ResourceRef;
use notez_protocol::request::RevisionPrecondition;

pub trait TaskUseCase {
    fn agenda(&self) -> Result<AgendaView, ApplicationError>;
    fn para_overview(&self) -> Result<ParaOverview, ApplicationError>;
    fn transition_task(
        &mut self,
        r_ref: &ResourceRef,
        to_state: &str,
        timestamp: &str,
        precondition: RevisionPrecondition,
    ) -> Result<StateTransition, ApplicationError>;
}
