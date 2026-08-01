//! Scan use case: rebuild a space's projection from the filesystem.

use std::path::Path;

use crate::application::{ApplicationError, ScanReport};

/// Traverse the native source under `space_root` and (re)write the
/// projection. Identical semantics to the historical
/// `ApplicationService::scan_native`; the method is moved into a
/// trait so that callers can take a narrow dependency on scanning.
pub trait ScanUseCase {
    fn scan_native(&mut self, root: &Path) -> Result<ScanReport, ApplicationError>;
    fn scan_federation(&mut self, space_root: &Path) -> Result<ScanReport, ApplicationError>;
}
