//! Scan use case: rebuild a space's projection from the filesystem.

use crate::application::{ApplicationError, ScanReport};

/// Traverse the native source bound to this facade and (re)write the
/// projection. Identical semantics to the historical
/// `ApplicationService::scan_native`; the method is moved into a
/// trait so that callers can take a narrow dependency on scanning.
pub trait ScanUseCase {
    fn scan_native(&mut self) -> Result<ScanReport, ApplicationError>;
    fn scan_federation(&mut self) -> Result<ScanReport, ApplicationError>;
}
