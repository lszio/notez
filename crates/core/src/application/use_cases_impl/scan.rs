//! ScanUseCase implementation for `Engine`.
//!
//! Owns the native-source scanning paths and writes observations through
//! the shared projection and journal pipeline.
//! a [`ChangeOp::Scan`] Change plus the link-resolution Changes in the
//! event journal.

use crate::application::link_resolution::LinkResolver;
use crate::application::projector::{Journaling, Projector};
use crate::application::service::{
    ApplicationError, Engine, ScanReport, StorageErrorKind,
};
use crate::application::use_cases::{ResourceUseCase, ScanUseCase};
use crate::domain::change::ChangeOp;
use crate::domain::{ProjectionReader, ProjectionStore, ProjectionWrite};

impl<S> ScanUseCase for Engine<S>
where
    S: ProjectionStore,
    S: ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>,
{
    fn scan_native(&mut self) -> Result<ScanReport, ApplicationError> {
        use crate::source::SourceAdapter;
        use crate::source::native::NativeSourceAdapter;

        let source_root = self.require_space_root()?;
        if self.format_parsers.is_empty() {
            return Err(ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: "no format parsers registered; call Engine::register_format_parser at composition root before scanning".to_string(),
            });
        }

        let config = crate::source::SourceConfig {
            id: "native".to_string(),
            kind: crate::source::SourceKind::Native,
            path: source_root.to_path_buf(),
            read_only: false,
            url: None,
            include_paths: vec![],
            exclude_paths: vec![],
        };
        let adapter = NativeSourceAdapter::new(config);
        // `NativeSourceAdapter` already wires the Org/Markdown parsers it
        // ships with. We still consult any parsers the service registered
        // for additional MIME types. Scan via the adapter, then merge any
        // extra-parser results for non-handled MIMEs (none today, but
        // keeps the seam open).
        let mut scanned = adapter.scan().map_err(|e| ApplicationError::Storage {
            kind: StorageErrorKind::InvalidState,
            message: e.to_string(),
        })?;
        for parser in &self.format_parsers {
            if parser.supports("text/org") || parser.supports("text/markdown") {
                continue;
            }
            // Additional MIME support: not used by the built-in transports.
            // Reserved for future extension.
            let _ = parser;
        }

        let scanned_files = scanned
            .resources
            .iter()
            .map(|r| r.locator.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let scanned_resources = scanned.resources.len();
        let link_occurrences = scanned.link_occurrences.clone();

        // Phase 1: swap the source slice, journaling a Scan Change.
        {
            let journaling = Journaling::from_parts(
                self.journal.as_ref(),
                self.audit.as_ref(),
                self.actor_principal(),
                self.now_unix_millis(),
            );
            let mut projector = Projector::new(&mut self.store, &journaling);
            projector
                .replace_source_op(
                    ChangeOp::Scan,
                    "native",
                    std::mem::take(&mut scanned.resources),
                    std::mem::take(&mut scanned.relations),
                    link_occurrences.clone(),
                    serde_json::Value::Null,
                )
                .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?;
        }

        // Phase 2: resolve links against the fresh rows and journal the
        // resulting relations + diagnostics.
        {
            let (relations, diagnostics) =
                LinkResolver::compute(&self.store, "native", &link_occurrences)?;
            let journaling = Journaling::from_parts(
                self.journal.as_ref(),
                self.audit.as_ref(),
                self.actor_principal(),
                self.now_unix_millis(),
            );
            let mut projector = Projector::new(&mut self.store, &journaling);
            projector
                .replace_resolved_relations("native", relations)
                .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?;
            projector
                .write_link_diagnostics("native", &diagnostics)
                .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?;
        }

        let mut resolved_count = 0;
        let page = <Self as ResourceUseCase>::query(self, &crate::domain::Selector::new())?;
        for res in page.items {
            if res.source_id == "native" {
                let rels = self.store.query_resolved_relations(&res.r#ref).map_err(
                    |e| ApplicationError::Storage {
                        kind: StorageErrorKind::Sqlite,
                        message: e.to_string(),
                    },
                )?;
                resolved_count += rels.len();
            }
        }

        Ok(ScanReport {
            scanned_files,
            scanned_resources,
            scanned_relations: resolved_count,
        })
    }

    fn scan_federation(&mut self) -> Result<ScanReport, ApplicationError> {
        let source_root = self.require_space_root()?;
        use crate::source::SourceAdapter;

        let mut total_resources = 0;

        let sources_cfg = crate::application::federation::SourceInstancesCache::load(&source_root)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;
        let exclude_paths: Vec<std::path::PathBuf> =
            sources_cfg.sources.iter().map(|s| s.path.clone()).collect();

        let native_config = crate::source::SourceConfig {
            id: "native".to_string(),
            kind: crate::source::SourceKind::Native,
            path: source_root.to_path_buf(),
            read_only: false,
            url: None,
            include_paths: vec![],
            exclude_paths,
        };
        let native_adapter = crate::source::NativeSourceAdapter::new(native_config);
        let native_scanned = native_adapter
            .scan()
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;

        total_resources += native_scanned.resources.len();

        self.scan_and_project_source(
            "native",
            native_scanned.resources,
            native_scanned.relations,
            &native_scanned.link_occurrences,
        )?;

        for src_cfg in sources_cfg.sources {
            let adapter = self
                .source_registry
                .build(src_cfg.clone().into_source_config())
                .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::InvalidState,
                    message: e.to_string(),
                })?;
            let scanned = adapter.scan().map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;

            total_resources += scanned.resources.len();

            self.scan_and_project_source(
                &src_cfg.id,
                scanned.resources,
                scanned.relations,
                &scanned.link_occurrences,
            )?;
        }

        let mut resolved_count = 0;
        let page = <Self as ResourceUseCase>::query(self, &crate::domain::Selector::new())?;
        for res in page.items {
            let rels = self
                .store
                .query_resolved_relations(&res.r#ref)
                .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?;
            resolved_count += rels.len();
        }

        Ok(ScanReport {
            scanned_files: total_resources, // Note: not fully accurate, but historically used
            scanned_resources: total_resources,
            scanned_relations: resolved_count,
        })
    }
}

