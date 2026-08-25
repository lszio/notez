//! Inspect use case: rule inspection, doctor, jobs, and freshness.

use crate::application::ApplicationError;
use crate::application::doctor::DoctorReport;
use crate::application::job_manager::{ArtifactStaleReport, JobRecord};
use crate::domain::{InspectResult, ResourceRef};

pub trait InspectUseCase {
    fn inspect_rules(&self, r_ref: &ResourceRef)
    -> Result<Option<InspectResult>, ApplicationError>;
    fn source_doctor(&self) -> Result<DoctorReport, ApplicationError>;
    fn list_jobs(&self) -> Result<Vec<JobRecord>, ApplicationError>;
    fn check_artifact_freshness(&self) -> Result<ArtifactStaleReport, ApplicationError>;
}
