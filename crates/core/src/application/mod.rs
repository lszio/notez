pub mod context;
pub mod link_resolution;

pub mod attachment;
pub mod write_check;
pub mod writeback;
pub use write_check::{check_address_uniqueness, check_capability, check_revision};
pub use writeback::{RelaySyncReport, WritebackReport};
pub mod job_manager;
pub use job_manager::{ArtifactStaleReport, JobRecord};
pub mod doctor;
pub use doctor::{DoctorIssue, DoctorReport};
pub mod sync_app;
pub use sync_app::ActiveConflicts;
pub mod community_app;
pub mod federation;
pub mod ports;
pub mod wire;
pub use ports::{BlobStore, Clock, ExtensionRuntime, FilesystemBlobStore, SystemClock};
pub use attachment::ExtractionResult;
pub mod dispatcher;
pub mod projector;
pub use community_app::SpaceCommunitiesConfig;
pub use federation::SourceInstancesCache;
pub mod service;

pub mod card_executor;
pub use card_executor::{
    CardCache, CardExecutionContext, CardExecutionError, CardExecutionService, CardExecutor,
    CardProjection, CardState,
};
#[cfg(not(target_arch = "wasm32"))]
pub mod janet;
#[cfg(not(target_arch = "wasm32"))]
pub use janet::{JanetScriptError, NativeJanetExecutor, DEFAULT_RESULT_LIMIT, DEFAULT_TIMEOUT_MS};
#[cfg(not(target_arch = "wasm32"))]
pub use service::JanetQuerySnapshot;
#[cfg(not(target_arch = "wasm32"))]
pub use service::JanetExecutor;
pub mod use_cases;
pub mod use_cases_impl;
#[cfg(not(target_arch = "wasm32"))]
pub mod watch;
#[cfg(not(target_arch = "wasm32"))]
pub use watch::{SourceObservation, WatchError, WatchEvent, WatchKind, WatchService, WatchStatus};

#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WatchStatus {
    pub root: std::path::PathBuf,
    pub started_at: std::time::SystemTime,
    pub event_count: usize,
}
#[cfg(target_arch = "wasm32")]
#[derive(Debug, thiserror::Error)]
pub enum WatchError {
    #[error("watch not supported on wasm32")]
    WasmUnavailable,
}
#[cfg(target_arch = "wasm32")]
#[derive(Debug, Clone)]
pub struct WatchService;
#[cfg(target_arch = "wasm32")]
impl WatchService {
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self)
    }
    pub fn start(&self, _path: &std::path::Path) -> Result<std::time::SystemTime, WatchError> {
        Err(WatchError::WasmUnavailable)
    }
    pub fn stop(&self, _path: &std::path::Path) -> bool {
        false
    }
    pub fn status(&self, _path: &std::path::Path) -> Option<WatchStatus> {
        None
    }
    pub fn events(&self, _path: &std::path::Path, _limit: usize) -> Vec<()> {
        vec![]
    }
}
pub mod graph;
pub use graph::{Graph, GraphEdge, GraphNode, MAX_NODES, layout_force};
pub mod task_para;
pub use context::SourceContext;
pub use service::{
    ApplicationError, Engine, DocumentErrorKind, ResolveResult, ScanReport,
    StorageErrorKind,
};
pub use use_cases::{
    ArtifactUseCase, AttachmentUseCase, CommunityUseCase, InspectUseCase, LinkUseCase,
    ResourceUseCase, ScanUseCase, SyncUseCase, TaskUseCase,
};
