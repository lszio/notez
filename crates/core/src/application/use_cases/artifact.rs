//! Artifact use case: derived outputs (summary, llms.txt, context-pack,
//! skill) and skill package export.

use std::path::Path;

use crate::application::ApplicationError;
use crate::artifact::{DerivedArtifact, SkillPackage};

pub trait ArtifactUseCase {
    fn derive_artifact(
        &self,
        community_id: &str,
        recipe_name: &str,
    ) -> Result<DerivedArtifact, ApplicationError>;
    fn export_skill(
        &self,
        community_id: &str,
        description: &str,
        export_path: &Path,
    ) -> Result<SkillPackage, ApplicationError>;
}
