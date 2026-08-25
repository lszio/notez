use crate::domain::Resource;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RecipeError {
    #[error("Evaluation failed: {0}")]
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecipeKind {
    Summary,
    LlmsTxt,
    ContextPack,
    SkillIr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recipe {
    pub name: String,
    pub kind: RecipeKind,
    pub token_budget: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedArtifact {
    pub recipe_name: String,
    pub kind: RecipeKind,
    pub content: String,
}

pub struct RecipeEvaluator;

impl RecipeEvaluator {
    pub fn evaluate(
        recipe: &Recipe,
        resources: &[Resource],
    ) -> Result<DerivedArtifact, RecipeError> {
        let content = match recipe.kind {
            RecipeKind::Summary => crate::artifact::generators::generate_summary(resources),
            RecipeKind::LlmsTxt => crate::artifact::generators::generate_llms_txt(resources),
            RecipeKind::ContextPack => {
                crate::artifact::generators::generate_context_pack(resources)
            }
            RecipeKind::SkillIr => crate::artifact::generators::generate_skill_ir_json(resources),
        };

        Ok(DerivedArtifact {
            recipe_name: recipe.name.clone(),
            kind: recipe.kind,
            content,
        })
    }
}
