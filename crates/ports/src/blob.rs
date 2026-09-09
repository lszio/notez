//! Content-addressed blob boundary.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobMeta {
    pub hash: String,
    pub size_bytes: u64,
    pub mime_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobError {
    Unavailable { message: String },
    Invalid { message: String },
}

pub trait BlobStore: Send + Sync {
    fn put(&self, bytes: &[u8], mime_type: &str) -> Result<BlobMeta, BlobError>;
    fn get(&self, hash: &str) -> Result<Option<Vec<u8>>, BlobError>;
}
