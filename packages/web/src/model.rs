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
        }
    }
    pub fn to_domain_resource(&self) -> notez_core::domain::Resource {
        let kind = match self.kind.as_str() {
            "document" => ResourceKind::Document,
            "heading" => ResourceKind::Heading,
            "block" => ResourceKind::Block,
            "attachment" => ResourceKind::Attachment,
            _ => ResourceKind::Document,
        };
        notez_core::domain::Resource {
            r#ref: ResourceRef::parse(&self.ref_str)
                .unwrap_or_else(|_| ResourceRef::new(kind, ulid::Ulid::new())),
            kind,
            title: self.title.clone(),
            revision: self.revision.clone(),
            source_id: self.source_id.clone(),
            locator: self.locator.clone(),
            properties: self.properties.clone(),
            object_id: notez_core::domain::ObjectId::new(ulid::Ulid::new()),
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
        let object_id = derived_object_id("hash", "loc", "h:0");
        let res = Resource {
            r#ref: r_ref,
            kind: ResourceKind::Heading,
            title: "X".to_string(),
            revision: "r1".to_string(),
            source_id: "src".to_string(),
            locator: "loc".to_string(),
            properties: BTreeMap::from([("k".to_string(), "v".to_string())]),
            object_id,
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
