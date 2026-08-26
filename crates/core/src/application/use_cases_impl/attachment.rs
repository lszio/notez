//! AttachmentUseCase impl for `ApplicationFacade`.
//!
//! Method bodies were previously inlined in `service.rs`; this file
//! is part of the 0.5.x-A1+A3 use-case impl split.

use crate::application::service::{
    ApplicationError, ApplicationFacade, DocumentErrorKind, StorageErrorKind,
};
use crate::application::use_cases::AttachmentUseCase;
use crate::domain::{ProjectionReader, ProjectionWrite};
use crate::application::use_cases::ResourceUseCase;
use crate::application::write_check;
use crate::domain::{
    ProjectionStore, Resource, ResourceKind, ResourceRef, SegmentRecord, Selector,
};
use std::path::Path;

impl<S> AttachmentUseCase for ApplicationFacade<S>
where
    S: ProjectionStore,
    S: ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>, {
    fn add_attachment(
        &mut self,
        file_path: &Path,
        default_mime: &str,
    ) -> Result<ResourceRef, ApplicationError> {
        let source_root = self.require_space_root()?;
        write_check::check_capability(self, "attachment")?;

        let bytes = std::fs::read(file_path).map_err(|e| ApplicationError::Io {
            path: Some(file_path.to_path_buf()),
            source: e.kind(),
        })?;
        let blob_store = crate::application::FilesystemBlobStore::new(&source_root);
        let meta = <crate::application::FilesystemBlobStore as crate::application::BlobStore>::store_bytes(&blob_store, &bytes, default_mime)
            .map_err(|e| ApplicationError::Io {
                path: Some(file_path.to_path_buf()),
                source: e.kind(),
            })?;

        let att_ulid = if meta.hash.len() >= 32 {
            u128::from_str_radix(&meta.hash[..32], 16)
                .map(ulid::Ulid::from)
                .unwrap_or_else(|_| ulid::Ulid::new())
        } else {
            ulid::Ulid::new()
        };
        let att_ref = ResourceRef::new(crate::domain::ResourceKind::Attachment, att_ulid);
        let mut properties = std::collections::BTreeMap::new();
        properties.insert("hash".to_string(), meta.hash.clone());
        properties.insert("mime".to_string(), meta.mime_type.clone());
        properties.insert("size_bytes".to_string(), meta.size_bytes.to_string());

        let title = file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Locator must be a POSIX-relative path so the tree builder
        // can reconstruct the directory hierarchy and so the raw
        // attachment endpoint serves the file under the space root.
        // When `file_path` is not under `source_root` (rare: user
        // dragged an external file in), fall back to the absolute
        // path so we never silently lose the file.
        let locator = file_path
            .strip_prefix(source_root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| file_path.to_string_lossy().to_string());

        let resource = Resource {
            r#ref: att_ref,
            kind: crate::domain::ResourceKind::Attachment,
            title,
            revision: meta.hash.clone(),
            source_id: "native".to_string(),
            locator,
            properties,
            object_id: crate::domain::derived_object_id("", "", ""),
            primary_source_id: String::new(),
        };

        let page = <Self as crate::application::use_cases::ResourceUseCase>::query(
            self,
            &Selector::new(),
        )?;
        let mut native_resources: Vec<Resource> = page
            .items
            .into_iter()
            .filter(|r| r.source_id == "native" && r.r#ref != att_ref)
            .collect();
        native_resources.push(resource);

        self.store
            .replace_source("native", native_resources, vec![], vec![])
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            })?;

        Ok(att_ref)
    }

    fn run_extraction(
        &mut self,
        att_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::SegmentRecord>, ApplicationError> {
        let source_root = self.require_space_root()?;

        use crate::artifact::{Extractor, ImageMetadataExtractor, SegmentSlicer, TextExtractor};
        let res = <Self as crate::application::use_cases::ResourceUseCase>::read(self, att_ref)?
            .ok_or_else(|| ApplicationError::NotFound {
                kind: att_ref.kind(),
                r_ref: att_ref.clone(),
            })?;

        let hash = res
            .properties
            .get("hash")
            .ok_or_else(|| ApplicationError::Storage {
                kind: StorageErrorKind::BlobMissing,
                message: "missing hash property".to_string(),
            })?;
        let mime = res
            .properties
            .get("mime")
            .cloned()
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let blob_store = crate::application::FilesystemBlobStore::new(&source_root);
        let bytes = <crate::application::FilesystemBlobStore as crate::application::BlobStore>::load(&blob_store, hash)
            .map_err(|e| ApplicationError::Io {
                path: None,
                source: e.kind(),
            })?
            .ok_or_else(|| ApplicationError::Storage {
                kind: StorageErrorKind::BlobMissing,
                message: format!("blob hash {hash}"),
            })?;

        let extracted_content = if mime.starts_with("image/") {
            let ext = ImageMetadataExtractor;
            ext.extract(&bytes, &mime)
                .map_err(|e| ApplicationError::Document {
                    source: DocumentErrorKind::Org(crate::document::OrgDocumentError::Other(
                        e.to_string(),
                    )),
                })?
        } else {
            let ext = TextExtractor;
            ext.extract(&bytes, &mime)
                .map_err(|e| ApplicationError::Document {
                    source: DocumentErrorKind::Org(crate::document::OrgDocumentError::Other(
                        e.to_string(),
                    )),
                })?
        };

        let slicer = SegmentSlicer::default();
        let records = slicer.slice(&att_ref.to_string(), &extracted_content.text);

        self.store
            .insert_segments(&records)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            })?;

        Ok(records)
    }

    fn query_segments(
        &self,
        att_ref: &ResourceRef,
    ) -> Result<Vec<crate::domain::SegmentRecord>, ApplicationError> {
        self.store
            .query_segments(&att_ref.to_string())
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            })
    }
}
