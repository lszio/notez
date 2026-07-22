pub mod extractor;
pub mod generators;
pub mod recipe;
pub mod skill;
pub use skill::{SkillExporter, SkillIr, SkillPackage};
pub use recipe::{DerivedArtifact, Recipe, RecipeError, RecipeEvaluator, RecipeKind};

pub use domain::SegmentRecord;
pub use extractor::{
    ExtractedContent, ExtractionError, Extractor, ImageMetadataExtractor, SegmentSlicer,
    TextExtractor,
};
