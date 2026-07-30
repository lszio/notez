//! `Send + Sync` adapter around `ApplicationService<SqliteProjection>`.
//!
//! `rusqlite::Connection` is `Send` but not `Sync`. axum requires
//! `State<T>: Send + Sync + Clone`, so we cannot hand a raw
//! `ApplicationService<SqliteProjection>` to the router. We instead wrap
//! the store in `Arc<std::sync::Mutex<SqliteProjection>>` and forward
//! read-only calls synchronously through the guard.
//!
//! The web server is intentionally a viewer (no mutations). Synchronous
//! locking is fine for the expected workload.

use std::sync::{Arc, Mutex, MutexGuard};

use application::ApplicationService;
use domain::{LinkOccurrence, QueryPage, Resource, ResourceRef, SegmentRecord};
use storage::{SqliteProjection, StorageError};
#[derive(Debug, thiserror::Error)]
pub enum WebSendError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("storage error: {0}")]
    Storage(String),
    #[error("service error: {0}")]
    Service(String),
    #[error("poisoned mutex: {0}")]
    Poisoned(String),
}

impl From<StorageError> for WebSendError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::Sqlite(s) => WebSendError::Sqlite(s),
            StorageError::Serialization(s) => WebSendError::Serde(s),
            StorageError::InvalidData(s) => WebSendError::Storage(s),
        }
    }
}

impl<T> From<std::sync::PoisonError<T>> for WebSendError {
    fn from(e: std::sync::PoisonError<T>) -> Self {
        WebSendError::Poisoned(e.to_string())
    }
}

/// Cheap-to-clone handle to the application service. Internally keeps an
/// `Arc` over a `std::sync::Mutex` around the (non-`Sync`) `SqliteProjection`.
#[derive(Clone)]
pub struct SendService {
    inner: Arc<Mutex<SqliteProjection>>,
}

impl SendService {
    pub fn new(store: SqliteProjection) -> Self {
        Self {
            inner: Arc::new(Mutex::new(store)),
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, SqliteProjection>, WebSendError> {
        Ok(self.inner.lock()?)
    }

    pub fn read(&self, r_ref: &ResourceRef) -> Result<Option<Resource>, WebSendError> {
        let store = self.lock()?;
        <SqliteProjection as domain::ProjectionStore>::get(&store, r_ref)
            .map_err(WebSendError::from)
    }

    pub fn query(&self, selector: &domain::Selector) -> Result<QueryPage, WebSendError> {
        let store = self.lock()?;
        <SqliteProjection as domain::ProjectionStore>::query(&store, selector)
            .map_err(WebSendError::from)
    }

    pub fn agenda(&self) -> Result<application::AgendaView, WebSendError> {
        let store = self.lock()?;
        let service = ApplicationService::new(SqliteProjectionRef(&store));
        service
            .agenda()
            .map_err(|e| WebSendError::Service(e.to_string()))
    }

    pub fn list_recent(&self, limit: usize) -> Result<Vec<Resource>, WebSendError> {
        let store = self.lock()?;
        let service = ApplicationService::new(SqliteProjectionRef(&store));
        service
            .list_recent(limit)
            .map_err(|e| WebSendError::Service(e.to_string()))
    }

    pub fn query_segments(
        &self,
        att_ref: &ResourceRef,
    ) -> Result<Vec<SegmentRecord>, WebSendError> {
        let store = self.lock()?;
        let s = att_ref.to_string();
        <SqliteProjection as domain::ProjectionStore>::query_segments(&store, &s)
            .map_err(WebSendError::from)
    }

    pub fn query_link_occurrences(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkOccurrence>, WebSendError> {
        let store = self.lock()?;
        <SqliteProjection as domain::ProjectionStore>::query_link_occurrences(
            &store,
            source_ref,
        )
        .map_err(WebSendError::from)
    }
}

/// Borrowed adapter that exposes a `&SqliteProjection` through the
/// `ProjectionStore` trait. Used internally by `SendService` to build a
/// fresh `ApplicationService` from a locked store reference.
pub struct SqliteProjectionRef<'a>(pub &'a SqliteProjection);

impl<'a> domain::ProjectionStore for SqliteProjectionRef<'a> {
    type Error = StorageError;

    fn replace_source(
        &mut self,
        _source_id: &str,
        _resources: Vec<Resource>,
        _relations: Vec<domain::ResourceRelation>,
        _link_occurrences: Vec<LinkOccurrence>,
    ) -> Result<(), Self::Error> {
        // Read-only — web handlers don't mutate.
        Ok(())
    }

    fn get(&self, r#ref: &ResourceRef) -> Result<Option<Resource>, Self::Error> {
        <SqliteProjection as domain::ProjectionStore>::get(self.0, r#ref)
    }

    fn query(&self, selector: &domain::Selector) -> Result<QueryPage, Self::Error> {
        <SqliteProjection as domain::ProjectionStore>::query(self.0, selector)
    }

    fn upsert_resource(&mut self, _resource: &Resource) -> Result<(), Self::Error> {
        Ok(())
    }

    fn delete_resource(&mut self, _r_ref: &ResourceRef) -> Result<(), Self::Error> {
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn insert_segments(&mut self, _segments: &[SegmentRecord]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn query_segments(
        &self,
        attachment_ref: &str,
    ) -> Result<Vec<SegmentRecord>, Self::Error> {
        <SqliteProjection as domain::ProjectionStore>::query_segments(self.0, attachment_ref)
    }

    fn replace_link_occurrences(
        &mut self,
        _source_id: &str,
        _occurrences: Vec<LinkOccurrence>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn query_link_occurrences(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<LinkOccurrence>, Self::Error> {
        <SqliteProjection as domain::ProjectionStore>::query_link_occurrences(self.0, source_ref)
    }

    fn replace_resolved_relations(
        &mut self,
        _source_id: &str,
        _relations: Vec<domain::ResolvedRelation>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn query_resolved_relations(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Vec<domain::ResolvedRelation>, Self::Error> {
        <SqliteProjection as domain::ProjectionStore>::query_resolved_relations(self.0, source_ref)
    }

    fn write_link_diagnostics(
        &mut self,
        _source_id: &str,
        _diagnostics: &[(LinkOccurrence, domain::ResolutionStatus, Vec<ResourceRef>)],
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn list_link_diagnostics(
        &self,
        source_ref: &ResourceRef,
    ) -> Result<Option<Vec<domain::LinkDiagnostic>>, Self::Error> {
        <SqliteProjection as domain::ProjectionStore>::list_link_diagnostics(self.0, source_ref)
    }
}