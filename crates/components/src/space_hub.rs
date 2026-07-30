use crate::{agenda_list::AgendaList, SpaceSummary};
use dioxus::prelude::*;
use domain::Resource;

/// Top-level hub page for a single space. Composes the metadata header,
/// the resource index, and the agenda list. Designed to be the SSR target
/// for the `/s/{name}` route.
#[component]
pub fn SpaceHub(
    summary: SpaceSummary,
    resources: Vec<Resource>,
    agenda: Vec<crate::agenda_list::TaskItem>,
) -> Element {
    rsx! {
        section { class: "space-hub",
            header { class: "space-hub-header",
                h1 { class: "space-name", "{summary.name}" }
                p { class: "space-root", "{summary.root.display()}" }
            }

            section { class: "space-hub-resources",
                h2 { "Resources ({resources.len()})" }
                if resources.is_empty() {
                    p { class: "empty", "No resources indexed yet." }
                } else {
                    ul { class: "resource-list",
                        for r in resources {
                            li { key: "{r.r#ref}",
                                class: "resource-item kind-{r.kind}",
                                a { href: "/s/{summary.name}/r/{r.r#ref}",
                                    "{r.title}"
                                    span { class: "resource-locator", " — {r.locator}" }
                                }
                            }
                        }
                    }
                }
            }

            section { class: "space-hub-agenda",
                h2 { "Agenda" }
                AgendaList { items: agenda }
            }
        }
    }
}