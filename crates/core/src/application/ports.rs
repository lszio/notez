//! Infrastructure ports.
//!
//! Ports hide infrastructure effects behind traits so the application
//! layer stays free of filesystem, clock, and blob-store implementation
//! details. Every port is a required trait method — no silent defaults
//! — and concrete adapters live in `crate::storage` or `crate::infra`.

use std::path::PathBuf;
use crate::storage::BlobMeta;
use std::time::SystemTime;

/// Wall-clock abstraction. Default implementation is `SystemClock`,
/// which wraps `std::time::SystemTime::now()`.
pub trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;
}

/// Native extension runtime boundary. Script adapters must not receive
/// storage handles or source writers; they operate on serialized snapshots.
pub trait ExtensionRuntime: Send + Sync {
    fn execute(&self, script: &str, input: &serde_json::Value) -> Result<serde_json::Value, String>;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// Content-addressed blob store. Implementations must hash the payload,
/// persist it under `<source_root>/.notez/blobs/<aa>/<bb>/<hash>`, and
/// return the metadata the application needs to record on the
/// resource.
pub trait BlobStore: Send + Sync {
    /// Persist `bytes` and return the content hash, byte size, and
    /// effective MIME type. Calling with the same `bytes` MUST be
    /// idempotent: an existing blob is not re-written.
    fn store_bytes(&self, bytes: &[u8], default_mime: &str) -> Result<BlobMeta, std::io::Error>;

    /// Resolve a previously stored blob by content hash. Returns
    /// `Ok(None)` when the blob is absent.
    fn load(&self, hash: &str) -> Result<Option<Vec<u8>>, std::io::Error>;

    /// File path the blob would be served from (used by the web raw
    /// endpoint for content-addressed attachment streaming).
    fn path_for(&self, hash: &str) -> Option<PathBuf>;
}


/// Filesystem-backed blob store rooted at `<source_root>/.notez/blobs`.
/// This is the only adapter the application facade currently ships; new
/// stores (e.g. S3, in-memory test store) plug in by implementing the
/// [`BlobStore`] trait.
pub struct FilesystemBlobStore {
    root: PathBuf,
}

impl FilesystemBlobStore {
    pub fn new(source_root: &std::path::Path) -> Self {
        Self {
            root: source_root.join(".notez/blobs"),
        }
    }
}

impl BlobStore for FilesystemBlobStore {
    fn store_bytes(
        &self,
        bytes: &[u8],
        default_mime: &str,
    ) -> Result<BlobMeta, std::io::Error> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let hash = format!("{:x}", hasher.finalize());

        let (prefix1, prefix2) = (&hash[0..2], &hash[2..4]);
        let dir = self.root.join(prefix1).join(prefix2);
        std::fs::create_dir_all(&dir)?;
        let blob_path = dir.join(&hash);
        if !blob_path.exists() {
            std::fs::write(&blob_path, bytes)?;
        }

        let mime_type = if default_mime.is_empty()
            || default_mime == "application/octet-stream"
        {
            detect_mime(bytes)
        } else {
            default_mime.to_string()
        };

        Ok(BlobMeta {
            hash,
            size_bytes: bytes.len() as u64,
            mime_type,
        })
    }

    fn load(&self, hash: &str) -> Result<Option<Vec<u8>>, std::io::Error> {
        if hash.len() < 4 {
            return Ok(None);
        }
        let (prefix1, prefix2) = (&hash[0..2], &hash[2..4]);
        let blob_path = self.root.join(prefix1).join(prefix2).join(hash);
        if !blob_path.exists() {
            return Ok(None);
        }
        Ok(Some(std::fs::read(blob_path)?))
    }

    fn path_for(&self, hash: &str) -> Option<PathBuf> {
        if hash.len() < 4 {
            return None;
        }
        let (prefix1, prefix2) = (&hash[0..2], &hash[2..4]);
        let blob_path = self.root.join(prefix1).join(prefix2).join(hash);
        if blob_path.exists() {
            Some(blob_path)
        } else {
            None
        }
    }
}

fn detect_mime(bytes: &[u8]) -> String {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png".to_string()
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "image/jpeg".to_string()
    } else if bytes.starts_with(b"%PDF-") {
        "application/pdf".to_string()
    } else if std::str::from_utf8(bytes).is_ok() {
        "text/plain".to_string()
    } else {
        "application/octet-stream".to_string()
    }
}