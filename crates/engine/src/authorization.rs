use std::collections::HashSet;
use std::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Principal(pub String);
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Space(pub String);
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Source(pub String);
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action { Read, Write, Ingest }
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Scope { Space(String), Source(String), Object(String) }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationError { Forbidden { principal: String, action: Action } }

pub trait Authorization: Send + Sync {
    fn check(&self, principal: &Principal, space: &Space, source: &Source, action: &Action, scope: &Scope) -> Result<(), AuthorizationError>;
}

#[derive(Default)]
pub struct AllowList { grants: RwLock<HashSet<(String, String, String, Action, Scope)>> }
impl AllowList {
    pub fn grant(&self, principal: &str, space: &str, source: &str, action: Action, scope: Scope) {
        self.grants.write().unwrap().insert((principal.into(), space.into(), source.into(), action, scope));
    }
}
impl Authorization for AllowList {
    fn check(&self, principal: &Principal, space: &Space, source: &Source, action: &Action, scope: &Scope) -> Result<(), AuthorizationError> {
        if self.grants.read().unwrap().contains(&(principal.0.clone(), space.0.clone(), source.0.clone(), action.clone(), scope.clone())) {
            Ok(())
        } else {
            Err(AuthorizationError::Forbidden { principal: principal.0.clone(), action: action.clone() })
        }
    }
}
