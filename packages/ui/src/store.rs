use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppStore {
    pub active_space: Option<String>,
    pub route: String,
    pub selection: Option<String>,
    pub query: String,
    pub document_cache: HashMap<String, String>,
    pub drafts: HashMap<String, String>,
    pub pending_commands: usize,
    pub notifications: Vec<String>,
    pub error: Option<String>,
}

impl Default for AppStore {
    fn default() -> Self {
        Self {
            active_space: None,
            route: "/inbox".into(),
            selection: None,
            query: String::new(),
            document_cache: HashMap::new(),
            drafts: HashMap::new(),
            pending_commands: 0,
            notifications: Vec::new(),
            error: None,
        }
    }
}

impl AppStore {
    pub fn set_route(&mut self, route: impl Into<String>) { self.route = route.into(); }
    pub fn set_query(&mut self, query: impl Into<String>) { self.query = query.into(); }
    pub fn cache_document(&mut self, key: impl Into<String>, content: impl Into<String>) {
        self.document_cache.insert(key.into(), content.into());
    }
    pub fn stage_draft(&mut self, key: impl Into<String>, content: impl Into<String>) {
        self.drafts.insert(key.into(), content.into());
    }
}
