use crate::resource_card::ResourceCard;
use dioxus::prelude::*;
use domain::Resource;
use preview::PreviewModel;

/// Recursive link-embed card. The `child` is rendered by `ResourceCard`,
/// which itself pattern-matches back through `PreviewModel::LinkEmbed` —
/// the recursion is bounded by the depth of `[[link:…]]` chains in the
/// source document.
#[component]
pub fn LinkEmbed(target: Resource, child: PreviewModel) -> Element {
    rsx! {
        div { class: "link-embed",
            header { class: "link-embed-header",
                span { class: "link-embed-label", "→ {crate::escape::escape_html(&target.title)}" }
            }
            div { class: "link-embed-body",
                ResourceCard { resource: target, model: child }
            }
        }
    }
}