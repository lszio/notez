pub mod extractor;
pub mod generators;
pub mod recipe;
pub mod skill;
pub use recipe::{DerivedArtifact, Recipe, RecipeError, RecipeEvaluator, RecipeKind};
pub use skill::{SkillExporter, SkillIr, SkillPackage};

pub use crate::domain::SegmentRecord;
pub use extractor::{
    ExtractedContent, ExtractionError, Extractor, ImageMetadataExtractor, PdfExtractor,
    PptxExtractor, SegmentSlicer, TextExtractor, XlsxExtractor, ZipExtractor,
};
