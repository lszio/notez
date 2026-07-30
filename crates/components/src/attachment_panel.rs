use crate::resource_card::ResourceCard;
use dioxus::prelude::*;
use domain::Resource;
use preview::PreviewModel;

/// Side-panel listing all attachments associated with a hub/resource. Each
/// row is a compact `ResourceCard` so the preview body is rendered inline.
#[component]
pub fn AttachmentPanel(items: Vec<(Resource, PreviewModel)>) -> Element {
    rsx! {
        aside { class: "attachment-panel",
            h2 { class: "attachment-panel-title", "Attachments" }
            if items.is_empty() {
                p { class: "attachment-empty", "No attachments." }
            } else {
                ul { class: "attachment-list",
                    for (resource, model) in items {
                        li { key: "{resource.r#ref}",
                            class: "attachment-item",
                            ResourceCard { resource: resource.clone(), model: model.clone() }
                        }
                    }
                }
            }
        }
    }
}