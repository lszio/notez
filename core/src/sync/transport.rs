use crate::sync::object::ObjectStore;
use std::path::Path;

pub struct FolderTransport {
    pub store: ObjectStore,
}

impl FolderTransport {
    pub fn new(shared_path: &Path) -> Self {
        Self {
            store: ObjectStore::new(shared_path),
        }
    }
}