impl<S> Engine<S>
where
    S: ProjectionStore,
    S: ProjectionReader<Error = crate::storage::StorageError>
        + ProjectionWrite<Error = crate::storage::StorageError>,
{
    /// Project one scanned source through the event-spine pipeline:
    /// a [`ChangeOp::Scan`]-tagged source swap followed by the
    /// journalled link-resolution writes.
    fn scan_and_project_source(
        &mut self,
        source_id: &str,
        resources: Vec<crate::domain::Resource>,
        relations: Vec<crate::domain::ResourceRelation>,
        link_occurrences: &[crate::domain::LinkOccurrence],
    ) -> Result<(), ApplicationError> {
        // Phase 1: swap the source slice, journaling a Scan Change.
        {
            let journaling = Journaling::from_parts(
                self.journal.as_ref(),
                self.audit.as_ref(),
                self.actor_principal(),
                self.now_unix_millis(),
            );
            let mut projector = Projector::new(&mut self.store, &journaling);
            projector
                .replace_source_op(
                    ChangeOp::Scan,
                    source_id,
                    resources,
                    relations,
                    link_occurrences.to_vec(),
                    serde_json::Value::Null,
                )
                .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?;
        }

        // Phase 2: resolve links against the fresh rows and journal the
        // resulting relations + diagnostics.
        let (relations, diagnostics) = LinkResolver::compute(&self.store, source_id, link_occurrences)?;
        let journaling = Journaling::from_parts(
            self.journal.as_ref(),
            self.audit.as_ref(),
            self.actor_principal(),
            self.now_unix_millis(),
        );
        let mut projector = Projector::new(&mut self.store, &journaling);
        projector
            .replace_resolved_relations(source_id, relations)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            })?;
        projector
            .write_link_diagnostics(source_id, &diagnostics)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            })?;
        Ok(())
    }
}
