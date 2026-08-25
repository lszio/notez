use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobMeta {
    pub hash: String,
    pub size_bytes: u64,
    pub mime_type: String,
}

pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    pub fn new(source_root: &Path) -> Self {
        Self {
            root: source_root.join(".notez/blobs"),
        }
    }

    pub fn store_bytes(&self, bytes: &[u8], default_mime: &str) -> Result<BlobMeta, io::Error> {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let hash = format!("{:x}", hasher.finalize());

        let (prefix1, prefix2) = (&hash[0..2], &hash[2..4]);
        let dir = self.root.join(prefix1).join(prefix2);
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }

        let blob_path = dir.join(&hash);
        if !blob_path.exists() {
            fs::write(&blob_path, bytes)?;
        }

        let mime_type = if default_mime.is_empty() || default_mime == "application/octet-stream" {
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

    pub fn has(&self, hash: &str) -> bool {
        if hash.len() < 4 {
            return false;
        }
        let (prefix1, prefix2) = (&hash[0..2], &hash[2..4]);
        let blob_path = self.root.join(prefix1).join(prefix2).join(hash);
        blob_path.exists()
    }

    pub fn get(&self, hash: &str) -> Result<Option<Vec<u8>>, io::Error> {
        if !self.has(hash) {
            return Ok(None);
        }
        let (prefix1, prefix2) = (&hash[0..2], &hash[2..4]);
        let blob_path = self.root.join(prefix1).join(prefix2).join(hash);
        let bytes = fs::read(blob_path)?;
        Ok(Some(bytes))
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
