//! `ResourceRow` — the DTO returned by web server functions.
//!
//! It mirrors `core::domain::Resource` but uses concrete `String` fields
//! (no ULID) so it serialises cleanly through `dioxus::server` without
//! requiring the client to import `ulid`. The `From<Resource>` impl
//! converts the domain type to the wire shape; rendering code in
//! `pages/list.rs` and `pages/detail.rs` consumes `ResourceRow` directly.

use std::collections::BTreeMap;

use notez_core::domain::{Resource, ResourceKind, ResourceRef};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResourceRow {
    pub ref_str: String,
    pub kind: String,
    pub title: String,
    pub source_id: String,
    pub locator: String,
    pub object_id: String,
    pub revision: String,
    pub properties: BTreeMap<String, String>,
    /// Rendered preview HTML for the resource body. Empty for
    /// kinds that don't have inline content (attachments).
    pub body_html: String,
    /// Raw source text for the edit textarea. Populated only in the
    /// document detail path; empty for list rows and non-documents.
    #[serde(default)]
    pub raw_content: String,
}

/// Slim DTO returned by `list_resources_via_backend` — the
/// surface-agnostic `ui::ResourceRow` shape, no `body_html`/`raw_content`
/// so the same wire form is observable from web SSR, desktop, and
/// mobile without per-surface translation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BackendResourceRow {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub locator: String,
}

impl From<ui::ResourceRow> for BackendResourceRow {
    fn from(r: ui::ResourceRow) -> Self {
        Self {
            id: r.id,
            kind: r.kind,
            title: r.title,
            locator: r.locator,
        }
    }
}

impl ResourceRow {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        r_ref: ResourceRef,
        kind: ResourceKind,
        title: String,
        source_id: String,
        locator: String,
        object_id: String,
        revision: String,
        properties: BTreeMap<String, String>,
        body_html: String,
    ) -> Self {
        Self {
            ref_str: r_ref.to_string(),
            kind: kind.as_str().to_string(),
            title,
            source_id,
            locator,
            object_id,
            revision,
            properties,
            body_html,
            raw_content: String::new(),
        }
    }
}
impl From<Resource> for ResourceRow {
    fn from(r: Resource) -> Self {
        Self::new(
            r.r#ref,
            r.kind,
            r.title,
            r.source_id,
            r.locator,
            r.object_id.to_string(),
            r.revision,
            r.properties,
            String::new(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notez_core::domain::{derived_object_id, ResourceKind, ResourceRef};

    #[test]
    fn from_resource_populates_all_fields() {
        let r_ref = ResourceRef::parse("heading:01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_resource_row_mirrors_ui_resource_row() {
        let row = ui::ResourceRow {
            id: "heading:01J0".into(),
            kind: "Heading".into(),
            title: "Design sync".into(),
            locator: "notes/design.org::Design sync".into(),
        };
        let dto = BackendResourceRow::from(row.clone());
        assert_eq!(dto.id, row.id);
        assert_eq!(dto.kind, row.kind);
        assert_eq!(dto.title, row.title);
        assert_eq!(dto.locator, row.locator);
    }
}
        let object_id = derived_object_id("hash", "loc", "h:0");
        let res = Resource {
            r#ref: r_ref,
            kind: ResourceKind::Heading,
            title: "X".to_string(),
            revision: "r1".to_string(),
            source_id: "src".to_string(),
            locator: "loc".to_string(),
            properties: BTreeMap::from([("k".to_string(), "v".to_string())]),
            object_id: object_id.clone(),
            primary_source_id: String::new(),
        };
        let row = ResourceRow::from(res);
        assert_eq!(row.ref_str, "heading:01ARZ3NDEKTSV4RRFFQ69G5FAV");
        assert_eq!(row.kind, "heading");
        assert_eq!(row.title, "X");
        assert_eq!(row.source_id, "src");
        assert_eq!(row.locator, "loc");
        assert_eq!(row.object_id, object_id.to_string());
        assert_eq!(row.revision, "r1");
        assert_eq!(row.properties.get("k").map(String::as_str), Some("v"));
        assert_eq!(row.body_html, "");
    }
}
