//! InspectUseCase implementation for `Engine`.
//!
//! Method bodies were previously inlined in `service.rs`; this file
//! is part of the 0.5.x-A1+A3 use-case impl split.

use crate::application::service::{ApplicationError, Engine, StorageErrorKind};
use crate::domain::{ProjectionReader, ProjectionWrite};
use crate::application::use_cases::InspectUseCase;
use crate::domain::{InspectResult, ProjectionStore, ResourceRef};

impl<S> InspectUseCase for Engine<S>
where
    S: ProjectionStore,
    S: ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>, {
    fn inspect_rules(
        &self,
        r_ref: &ResourceRef,
    ) -> Result<Option<crate::domain::InspectResult>, ApplicationError> {
        let res = self
            .store
            .get(r_ref)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            })?;
        Ok(res.map(|r| self.rule_engine.evaluate(&r)))
    }

    fn source_doctor(&self) -> Result<crate::application::doctor::DoctorReport, ApplicationError> {
        let _space_root = self.require_space_root()?;
        Err(ApplicationError::UnsupportedCapability {
            capability: "space doctor is not yet implemented",
        })
    }

    fn list_jobs(
        &self,
    ) -> Result<Vec<crate::application::job_manager::JobRecord>, ApplicationError> {
        Err(ApplicationError::UnsupportedCapability {
            capability: "job manager is not yet implemented; jobs are tracked via `notez task`",
        })
    }
    fn check_artifact_freshness(
        &self,
    ) -> Result<crate::application::job_manager::ArtifactStaleReport, ApplicationError> {
        let _space_root = self.require_space_root()?;
        Err(ApplicationError::UnsupportedCapability {
            capability: "artifact freshness check is not yet implemented",
        })
    }
}
