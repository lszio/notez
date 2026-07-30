use dioxus::prelude::*;
use domain::Resource;
use preview::PreviewModel;

/// Render a `Resource` alongside the preview produced by its `Previewer`.
/// The body branch is selected via `match` on `PreviewModel` so each
/// previewer has its own dedicated rendering path. String fields that came
/// from a file are escaped via `crate::escape::escape_html` before being
/// emitted as HTML to prevent injection from untrusted source files.
///
/// C3 ships the data-rendering variants (Org, Markdown, Pdf, Xlsx, Pptx,
/// Zip, Image, QueryEmbed, Fallback). The embed/diagram variants
/// (Mermaid, D2, Iframe, LinkEmbed, BlockEmbed) are wired in C4.
#[component]
pub fn ResourceCard(resource: Resource, model: PreviewModel) -> Element {
    let title = crate::escape::escape_html(&resource.title);
    let locator = crate::escape::escape_html(&resource.locator);
    rsx! {
        article { class: "resource-card kind-{resource.kind}",
            header { class: "resource-card-header",
                h3 { class: "resource-card-title",
                    a { href: "/r/{resource.r#ref}", "{title}" }
                }
                p { class: "resource-card-locator", "{locator}" }
            }
            div { class: "resource-card-body",
                {render_body(model)}
            }
        }
    }
}

fn render_body(model: PreviewModel) -> Element {
    match model {
        PreviewModel::Org { html, outline: _ } => rsx! {
            div { class: "preview-org",
                div { class: "preview-org-content",
                    div { dangerous_inner_html: "{html}" }
                }
            }
        },
        PreviewModel::Markdown { html } => rsx! {
            div { class: "preview-markdown", div { dangerous_inner_html: "{html}" } }
        },
        PreviewModel::Pdf { pages, text } => rsx! {
            div { class: "preview-pdf",
                p { class: "preview-pdf-summary",
                    "{pages.len()} pages, {text.len()} chars extracted"
                }
                for page in pages {
                    div { class: "preview-pdf-page",
                        h4 { "Page {page.index + 1}" }
                        pre { class: "preview-pdf-text",
                            "{crate::escape::escape_html(&page.text)}"
                        }
                    }
                }
            }
        },
        PreviewModel::Xlsx { sheets } => rsx! {
            div { class: "preview-xlsx",
                for sheet in sheets {
                    div { class: "preview-xlsx-sheet",
                        h4 { "{sheet.name}" }
                        table { class: "preview-xlsx-table",
                            for row in sheet.rows {
                                tr {
                                    for cell in row {
                                        td { "{crate::escape::escape_html(&cell)}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        PreviewModel::Pptx { slides } => rsx! {
            div { class: "preview-pptx",
                for slide in slides {
                    div { class: "preview-pptx-slide",
                        h4 {
                            if let Some(title) = &slide.title {
                                "{crate::escape::escape_html(title)}"
                            } else {
                                "Slide {slide.index + 1}"
                            }
                        }
                        for line in &slide.body {
                            p { "{crate::escape::escape_html(line)}" }
                        }
                        if let Some(notes) = &slide.notes {
                            p { class: "preview-pptx-notes",
                                "{crate::escape::escape_html(notes)}"
                            }
                        }
                    }
                }
            }
        },
        PreviewModel::Zip { entries } => rsx! {
            div { class: "preview-zip",
                p { "{entries.len()} entries" }
                ul {
                    for entry in entries {
                        li {
                            if entry.is_dir { "📁 " } else { "📄 " }
                            "{crate::escape::escape_html(&entry.path)}"
                            span { class: "preview-zip-size", " ({entry.size} bytes)" }
                        }
                    }
                }
            }
        },
        PreviewModel::Image { src, width, height, mime } => rsx! {
            div { class: "preview-image",
                img {
                    src: "{src}",
                    alt: "{crate::escape::escape_html(&alt_from_src(&src))}",
                    width: "{width}",
                    height: "{height}"
                }
                p { class: "preview-image-meta", "{mime} — {width}×{height}" }
            }
        },
        PreviewModel::Mermaid { source: _ } => rsx! {
            div { class: "preview-placeholder",
                "Mermaid diagram (rendered in C4)"
            }
        },
        PreviewModel::D2 { source: _ } => rsx! {
            div { class: "preview-placeholder",
                "D2 diagram (rendered in C4)"
            }
        },
        PreviewModel::Iframe { src: _, sandbox: _ } => rsx! {
            div { class: "preview-placeholder",
                "Iframe embed (rendered in C4)"
            }
        },
        PreviewModel::LinkEmbed { target: _, child: _ } => rsx! {
            div { class: "preview-placeholder",
                "Link embed (rendered in C4)"
            }
        },
        PreviewModel::BlockEmbed { source: _, html: _ } => rsx! {
            div { class: "preview-placeholder",
                "Block embed (rendered in C4)"
            }
        },
        PreviewModel::QueryEmbed { query, snapshot } => rsx! {
            div { class: "preview-query-embed",
                header {
                    h4 { "Query: {crate::escape::escape_html(&query.source)}" }
                    if let Some(hint) = &query.kind_hint {
                        span { class: "query-hint", "kind: {crate::escape::escape_html(hint)}" }
                    }
                }
                p { class: "query-summary", "{snapshot.len()} result(s)" }
                ul { class: "query-snapshot",
                    for r in snapshot {
                        li { key: "{r.r#ref}", "{r.title}" }
                    }
                }
            }
        },
        PreviewModel::Fallback { message } => rsx! {
            div { class: "preview-fallback",
                p { "{crate::escape::escape_html(&message)}" }
            }
        },
    }
}

fn alt_from_src(src: &str) -> String {
    std::path::Path::new(src)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(src)
        .to_string()
}