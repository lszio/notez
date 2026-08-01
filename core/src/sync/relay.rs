use crate::sync::manifest::Manifest;
use crate::sync::object::ObjectStore;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent {
    ManifestPushed(Manifest),
    ObjectPushed(String),
    ManifestPulled(Manifest),
    ObjectPulled(String),
}

pub trait SyncTransport {
    fn push_manifest(&mut self, manifest: &Manifest) -> Result<(), io::Error>;
    fn pull_manifest(&self, logical_path: &str) -> Result<Option<Manifest>, io::Error>;
    fn push_object(&mut self, hash: &str, bytes: &[u8]) -> Result<(), io::Error>;
    fn pull_object(&self, hash: &str) -> Result<Option<Vec<u8>>, io::Error>;
    fn drain_events(&mut self) -> Vec<SyncEvent>;
}

pub struct RelayTransport {
    store: ObjectStore,
    events: Arc<Mutex<Vec<SyncEvent>>>,
}

impl RelayTransport {
    pub fn new(relay_root: &Path) -> Self {
        Self {
            store: ObjectStore::new(relay_root),
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn events_handle(&self) -> Arc<Mutex<Vec<SyncEvent>>> {
        self.events.clone()
    }
}

impl SyncTransport for RelayTransport {
    fn push_manifest(&mut self, manifest: &Manifest) -> Result<(), io::Error> {
        self.store
            .write_manifest(manifest)
            .map_err(|e| io::Error::other(e.to_string()))?;
        if let Ok(mut events) = self.events.lock() {
            events.push(SyncEvent::ManifestPushed(manifest.clone()));
        }
        Ok(())
    }

    fn pull_manifest(&self, logical_path: &str) -> Result<Option<Manifest>, io::Error> {
        let result = self
            .store
            .read_manifest(logical_path)
            .map_err(|e| io::Error::other(e.to_string()))?;
        if let Some(m) = &result
            && let Ok(mut events) = self.events.lock()
        {
            events.push(SyncEvent::ManifestPulled(m.clone()));
        }
        Ok(result)
    }

    fn push_object(&mut self, hash: &str, bytes: &[u8]) -> Result<(), io::Error> {
        let obj = crate::sync::object::SyncObject {
            hash: hash.to_string(),
            payload: bytes.to_vec(),
        };
        self.store
            .write_object(&obj)
            .map_err(|e| io::Error::other(e.to_string()))?;
        if let Ok(mut events) = self.events.lock() {
            events.push(SyncEvent::ObjectPushed(hash.to_string()));
        }
        Ok(())
    }

    fn pull_object(&self, hash: &str) -> Result<Option<Vec<u8>>, io::Error> {
        let result = self
            .store
            .read_object(hash)
            .map_err(|e| io::Error::other(e.to_string()))?;
        if result.is_some()
            && let Ok(mut events) = self.events.lock()
        {
            events.push(SyncEvent::ObjectPulled(hash.to_string()));
        }
        Ok(result.map(|o| o.payload))
    }

    fn drain_events(&mut self) -> Vec<SyncEvent> {
        if let Ok(mut events) = self.events.lock() {
            let drained: Vec<SyncEvent> = events.drain(..).collect();
            drained
        } else {
            Vec::new()
        }
    }
}

pub struct P2pTransport {
    peers: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
}

impl P2pTransport {
    pub fn new() -> Self {
        Self {
            peers: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

impl Default for P2pTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncTransport for P2pTransport {
    fn push_manifest(&mut self, _manifest: &Manifest) -> Result<(), io::Error> {
        Ok(())
    }

    fn pull_manifest(&self, _logical_path: &str) -> Result<Option<Manifest>, io::Error> {
        Ok(None)
    }

    fn push_object(&mut self, hash: &str, bytes: &[u8]) -> Result<(), io::Error> {
        if let Ok(mut peers) = self.peers.lock() {
            peers.insert(hash.to_string(), bytes.to_vec());
        }
        Ok(())
    }

    fn pull_object(&self, hash: &str) -> Result<Option<Vec<u8>>, io::Error> {
        Ok(self.peers.lock().ok().and_then(|p| p.get(hash).cloned()))
    }

    fn drain_events(&mut self) -> Vec<SyncEvent> {
        Vec::new()
    }
}

pub struct TransportRegistry {
    pub root: PathBuf,
    pub transports: BTreeMap<String, Arc<Mutex<dyn SyncTransport>>>,
}

impl TransportRegistry {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            transports: BTreeMap::new(),
        }
    }

    pub fn register<T: SyncTransport + 'static>(&mut self, name: &str, transport: T) {
        self.transports
            .insert(name.to_string(), Arc::new(Mutex::new(transport)));
    }
}
