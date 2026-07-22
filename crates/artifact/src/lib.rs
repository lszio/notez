pub mod extractor;
pub mod generators;
pub mod recipe;
pub use recipe::{DerivedArtifact, Recipe, RecipeError, RecipeEvaluator, RecipeKind};

pub use domain::SegmentRecord;
pub use extractor::{
    ExtractedContent, ExtractionError, Extractor, ImageMetadataExtractor, SegmentSlicer,
    TextExtractor,
};
