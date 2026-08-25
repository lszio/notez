use crate::document::security::{SecurityError, SecurityGuard};
use std::path::Path;

#[test]
fn security_guard_prevents_path_traversal_and_oversized_files() {
    let source_root = Path::new("/space");

    let valid_path = Path::new("/space/notes/doc.org");
    assert!(SecurityGuard::sanitize_path(source_root, valid_path).is_ok());

    let invalid_path = Path::new("/space/../../etc/passwd");
    assert!(SecurityGuard::sanitize_path(source_root, invalid_path).is_err());

    assert!(SecurityGuard::validate_attachment_size(50 * 1024 * 1024).is_ok());
    assert!(matches!(
        SecurityGuard::validate_attachment_size(200 * 1024 * 1024),
        Err(SecurityError::AttachmentTooLarge { .. })
    ));
}
