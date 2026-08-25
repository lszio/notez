//! ScanUseCase impl for `ApplicationFacade`.
//!
//! Owns the two native-source scanning paths. Method bodies were
//! previously inlined in `service.rs`; moving them here is the first
//! step of the `0.5.x-A1+A3` use-case impl split (spec
//! `docs/superpowers/specs/2026-08-09-0.5x-a1-a3-usecase-impl-split-and-write-checks-design.org`).

use crate::application::service::{
    ApplicationError, ApplicationFacade, ScanReport, StorageErrorKind,
};
use crate::application::use_cases::ScanUseCase;
use crate::domain::ProjectionStore;

impl<S: ProjectionStore> ScanUseCase for ApplicationFacade<S> {
    fn scan_native(&mut self) -> Result<ScanReport, ApplicationError> {
        use crate::application::link_resolution::resolve_and_store_links;
        use crate::source::SourceAdapter;
        use crate::source::native::NativeSourceAdapter;

        let source_root = self.require_space_root()?;
        if self.format_parsers.is_empty() {
            return Err(ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: "no format parsers registered; call ApplicationService::register_format_parser at composition root before scanning".to_string(),
            });
        }

        let config = crate::source::SourceConfig {
            id: "native".to_string(),
            kind: crate::source::SourceKind::Native,
            path: source_root.to_path_buf(),
            read_only: false,
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
        self.store
            .replace_source(
                "native",
                std::mem::take(&mut scanned.resources),
                std::mem::take(&mut scanned.relations),
                link_occurrences.clone(),
            )
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            })?;

        resolve_and_store_links(&mut self.store, "native", link_occurrences)?;

        let mut resolved_count = 0;
        let page = <Self as crate::application::use_cases::ResourceUseCase>::query(
            self,
            &crate::domain::Selector::new(),
        )?;
        for res in page.items {
            if res.source_id == "native" {
                let rels = self
                    .store
                    .query_resolved_relations(&res.r#ref)
                    .unwrap_or_default();
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

        self.store
            .replace_source(
                "native",
                native_scanned.resources,
                native_scanned.relations,
                native_scanned.link_occurrences.clone(),
            )
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::Sqlite,
                message: e.to_string(),
            })?;

        crate::application::link_resolution::resolve_and_store_links(
            &mut self.store,
            "native",
            native_scanned.link_occurrences,
        )?;

        let sources_cfg = crate::application::federation::SourceInstancesCache::load(&source_root)
            .map_err(|e| ApplicationError::Storage {
                kind: StorageErrorKind::InvalidState,
                message: e.to_string(),
            })?;
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

            self.store
                .replace_source(
                    &src_cfg.id,
                    scanned.resources,
                    scanned.relations,
                    scanned.link_occurrences.clone(),
                )
                .map_err(|e| ApplicationError::Storage {
                    kind: StorageErrorKind::Sqlite,
                    message: e.to_string(),
                })?;

            crate::application::link_resolution::resolve_and_store_links(
                &mut self.store,
                &src_cfg.id,
                scanned.link_occurrences,
            )?;
        }

        let mut resolved_count = 0;
        let page = <Self as crate::application::use_cases::ResourceUseCase>::query(
            self,
            &crate::domain::Selector::new(),
        )?;
        for res in page.items {
            let rels = self
                .store
                .query_resolved_relations(&res.r#ref)
                .unwrap_or_default();
            resolved_count += rels.len();
        }

        Ok(ScanReport {
            scanned_files: total_resources, // Note: not fully accurate, but historically used
            scanned_resources: total_resources,
            scanned_relations: resolved_count,
        })
    }
}
