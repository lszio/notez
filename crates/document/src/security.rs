use std::path::{Component, Path, PathBuf};
use thiserror::Error;

pub const MAX_ATTACHMENT_SIZE_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SecurityError {
    #[error("Path traversal attempt detected: {path}")]
    PathTraversal { path: String },
    #[error("Attachment size {size_bytes} exceeds limit of {max_bytes} bytes")]
    AttachmentTooLarge { size_bytes: u64, max_bytes: u64 },
}

pub struct SecurityGuard;

impl SecurityGuard {
    pub fn sanitize_path(_space_root: &Path, input_path: &Path) -> Result<PathBuf, SecurityError> {
        let mut normalized = PathBuf::new();
        for comp in input_path.components() {
            match comp {
                Component::ParentDir => {
                    if !normalized.pop() {
                        return Err(SecurityError::PathTraversal {
                            path: input_path.to_string_lossy().to_string(),
                        });
                    }
                }
                Component::CurDir => {}
                comp => normalized.push(comp),
            }
        }

        if input_path.to_string_lossy().contains("..") {
            return Err(SecurityError::PathTraversal {
                path: input_path.to_string_lossy().to_string(),
            });
        }

        Ok(normalized)
    }

    pub fn validate_attachment_size(size_bytes: u64) -> Result<(), SecurityError> {
        if size_bytes > MAX_ATTACHMENT_SIZE_BYTES {
            return Err(SecurityError::AttachmentTooLarge {
                size_bytes,
                max_bytes: MAX_ATTACHMENT_SIZE_BYTES,
            });
        }
        Ok(())
    }
}
