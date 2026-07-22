use domain::Resource;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgendaItem {
    pub r_ref: String,
    pub title: String,
    pub todo: Option<String>,
    pub scheduled: Option<String>,
    pub deadline: Option<String>,
    pub locator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AgendaView {
    pub items: Vec<AgendaItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ParaOverview {
    pub projects: Vec<Resource>,
    pub areas: Vec<Resource>,
    pub resources: Vec<Resource>,
    pub archives: Vec<Resource>,
}
