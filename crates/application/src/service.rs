use document::{DocumentError, OrgScanner};
use domain::{ProjectionStore, QueryPage, Resource, ResourceRef, Selector};
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Document error: {0}")]
    Document(#[from] DocumentError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Resource not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveResult {
    Found(ResourceRef),
    NotFound,
    Ambiguous(Vec<ResourceRef>),
}

#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub scanned_files: usize,
    pub scanned_resources: usize,
    pub scanned_relations: usize,
}

pub struct ApplicationService<S: ProjectionStore> {
    store: S,
    rule_engine: domain::RuleEngine,
}

impl<S: ProjectionStore> ApplicationService<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            rule_engine: domain::RuleEngine::default_rules(),
        }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }
    pub fn inspect_rules(
        &self,
        r_ref: &ResourceRef,
    ) -> Result<Option<domain::InspectResult>, ApplicationError> {
        let res = self
            .store
            .get(r_ref)
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        Ok(res.map(|r| self.rule_engine.evaluate(&r)))
    }

    pub fn scan_native(&mut self, root: &Path) -> Result<ScanReport, ApplicationError> {
        let mut entries: Vec<PathBuf> = Vec::new();
        for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
            let path = entry.path();
            let rel_path = path.strip_prefix(root).unwrap_or(path);
            if rel_path
                .components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
            {
                continue;
            }
            if path.is_file() && path.extension().is_some_and(|ext| ext == "org") {
                entries.push(path.to_path_buf());
            }
        }

        entries.sort();

        let mut all_resources = Vec::new();
        let mut all_relations = Vec::new();
        let scanned_files = entries.len();

        for path in entries {
            let scanned = OrgScanner::scan(&path, "native")?;
            all_resources.extend(scanned.resources);
            all_relations.extend(scanned.links);
        }

        let scanned_resources = all_resources.len();
        let scanned_relations = all_relations.len();

        self.store
            .replace_source("native", all_resources, all_relations)
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

        Ok(ScanReport {
            scanned_files,
            scanned_resources,
            scanned_relations,
        })
    }

    pub fn resolve(&self, query_str: &str) -> Result<ResolveResult, ApplicationError> {
        let trimmed = query_str.trim();

        if let Ok(r_ref) = ResourceRef::parse(trimmed)
            && let Some(res) = self
                .store
                .get(&r_ref)
                .map_err(|e| ApplicationError::Storage(e.to_string()))?
        {
            return Ok(ResolveResult::Found(res.r#ref));
        }

        if trimmed.len() == 26 {
            let mut matched = Vec::new();
            if let Ok(heading_ref) = ResourceRef::parse(&format!("heading:{trimmed}"))
                && let Some(res) = self
                    .store
                    .get(&heading_ref)
                    .map_err(|e| ApplicationError::Storage(e.to_string()))?
            {
                matched.push(res.r#ref);
            }
            if let Ok(doc_ref) = ResourceRef::parse(&format!("document:{trimmed}"))
                && let Some(res) = self
                    .store
                    .get(&doc_ref)
                    .map_err(|e| ApplicationError::Storage(e.to_string()))?
            {
                matched.push(res.r#ref);
            }
            if matched.len() == 1 {
                return Ok(ResolveResult::Found(matched[0]));
            } else if matched.len() > 1 {
                return Ok(ResolveResult::Ambiguous(matched));
            }
        }

        let page_all = self.query(&Selector::new())?;
        let locator_matches: Vec<ResourceRef> = page_all
            .items
            .iter()
            .filter(|r| r.locator == trimmed)
            .map(|r| r.r#ref)
            .collect();
        if locator_matches.len() == 1 {
            return Ok(ResolveResult::Found(locator_matches[0]));
        } else if locator_matches.len() > 1 {
            return Ok(ResolveResult::Ambiguous(locator_matches));
        }

        let title_selector = Selector::new().with_title_contains(trimmed);
        let page_title = self.query(&title_selector)?;
        let title_matches: Vec<ResourceRef> = page_title.items.iter().map(|r| r.r#ref).collect();
        if title_matches.len() == 1 {
            return Ok(ResolveResult::Found(title_matches[0]));
        } else if title_matches.len() > 1 {
            return Ok(ResolveResult::Ambiguous(title_matches));
        }

        Ok(ResolveResult::NotFound)
    }

    pub fn query(&self, selector: &Selector) -> Result<QueryPage, ApplicationError> {
        self.store
            .query(selector)
            .map_err(|e| ApplicationError::Storage(e.to_string()))
    }

    pub fn read(&self, r_ref: &ResourceRef) -> Result<Option<Resource>, ApplicationError> {
        self.store
            .get(r_ref)
            .map_err(|e| ApplicationError::Storage(e.to_string()))
    }

    pub fn rebuild(&mut self, root: &Path) -> Result<ScanReport, ApplicationError> {
        self.store
            .clear()
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        self.scan_native(root)
    }
}
