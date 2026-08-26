use crate::sync::manifest::Manifest;
use crate::sync::merge::{ConflictRecord, HeadsTracker, MergeResult, ThreeWayMerger};
use crate::sync::object::{SyncError, SyncObject};
use crate::sync::transport::FolderTransport;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PushReport {
    pub pushed_files: usize,
    pub pushed_manifests: usize,
    pub pushed_objects: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PullReport {
    pub pulled_files: usize,
    pub merged_files: usize,
    pub conflicts: Vec<ConflictRecord>,
}


/// Per-device memory of "the last hash I knew for this path" — the
/// honest causal parent for the next push. Stored under
/// `<space>/.notez/sync-state.json`.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct SyncState {
    pub known: std::collections::BTreeMap<String, String>,
}

impl SyncState {
    fn load(device_space: &Path) -> Self {
        let p = device_space.join(".notez/sync-state.json");
        fs::read_to_string(p)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn save(&self, device_space: &Path) -> Result<(), SyncError> {
        let dir = device_space.join(".notez");
        fs::create_dir_all(&dir)?;
        let p = dir.join("sync-state.json");
        fs::write(p, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

pub struct SyncEngine {
    actor_id: String,
    device_space: PathBuf,
    transport: FolderTransport,
}

impl SyncEngine {
    pub fn new(actor_id: &str, device_space: &Path, transport: FolderTransport) -> Self {
        Self {
            actor_id: actor_id.to_string(),
            device_space: device_space.to_path_buf(),
            transport,
        }
    }

    pub fn push(&self) -> Result<PushReport, SyncError> {
        let mut sync_state = SyncState::load(&self.device_space);
        let mut pushed_files = 0;
        let mut pushed_manifests = 0;
        let mut pushed_objects = 0;

        for entry in WalkDir::new(&self.device_space)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            let rel_path = path.strip_prefix(&self.device_space).unwrap_or(path);
            if rel_path
                .components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
            {
                continue;
            }
            if path.is_file() {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or_default();
                if ext == "org" || ext == "md" {
                    let payload = fs::read(path)?;
                    let hash = format!("{:x}", Sha256::digest(&payload));

                    let logical_path = rel_path.to_string_lossy().to_string();

                    let obj = SyncObject {
                        hash: hash.clone(),
                        payload,
                    };
                    self.transport.store.write_object(&obj)?;
                    pushed_objects += 1;

                    // M5: the causal parent is the hash THIS device
                    // last knew for this path (sync-state), because
                    // that is the state this push evolved from. Fall
                    // back to the shared folder's previous manifest.
                    let mut parents: Vec<String> = Vec::new();
                    if let Some(prev) = sync_state.known.get(&logical_path) {
                        if prev != &hash && !parents.contains(prev) {
                            parents.push(prev.clone());
                        }
                    }
                    if parents.is_empty() {
                        if let Some(prev) = self.transport.store.read_manifest(&logical_path)? {
                            if prev.content_hash != hash && !parents.contains(&prev.content_hash) {
                                parents.push(prev.content_hash);
                            }
                        }
                    }
                    sync_state.known.insert(logical_path.clone(), hash.clone());

                    let manifest = Manifest {
                        source_id: "default_source".to_string(),
                        actor_id: self.actor_id.clone(),
                        parent_snapshots: parents,
                        logical_path,
                        content_hash: hash,
                        properties: Default::default(),
                    };
                    self.transport.store.write_manifest(&manifest)?;
                    pushed_manifests += 1;
                    pushed_files += 1;
                }
            }
        }

        let heads = HeadsTracker::new(self.transport.store.manifests_dir().parent().unwrap());
        heads.set_head(&self.actor_id, &format!("snap_{pushed_files}"))?;
        sync_state.save(&self.device_space)?;

        Ok(PushReport {
            pushed_files,
            pushed_manifests,
            pushed_objects,
        })
    }
    pub fn pull(&self) -> Result<PullReport, SyncError> {
        let mut sync_state = SyncState::load(&self.device_space);
        let mut pulled_files = 0;
        let mut merged_files = 0;
        let mut conflicts = Vec::new();

        let manifests_dir = self.transport.store.manifests_dir();
        if !manifests_dir.exists() {
            return Ok(PullReport::default());
        }

        for entry in WalkDir::new(&manifests_dir)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|e| e == "json") {
                let content = fs::read_to_string(path)?;
                if let Ok(manifest) = serde_json::from_str::<Manifest>(&content) {
                    let local_file = self.device_space.join(&manifest.logical_path);

                    if let Some(obj) = self.transport.store.read_object(&manifest.content_hash)? {
                        let incoming_text = String::from_utf8_lossy(&obj.payload).to_string();

                        if !local_file.exists() {
                            if let Some(parent) = local_file.parent() {
                                fs::create_dir_all(parent)?;
                            }
                            fs::write(&local_file, &obj.payload)?;
                            pulled_files += 1;
                            sync_state.known.insert(manifest.logical_path.clone(), manifest.content_hash.clone());
                        } else {
                            let local_text = fs::read_to_string(&local_file)?;
                            if local_text != incoming_text {
                                // M5: find a real common ancestor — the
                                // first incoming parent whose object we
                                // also have locally. Empty base only as
                                // a last resort.
                                let mut base_text = String::new();
                                for cand in &manifest.parent_snapshots {
                                    if let Some(base_obj) =
                                        self.transport.store.read_object(cand)?
                                    {
                                        let cand_text =
                                            String::from_utf8_lossy(&base_obj.payload);
                                        if cand_text != local_text
                                            || cand_text != incoming_text
                                        {
                                            base_text = cand_text.to_string();
                                            break;
                                        }
                                    }
                                }
                                match ThreeWayMerger::merge(
                                    &manifest.logical_path,
                                    &base_text,
                                    &local_text,
                                    &incoming_text,
                                ) {
                                    MergeResult::Clean(merged) => {
                                        fs::write(&local_file, &merged)?;
                                        let merged_hash =
                                            format!("{:x}", Sha256::digest(merged.as_bytes()));
                                        self.transport.store.write_object(&SyncObject {
                                            hash: merged_hash.clone(),
                                            payload: merged.into_bytes(),
                                        })?;
                                        sync_state
                                            .known
                                            .insert(manifest.logical_path.clone(), merged_hash);
                                        merged_files += 1;
                                    }
                                    MergeResult::Conflict { conflict } => {
                                        fs::write(&local_file, &conflict.conflict_text)?;
                                        conflicts.push(conflict);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        sync_state.save(&self.device_space)?;

        Ok(PullReport {
            pulled_files,
            merged_files,
            conflicts,
        })
    }
}
