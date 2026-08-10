#[cfg(not(target_arch = "wasm32"))]
pub mod sqlite;
#[cfg(not(target_arch = "wasm32"))]
pub use sqlite::{SqliteProjection, StorageError};

#[cfg(target_arch = "wasm32")]
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("sqlite storage is not available on wasm32 client")]
    WasmUnavailable,
}

#[cfg(target_arch = "wasm32")]
pub struct SqliteProjection;

pub use crate::domain::SegmentRecord;
pub mod blob;
pub use blob::{BlobMeta, BlobStore};

pub use crate::domain::ProjectionStore;
