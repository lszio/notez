use dioxus::prelude::*;

use crate::model::ResourceRow;
use crate::server::list_resources;

#[component]
pub fn ListPage() -> Element {
    let resources = use_server_future(|| async { list_resources().await })?;

    rsx! {
        div { class: "min-h-screen p-6 bg-white text-gray-900",
            header { class: "mb-6",
                h1 { class: "text-2xl font-semibold", "Notez" }
                p { class: "text-sm text-gray-600",
                    match resources() {
                        Some(Ok(rows)) => format!("{} resources", rows.len()),
                        Some(Err(e)) => format!("error: {e}"),
                        None => "loading…".to_string(),
                    }
                }
            }
            section {
                match resources() {
                    Some(Ok(rows)) if rows.is_empty() => rsx! {
                        p { class: "text-gray-600",
                            "该 Space 还没有被扫描过。运行 `notez scan`。"
                        }
                    },
                    Some(Ok(rows)) => rsx! {
                        ResourceTable { rows }
                    },
                    Some(Err(e)) => rsx! {
                        p { class: "text-red-600", "加载失败：{e}" }
                    },
                    None => rsx! {
                        p { class: "text-gray-500", "loading…" }
                    },
                }
            }
        }
    }
}

#[component]
fn ResourceTable(rows: Vec<ResourceRow>) -> Element {
    rsx! {
        table { class: "w-full border-collapse",
            thead { class: "border-b",
                tr {
                    th { class: "text-left p-2", "ref" }
                    th { class: "text-left p-2", "title" }
                    th { class: "text-left p-2", "source_id" }
                    th { class: "text-left p-2", "locator" }
                }
            }
            tbody {
                for row in rows.iter() {
                    tr { class: "border-b hover:bg-gray-50",
                        td { class: "p-2 font-mono text-xs",
                            a {
                                href: format!("/resource/{}", urlencoding::encode(&row.ref_str)),
                                "{row.ref_str}"
                            }
                        }
                        td { class: "p-2", "{row.title}" }
                        td { class: "p-2 font-mono text-xs", "{row.source_id}" }
                        td { class: "p-2 font-mono text-xs", "{row.locator}" }
                    }
                }
            }
        }
    }
}
