use crate::manifest::Manifest;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SyncError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Sync error: {0}")]
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncObject {
    pub hash: String,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TombstoneRecord {
    pub logical_path: String,
    pub deleted_at: String,
}

pub struct ObjectStore {
    root: PathBuf,
}

impl ObjectStore {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    pub fn manifests_dir(&self) -> PathBuf {
        let dir = self.root.join("manifests");
        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
        }
        dir
    }

    pub fn objects_dir(&self) -> PathBuf {
        let dir = self.root.join("objects");
        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
        }
        dir
    }

    pub fn tombstones_dir(&self) -> PathBuf {
        let dir = self.root.join("tombstones");
        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
        }
        dir
    }

    pub fn write_manifest(&self, manifest: &Manifest) -> Result<(), SyncError> {
        let safe_name = manifest.logical_path.replace('/', "_");
        let path = self.manifests_dir().join(format!("{safe_name}.json"));
        let json = serde_json::to_string_pretty(manifest)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn read_manifest(&self, logical_path: &str) -> Result<Option<Manifest>, SyncError> {
        let safe_name = logical_path.replace('/', "_");
        let path = self.manifests_dir().join(format!("{safe_name}.json"));
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(path)?;
        let manifest: Manifest = serde_json::from_str(&content)?;
        Ok(Some(manifest))
    }

    pub fn write_object(&self, object: &SyncObject) -> Result<(), SyncError> {
        let path = self.objects_dir().join(&object.hash);
        if !path.exists() {
            fs::write(path, &object.payload)?;
        }
        Ok(())
    }

    pub fn has_object(&self, hash: &str) -> bool {
        self.objects_dir().join(hash).exists()
    }

    pub fn read_object(&self, hash: &str) -> Result<Option<SyncObject>, SyncError> {
        let path = self.objects_dir().join(hash);
        if !path.exists() {
            return Ok(None);
        }
        let payload = fs::read(path)?;
        Ok(Some(SyncObject {
            hash: hash.to_string(),
            payload,
        }))
    }

    pub fn write_tombstone(&self, tombstone: &TombstoneRecord) -> Result<(), SyncError> {
        let safe_name = tombstone.logical_path.replace('/', "_");
        let path = self.tombstones_dir().join(format!("{safe_name}.json"));
        let json = serde_json::to_string_pretty(tombstone)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn has_tombstone(&self, logical_path: &str) -> bool {
        let safe_name = logical_path.replace('/', "_");
        self.tombstones_dir()
            .join(format!("{safe_name}.json"))
            .exists()
    }
}
