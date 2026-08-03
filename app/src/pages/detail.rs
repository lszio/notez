use dioxus::prelude::*;

use crate::server::get_resource;

#[component]
pub fn DetailPage(encoded_ref: String) -> Element {
    let decoded = urlencoding::decode(&encoded_ref)
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| encoded_ref.clone());
    // `use_server_future` takes ownership of its closure, so we hand it a
    // clone of the decoded ref. The original `decoded` stays in scope for
    // the title fallback below.
    let decoded_for_fetch = decoded.clone();
    let resource = use_server_future(move || {
        let r = decoded_for_fetch.clone();
        async move { get_resource(r).await }
    })?;

    rsx! {
        div { class: "min-h-screen p-6 bg-white text-gray-900",
            header { class: "mb-6 flex items-baseline justify-between",
                h1 { class: "text-2xl font-semibold",
                    match resource() {
                        Some(Ok(Some(row))) => format!("[{}] {}", row.ref_str, row.title),
                        _ => format!("{decoded}"),
                    }
                }
                a { class: "text-sm text-blue-600 hover:underline", href: "/",
                    "← 返回列表"
                }
            }
            section {
                match resource() {
                    Some(Ok(Some(row))) => rsx! {
                        div {
                            h2 { class: "text-lg font-semibold mb-2", "属性" }
                            table { class: "w-full border-collapse",
                                tbody {
                                    tr { class: "border-b",
                                        th { class: "text-left p-2 w-40", scope: "row", "kind" }
                                        td { class: "p-2 font-mono", "{row.kind}" }
                                    }
                                    tr { class: "border-b",
                                        th { class: "text-left p-2", scope: "row", "source_id" }
                                        td { class: "p-2 font-mono text-xs", "{row.source_id}" }
                                    }
                                    tr { class: "border-b",
                                        th { class: "text-left p-2", scope: "row", "locator" }
                                        td { class: "p-2 font-mono text-xs", "{row.locator}" }
                                    }
                                    tr { class: "border-b",
                                        th { class: "text-left p-2", scope: "row", "revision" }
                                        td { class: "p-2 font-mono text-xs", "{row.revision}" }
                                    }
                                    tr { class: "border-b",
                                        th { class: "text-left p-2", scope: "row", "object_id" }
                                        td { class: "p-2 font-mono text-xs", "{row.object_id}" }
                                    }
                                }
                            }
                        }
                        div { class: "mt-6",
                            h2 { class: "text-lg font-semibold mb-2", "properties" }
                            if row.properties.is_empty() {
                                p { class: "text-gray-500", "（无）" }
                            } else {
                                table { class: "w-full border-collapse",
                                    tbody {
                                        for (k, v) in row.properties.iter() {
                                            tr { class: "border-b",
                                                td { class: "p-2 font-mono text-xs w-1/3", "{k}" }
                                                td { class: "p-2 font-mono text-xs", "{v}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Some(Ok(None)) => rsx! {
                        p { class: "text-gray-600", "资源不存在" }
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
