//! Attachment use case: blob storage and segment extraction.

use std::path::Path;

use crate::application::ApplicationError;
use crate::domain::{ResourceRef, SegmentRecord};

pub trait AttachmentUseCase {
    fn add_attachment(
        &mut self,
        space_root: &Path,
        file_path: &Path,
        default_mime: &str,
    ) -> Result<ResourceRef, ApplicationError>;
    fn run_extraction(
        &mut self,
        space_root: &Path,
        att_ref: &ResourceRef,
    ) -> Result<Vec<SegmentRecord>, ApplicationError>;
    fn query_segments(
        &self,
        att_ref: &ResourceRef,
    ) -> Result<Vec<SegmentRecord>, ApplicationError>;
}