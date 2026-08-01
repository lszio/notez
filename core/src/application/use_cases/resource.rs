//! Resource use case: single-resource CRUD and lookup helpers.

use crate::application::{ApplicationError, ResolveResult};
use crate::domain::{
    QueryPage, Resource, ResourceRef, Selector,
};

pub trait ResourceUseCase {
    fn upsert_resource(&mut self, resource: Resource) -> Result<(), ApplicationError>;
    fn delete_resource(&mut self, r_ref: &ResourceRef) -> Result<(), ApplicationError>;
    fn query(&self, selector: &Selector) -> Result<QueryPage, ApplicationError>;
    fn read(&self, r_ref: &ResourceRef) -> Result<Option<Resource>, ApplicationError>;
    fn list_recent(&self, limit: usize) -> Result<Vec<Resource>, ApplicationError>;
    fn list_by_source(
        &self,
        source_id: &str,
        limit: usize,
    ) -> Result<Vec<Resource>, ApplicationError>;
    fn resolve(&self, query_str: &str) -> Result<ResolveResult, ApplicationError>;
    fn resolve_address(
        &self,
        address: &crate::domain::ResourceAddress,
    ) -> Result<ResolveResult, ApplicationError>;
}
