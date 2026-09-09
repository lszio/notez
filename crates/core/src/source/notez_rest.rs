use crate::domain::Resource;
use crate::source::adapter::{PreparedWrite, ScannedSource, SourceAdapter, SourceCapabilities, SourceConfig, SourceError, WriteResult};
use crate::source::protocol::{RawEntity, SourceTransport, TransportError};
use std::io::{Read, Write};
use std::net::TcpStream;

/// Minimal read-only client for a Notez HTTP source. It never follows source
/// references, so an upstream cannot recursively execute other adapters.
pub struct NotezRestSourceAdapter {
    config: SourceConfig,
}

impl NotezRestSourceAdapter {
    pub fn new(mut config: SourceConfig) -> Self {
        config.read_only = true;
        Self { config }
    }
    fn url(&self) -> Result<&str, SourceError> {
        if self.config.url.as_deref().unwrap_or("").is_empty() { return Err(SourceError::Other("remote Notez source URL is unavailable".into())); }
        Ok(self.config.url.as_deref().unwrap())
    }

    fn get_json(&self, path: &str) -> Result<serde_json::Value, SourceError> {
        let base = self.url()?;
        let (host, port, prefix) = parse_base_url(base)?;
        let target = format!("{}{}", prefix.trim_end_matches('/'), path);
        let mut stream = TcpStream::connect((host.as_str(), port))
            .map_err(|e| SourceError::Other(format!("remote Notez source unavailable: {e}")))?;
        let request = format!("GET {target} HTTP/1.1\r\nHost: {host}\r\nAccept: application/json\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).map_err(SourceError::Io)?;
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).map_err(SourceError::Io)?;
        let text = String::from_utf8_lossy(&bytes);
        let (head, body) = text.split_once("\r\n\r\n").ok_or_else(|| SourceError::Other("remote Notez source returned malformed HTTP".into()))?;
        let status = head.lines().next().and_then(|l| l.split_whitespace().nth(1)).and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
        if status == 401 || status == 403 { return Err(SourceError::Other(format!("remote Notez source unavailable: HTTP {status}"))); }
        if status == 404 { return Err(SourceError::Other("remote Notez source operation unsupported (HTTP 404)".into())); }
        if !(200..300).contains(&status) { return Err(SourceError::Other(format!("remote Notez source unavailable: HTTP {status}"))); }
        serde_json::from_str(body).map_err(|e| SourceError::Other(format!("remote Notez source returned invalid JSON: {e}")))
    }

    pub fn discover_capabilities(&self) -> Result<SourceCapabilities, SourceError> {
        let value = self.get_json("/api/capabilities")?;
        serde_json::from_value(value.get("capabilities").cloned().unwrap_or(value))
            .map_err(|e| SourceError::Other(format!("remote Notez capabilities invalid: {e}")))
    }
    fn resources(&self, value: serde_json::Value) -> Result<Vec<Resource>, SourceError> {
        let value = value.get("items").cloned().or_else(|| value.get("resources").cloned()).unwrap_or(value);
        let arr = value.as_array().ok_or_else(|| SourceError::Other("remote Notez source returned unsupported resource payload".into()))?;
        let mut out = Vec::with_capacity(arr.len());
        for item in arr {
            let mut resource: Resource = serde_json::from_value(item.clone()).map_err(|e| SourceError::Other(format!("remote Notez source returned invalid resource: {e}")))?;
            resource.source_id = self.config.id.clone();
            out.push(resource);
        }
        Ok(out)
    }
}

impl SourceAdapter for NotezRestSourceAdapter {
    fn config(&self) -> &SourceConfig { &self.config }
    fn scan(&self) -> Result<ScannedSource, SourceError> {
        Ok(ScannedSource { source_id: self.config.id.clone(), resources: self.list_resources()?, relations: vec![], link_occurrences: vec![], ignored: Default::default() })
    }
    fn capabilities(&self) -> SourceCapabilities {
        self.discover_capabilities().unwrap_or(SourceCapabilities { can_read: false, can_write: false, can_import: false, can_watch: false })
    }
    fn list_resources(&self) -> Result<Vec<Resource>, SourceError> { self.resources(self.get_json("/api/resources")?) }
    fn read_resource(&self, locator: &str) -> Result<Option<Resource>, SourceError> {
        let path = format!("/api/resources/{}", percent_encode(locator));
        let value = self.get_json(&path)?;
        if value.is_null() { return Ok(None); }
        let mut resource: Resource = serde_json::from_value(value.get("resource").cloned().unwrap_or(value)).map_err(|e| SourceError::Other(format!("remote Notez source returned invalid resource: {e}")))?;
        resource.source_id = self.config.id.clone();
        Ok(Some(resource))
    }
    fn search_resources(&self, query: &str, limit: usize) -> Result<Vec<Resource>, SourceError> {
        let path = format!("/api/search?q={}&limit={limit}", percent_encode(query));
        self.resources(self.get_json(&path)?)
    }
    fn prepare_write(&self, _target_ref: &str, _payload: &str) -> Result<PreparedWrite, SourceError> { Err(SourceError::Other("remote Notez source is read-only; write unsupported".into())) }
    fn commit_write(&self, _prep: &PreparedWrite) -> Result<WriteResult, SourceError> { Err(SourceError::Other("remote Notez source is read-only; write unsupported".into())) }
}

impl SourceTransport for NotezRestSourceAdapter {
    fn fetch_raw(&self) -> Result<Vec<RawEntity>, TransportError> { self.list_resources().map(|rs| rs.into_iter().map(|r| { let locator = r.locator.clone(); RawEntity { locator, mime_type: "application/json".into(), payload: serde_json::to_vec(&r).unwrap_or_default() } }).collect()).map_err(|e| TransportError::Other(e.to_string())) }
    fn mutate(&self, _locator: &str, _payload: &str) -> Result<(), TransportError> {
        Err(TransportError::Other("remote Notez transport is read-only".into()))
    }
}

fn parse_base_url(url: &str) -> Result<(String, u16, String), SourceError> {
    let rest = url.strip_prefix("http://").ok_or_else(|| SourceError::Other("remote Notez source only supports http:// URLs".into()))?;
    let (authority, prefix) = rest.split_once('/').map_or((rest, ""), |(a,p)| (a,p));
    let (host, port) = authority.rsplit_once(':').map_or((authority, 80), |(h,p)| (h, p.parse().unwrap_or(80)));
    if host.is_empty() { return Err(SourceError::Other("remote Notez source URL is invalid".into())); }
    Ok((host.to_string(), port, format!("/{prefix}")))
}
fn percent_encode(s: &str) -> String { s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect() }
#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceKind;
    #[test] fn url_config_is_read_only() { let mut c = SourceConfig::new("up", SourceKind::NotezRest, "/", false); c.url = Some("http://localhost:1".into()); let a = NotezRestSourceAdapter::new(c); assert!(a.config().read_only); assert!(!a.capabilities().can_write); }
    #[test] fn unavailable_is_explicit() { let mut c = SourceConfig::new("up", SourceKind::NotezRest, "/", true); c.url = Some("http://127.0.0.1:1".into()); let a = NotezRestSourceAdapter::new(c); assert!(a.list_resources().unwrap_err().to_string().contains("unavailable")); }
}
