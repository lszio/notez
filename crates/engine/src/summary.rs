use crate::PortBundle;
pub struct Summary;
impl Summary { pub fn revision(ports: &PortBundle, source_id: &str) -> Option<String> { ports.projection.as_ref().and_then(|p| p.snapshot(source_id).ok()).map(|s| s.revision) } }
