pub mod extractor;

pub use domain::SegmentRecord;
pub use extractor::{
    ExtractedContent, ExtractionError, Extractor, ImageMetadataExtractor, SegmentSlicer,
    TextExtractor,
};
