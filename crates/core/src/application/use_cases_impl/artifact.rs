//! ArtifactUseCase implementation for `Engine`.
//!
//! Method bodies were previously inlined in `service.rs`; this file
//! is part of the 0.5.x-A1+A3 use-case impl split.

use crate::application::service::{
    ApplicationError, Engine, DocumentErrorKind, StorageErrorKind,
};
use crate::application::use_cases::{ArtifactUseCase, ResourceUseCase};
use crate::domain::{ProjectionReader, ProjectionWrite};
use crate::application::write_check;
use crate::artifact::{DerivedArtifact, SkillPackage};
use crate::domain::{ProjectionStore, Resource, Selector};
use std::path::Path;

impl<S> ArtifactUseCase for Engine<S>
where
    S: ProjectionStore,
    S: ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>, {
    fn derive_artifact(
        &self,
        community_id: &str,
        recipe_name: &str,
    ) -> Result<crate::artifact::DerivedArtifact, ApplicationError> {
        let _source_root = self.require_space_root()?;
        write_check::check_capability(self, "artifact")?;

        let communities =
            <Self as crate::application::use_cases::CommunityUseCase>::list_communities(self)?;
        let comm = communities
            .iter()
            .find(|c| c.id == community_id)
            .ok_or_else(|| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: format!("community {community_id} not found"),
            })?;

        let page = <Self as crate::application::use_cases::ResourceUseCase>::query(
            self,
            &Selector::new(),
        )?;
        let members: Vec<Resource> = comm
            .filter_members(&page.items)
            .into_iter()
            .cloned()
            .collect();

        let recipe_kind = match recipe_name {
            "summary" => crate::artifact::RecipeKind::Summary,
            "llms-txt" | "llms.txt" => crate::artifact::RecipeKind::LlmsTxt,
            "context-pack" => crate::artifact::RecipeKind::ContextPack,
            "skill-ir" => crate::artifact::RecipeKind::SkillIr,
            _ => {
                return Err(ApplicationError::Document {
                    source: DocumentErrorKind::Org(crate::document::OrgDocumentError::Other(
                        format!("unknown recipe: {recipe_name}"),
                    )),
                });
            }
        };

        let recipe = crate::artifact::Recipe {
            name: recipe_name.to_string(),
            kind: recipe_kind,
            token_budget: 4000,
        };

        let derived =
            crate::artifact::RecipeEvaluator::evaluate(&recipe, &members).map_err(|e| {
                ApplicationError::Document {
                    source: DocumentErrorKind::Org(crate::document::OrgDocumentError::Other(
                        e.to_string(),
                    )),
                }
            })?;

        Ok(derived)
    }

    fn export_skill(
        &self,
        community_id: &str,
        description: &str,
        export_path: &Path,
    ) -> Result<crate::artifact::SkillPackage, ApplicationError> {
        let _source_root = self.require_space_root()?;
        write_check::check_capability(self, "artifact")?;

        let communities =
            <Self as crate::application::use_cases::CommunityUseCase>::list_communities(self)?;
        let comm = communities
            .iter()
            .find(|c| c.id == community_id)
            .ok_or_else(|| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: format!("community {community_id} not found"),
            })?;

        let page = <Self as crate::application::use_cases::ResourceUseCase>::query(
            self,
            &Selector::new(),
        )?;
        let members: Vec<Resource> = comm
            .filter_members(&page.items)
            .into_iter()
            .cloned()
            .collect();

        let skill_ir = crate::artifact::SkillIr::compile(&comm.name, description, &members);

        let package =
            crate::artifact::SkillExporter::export(&skill_ir, export_path).map_err(|e| {
                ApplicationError::Io {
                    path: Some(export_path.to_path_buf()),
                    source: e.kind(),
                }
            })?;

        Ok(package)
    }
}
