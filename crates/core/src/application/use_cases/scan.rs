//! Scan use case: rebuild a space's projection from the filesystem.

use crate::application::{ApplicationError, ScanReport};

/// Traverse the native source bound to this facade and (re)write the
/// Projection scan use case exposed by `Engine`.
/// Native scanning records source observations through the journal.
pub trait ScanUseCase {
    fn scan_native(&mut self) -> Result<ScanReport, ApplicationError>;
    fn scan_federation(&mut self) -> Result<ScanReport, ApplicationError>;
}
