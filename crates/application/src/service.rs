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
    pub fn add_source(
        &self,
        space_root: &Path,
        config: source::SourceConfig,
    ) -> Result<(), ApplicationError> {
        let mut cfg = crate::federation::SpaceSourcesConfig::load(space_root)?;
        cfg.sources.retain(|s| s.id != config.id);
        cfg.sources.push(config);
        cfg.save(space_root)?;
        Ok(())
    }

    pub fn list_sources(
        &self,
        space_root: &Path,
    ) -> Result<Vec<source::SourceConfig>, ApplicationError> {
        let cfg = crate::federation::SpaceSourcesConfig::load(space_root)?;
        Ok(cfg.sources)
    }

    pub fn scan_federation(&mut self, space_root: &Path) -> Result<ScanReport, ApplicationError> {
        use source::SourceAdapter;

        let mut total_resources = 0;
        let mut total_relations = 0;

        let sources_cfg = crate::federation::SpaceSourcesConfig::load(space_root)?;
        let exclude_paths: Vec<std::path::PathBuf> =
            sources_cfg.sources.iter().map(|s| s.path.clone()).collect();

        let native_config = source::SourceConfig {
            id: "native".to_string(),
            kind: source::SourceKind::Native,
            path: space_root.to_path_buf(),
            read_only: false,
            exclude_paths,
        };
        let native_adapter = source::NativeSourceAdapter::new(native_config);
        let native_scanned = native_adapter
            .scan()
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

        total_resources += native_scanned.resources.len();
        total_relations += native_scanned.relations.len();

        self.store
            .replace_source("native", native_scanned.resources, native_scanned.relations)
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        let sources_cfg = crate::federation::SpaceSourcesConfig::load(space_root)?;
        for src_cfg in sources_cfg.sources {
            let scanned = match src_cfg.kind {
                source::SourceKind::Native => {
                    let adapter = source::NativeSourceAdapter::new(src_cfg.clone());
                    adapter.scan()
                }
                source::SourceKind::Git => {
                    let adapter = source::GitSourceAdapter::new(src_cfg.clone());
                    adapter.scan()
                }
                source::SourceKind::Obsidian => {
                    let adapter = source::ObsidianSourceAdapter::new(src_cfg.clone());
                    adapter.scan()
                }
                source::SourceKind::Anytype => {
                    let adapter = source::AnytypeSourceAdapter::new(src_cfg.clone());
                    adapter.scan()
                }
            }
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

            total_resources += scanned.resources.len();
            total_relations += scanned.relations.len();

            self.store
                .replace_source(&src_cfg.id, scanned.resources, scanned.relations)
                .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        }

        Ok(ScanReport {
            scanned_files: total_resources,
            scanned_resources: total_resources,
            scanned_relations: total_relations,
        })
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
    pub fn agenda(&self) -> Result<crate::task_para::AgendaView, ApplicationError> {
        let page = self.query(&Selector::new())?;
        let mut items = Vec::new();

        for res in page.items {
            let scheduled = res.properties.get("SCHEDULED").cloned();
            let deadline = res.properties.get("DEADLINE").cloned();
            let todo = res.properties.get("TODO").cloned();

            if scheduled.is_some() || deadline.is_some() || todo.is_some() {
                items.push(crate::task_para::AgendaItem {
                    r_ref: res.r#ref.to_string(),
                    title: res.title,
                    todo,
                    scheduled,
                    deadline,
                    locator: res.locator,
                });
            }
        }

        Ok(crate::task_para::AgendaView { items })
    }

    pub fn transition_task(
        &mut self,
        r_ref: &ResourceRef,
        to_state: &str,
        timestamp: &str,
    ) -> Result<document::StateTransition, ApplicationError> {
        let mut res = self
            .read(r_ref)?
            .ok_or_else(|| ApplicationError::NotFound(r_ref.to_string()))?;

        let current_todo = res
            .properties
            .get("TODO")
            .cloned()
            .unwrap_or_else(|| "TODO".to_string());

        let profile = document::WorkflowProfile::default();
        let transition = profile
            .transition(&current_todo, to_state, timestamp)
            .map_err(|e| {
                ApplicationError::Document(document::DocumentError::Other(e.to_string()))
            })?;

        res.properties
            .insert("TODO".to_string(), transition.to_state.clone());
        if let Some(ref closed_ts) = transition.closed_timestamp {
            res.properties
                .insert("CLOSED".to_string(), closed_ts.clone());
        }

        let source_id = res.source_id.clone();
        self.store
            .replace_source(&source_id, vec![res], vec![])
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

        Ok(transition)
    }

    pub fn para_overview(&self) -> Result<crate::task_para::ParaOverview, ApplicationError> {
        let page = self.query(&Selector::new())?;
        let mut projects = Vec::new();
        let mut areas = Vec::new();
        let mut resources = Vec::new();
        let mut archives = Vec::new();

        for res in page.items {
            let inspect_res = self.inspect_rules(&res.r#ref)?;
            let para_val = inspect_res
                .as_ref()
                .and_then(|i| i.derived_properties.get("para").map(|s| s.as_str()))
                .or_else(|| res.properties.get("para").map(|s| s.as_str()))
                .or_else(|| res.properties.get("TYPE").map(|s| s.as_str()));

            match para_val {
                Some("projects") | Some("project") => projects.push(res),
                Some("areas") | Some("area") => areas.push(res),
                Some("resources") | Some("resource") => resources.push(res),
                Some("archives") | Some("archive") => archives.push(res),
                _ => {}
            }
        }

        Ok(crate::task_para::ParaOverview {
            projects,
            areas,
            resources,
            archives,
        })
    }
    pub fn add_attachment(
        &mut self,
        space_root: &Path,
        file_path: &Path,
        default_mime: &str,
    ) -> Result<ResourceRef, ApplicationError> {
        let bytes = std::fs::read(file_path)?;
        let blob_store = storage::BlobStore::new(space_root);
        let meta = blob_store
            .store_bytes(&bytes, default_mime)
            .map_err(ApplicationError::Io)?;

        let att_ulid = if meta.hash.len() >= 32 {
            u128::from_str_radix(&meta.hash[..32], 16)
                .map(ulid::Ulid::from)
                .unwrap_or_else(|_| ulid::Ulid::new())
        } else {
            ulid::Ulid::new()
        };
        let att_ref = ResourceRef::new(domain::ResourceKind::Attachment, att_ulid);
        let mut properties = std::collections::BTreeMap::new();
        properties.insert("hash".to_string(), meta.hash.clone());
        properties.insert("mime".to_string(), meta.mime_type.clone());
        properties.insert("size_bytes".to_string(), meta.size_bytes.to_string());

        let title = file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let resource = Resource {
            r#ref: att_ref,
            kind: domain::ResourceKind::Attachment,
            title,
            revision: meta.hash,
            source_id: "native".to_string(),
            locator: file_path.to_string_lossy().to_string(),
            properties,
        };

        let page = self.query(&Selector::new())?;
        let mut native_resources: Vec<Resource> = page
            .items
            .into_iter()
            .filter(|r| r.source_id == "native" && r.r#ref != att_ref)
            .collect();
        native_resources.push(resource);

        self.store
            .replace_source("native", native_resources, vec![])
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

        Ok(att_ref)
    }

    pub fn run_extraction(
        &mut self,
        space_root: &Path,
        att_ref: &ResourceRef,
    ) -> Result<Vec<domain::SegmentRecord>, ApplicationError> {
        use artifact::{Extractor, ImageMetadataExtractor, SegmentSlicer, TextExtractor};

        let res = self
            .read(att_ref)?
            .ok_or_else(|| ApplicationError::NotFound(att_ref.to_string()))?;

        let hash = res
            .properties
            .get("hash")
            .ok_or_else(|| ApplicationError::Storage("missing hash property".to_string()))?;
        let mime = res
            .properties
            .get("mime")
            .cloned()
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let blob_store = storage::BlobStore::new(space_root);
        let bytes = blob_store
            .get(hash)?
            .ok_or_else(|| ApplicationError::NotFound(format!("blob hash {hash}")))?;

        let extracted_content = if mime.starts_with("image/") {
            let ext = ImageMetadataExtractor;
            ext.extract(&bytes, &mime).map_err(|e| {
                ApplicationError::Document(document::DocumentError::Other(e.to_string()))
            })?
        } else {
            let ext = TextExtractor;
            ext.extract(&bytes, &mime).map_err(|e| {
                ApplicationError::Document(document::DocumentError::Other(e.to_string()))
            })?
        };

        let slicer = SegmentSlicer::default();
        let records = slicer.slice(&att_ref.to_string(), &extracted_content.text);

        self.store
            .insert_segments(&records)
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

        Ok(records)
    }

    pub fn query_segments(
        &self,
        att_ref: &ResourceRef,
    ) -> Result<Vec<domain::SegmentRecord>, ApplicationError> {
        self.store
            .query_segments(&att_ref.to_string())
            .map_err(|e| ApplicationError::Storage(e.to_string()))
    }
    pub fn create_community(
        &self,
        space_root: &Path,
        community: domain::community::Community,
    ) -> Result<(), ApplicationError> {
        let mut cfg = crate::community_app::SpaceCommunitiesConfig::load(space_root)?;
        cfg.communities.retain(|c| c.id != community.id);
        cfg.communities.push(community);
        cfg.save(space_root)?;
        Ok(())
    }

    pub fn list_communities(
        &self,
        space_root: &Path,
    ) -> Result<Vec<domain::community::Community>, ApplicationError> {
        let cfg = crate::community_app::SpaceCommunitiesConfig::load(space_root)?;
        Ok(cfg.communities)
    }

    pub fn derive_artifact(
        &self,
        space_root: &Path,
        community_id: &str,
        recipe_name: &str,
    ) -> Result<artifact::DerivedArtifact, ApplicationError> {
        let communities = self.list_communities(space_root)?;
        let comm = communities
            .iter()
            .find(|c| c.id == community_id)
            .ok_or_else(|| ApplicationError::NotFound(format!("community {community_id}")))?;

        let page = self.query(&Selector::new())?;
        let members: Vec<Resource> = comm
            .filter_members(&page.items)
            .into_iter()
            .cloned()
            .collect();

        let recipe_kind = match recipe_name {
            "summary" => artifact::RecipeKind::Summary,
            "llms-txt" | "llms.txt" => artifact::RecipeKind::LlmsTxt,
            "context-pack" => artifact::RecipeKind::ContextPack,
            "skill-ir" => artifact::RecipeKind::SkillIr,
            _ => {
                return Err(ApplicationError::Document(document::DocumentError::Other(
                    format!("unknown recipe: {recipe_name}"),
                )));
            }
        };

        let recipe = artifact::Recipe {
            name: recipe_name.to_string(),
            kind: recipe_kind,
            token_budget: 4000,
        };

        let derived = artifact::RecipeEvaluator::evaluate(&recipe, &members).map_err(|e| {
            ApplicationError::Document(document::DocumentError::Other(e.to_string()))
        })?;

        Ok(derived)
    }

    pub fn export_skill(
        &self,
        space_root: &Path,
        community_id: &str,
        description: &str,
        export_path: &Path,
    ) -> Result<artifact::SkillPackage, ApplicationError> {
        let communities = self.list_communities(space_root)?;
        let comm = communities
            .iter()
            .find(|c| c.id == community_id)
            .ok_or_else(|| ApplicationError::NotFound(format!("community {community_id}")))?;

        let page = self.query(&Selector::new())?;
        let members: Vec<Resource> = comm
            .filter_members(&page.items)
            .into_iter()
            .cloned()
            .collect();

        let skill_ir = artifact::SkillIr::compile(&comm.name, description, &members);

        let package = artifact::SkillExporter::export(&skill_ir, export_path)
            .map_err(ApplicationError::Io)?;

        Ok(package)
    }
    pub fn sync_push(
        &mut self,
        actor_id: &str,
        space_root: &Path,
        shared_folder: &Path,
    ) -> Result<sync::PushReport, ApplicationError> {
        let transport = sync::FolderTransport::new(shared_folder);
        let engine = sync::SyncEngine::new(actor_id, space_root, transport);
        let report = engine
            .push()
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        Ok(report)
    }

    pub fn sync_pull(
        &mut self,
        actor_id: &str,
        space_root: &Path,
        shared_folder: &Path,
    ) -> Result<sync::PullReport, ApplicationError> {
        let transport = sync::FolderTransport::new(shared_folder);
        let engine = sync::SyncEngine::new(actor_id, space_root, transport);
        let report = engine
            .pull()
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;

        self.scan_native(space_root)?;

        Ok(report)
    }

    pub fn list_conflicts(&self) -> Result<Vec<sync::ConflictRecord>, ApplicationError> {
        Ok(Vec::new())
    }
    pub fn space_doctor(
        &self,
        space_root: &Path,
    ) -> Result<crate::doctor::DoctorReport, ApplicationError> {
        let mut issues = Vec::new();

        let db_path = space_root.join(".notez/index.sqlite");
        if !db_path.exists() {
            issues.push(crate::doctor::DoctorIssue {
                severity: "warning".to_string(),
                code: "MISSING_INDEX".to_string(),
                message: "SQLite index database does not exist. Run 'notez space rebuild'."
                    .to_string(),
            });
        }

        let page = self.query(&Selector::new())?;
        for res in page.items {
            let loc_path = Path::new(&res.locator);
            if !loc_path.exists() {
                issues.push(crate::doctor::DoctorIssue {
                    severity: "error".to_string(),
                    code: "MISSING_FILE".to_string(),
                    message: format!(
                        "Resource {} points to non-existent file: {}",
                        res.r#ref, res.locator
                    ),
                });
            }
        }

        let status = if issues.iter().any(|i| i.severity == "error") {
            "unhealthy".to_string()
        } else if !issues.is_empty() {
            "degraded".to_string()
        } else {
            "healthy".to_string()
        };

        Ok(crate::doctor::DoctorReport { status, issues })
    }
    pub fn list_jobs(&self) -> Result<Vec<crate::job_manager::JobRecord>, ApplicationError> {
        Ok(Vec::new())
    }

    pub fn check_artifact_freshness(
        &self,
        _space_root: &Path,
    ) -> Result<crate::job_manager::ArtifactStaleReport, ApplicationError> {
        Ok(crate::job_manager::ArtifactStaleReport {
            status: "fresh".to_string(),
            stale_artifacts: Vec::new(),
        })
    }

    pub fn rebuild(&mut self, root: &Path) -> Result<ScanReport, ApplicationError> {
        self.store
            .clear()
            .map_err(|e| ApplicationError::Storage(e.to_string()))?;
        self.scan_native(root)
    }
}
