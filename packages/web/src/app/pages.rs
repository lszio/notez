//! The read-only pages: space index, document/attachment view, new note.

use dioxus::prelude::*;

use crate::app::edit::{restore_notice, save_notice, Editor};
use crate::app::route::DocQuery;
use crate::app::shell::{topbar_doc, topbar_space, Shell};
use crate::data::space::{self, Entry, Space};
use crate::data::urls;
use ui::NzDocBody;

/// Shared error rendering (a failed space open, missing file, …).
fn error_body(message: &str) -> Element {
    rsx! {
        div { class: "banner err", "{message}" }
        p { class: "edit-hint",
            a { href: "/", "back to the space picker" }
        }
    }
}

fn open_space(encoded: &str) -> Result<Space, Element> {
    space::open(encoded).map_err(|e| error_body(e.message()))
}

/// `/s/{space}` — README/index page when present, otherwise the page list.
#[component]
pub fn SpaceIndexPage(encoded: String) -> Element {
    let space = match open_space(&encoded) {
        Ok(s) => s,
        Err(node) => return node,
    };
    let entries = space::entries(&space.root).unwrap_or_default();
    let landing = ["README.org", "README.md", "index.org", "index.md"]
        .into_iter()
        .find(|name| space.root.join(name).is_file());

    match landing {
        Some(locator) => {
            let (content, revision) = match space::read_doc(&space.root, locator) {
                Ok(v) => v,
                Err(e) => return error_body(e.message()),
            };
            let title = space::doc_title(&space.root, locator, &content);
            let html = space::render_document(&space, locator, &content).unwrap_or_default();
            let edit = urls::edit_url(&encoded, locator);
            let _ = revision;
            rsx! {
                Shell {
                    space: space.clone(),
                    title: title.clone(),
                    current: Some(locator.to_string()),
                    return_to: urls::space_url(&encoded),
                    topbar: topbar_space(&space, &title),
                    NzDocBody { html }
                    hr {}
                    p { class: "edit-hint", a { href: "{edit}", "edit {locator}" } }
                }
            }
        }
        None => {
            let docs = entries.iter().filter(|e| e.editable()).count();
            rsx! {
                Shell {
                    space: space.clone(),
                    title: space.name.clone(),
                    current: None,
                    return_to: urls::space_url(&encoded),
                    topbar: topbar_space(&space, &space.name.clone()),
                    h1 { "{space.name}" }
                    p { class: "edit-hint", "{entries.len()} files · {docs} documents" }
                    PageList { space: space.clone(), entries: entries.clone() }
                }
            }
        }
    }
}

/// Documents grouped by folder.
#[component]
fn PageList(space: Space, entries: Vec<Entry>) -> Element {
    let mut groups: std::collections::BTreeMap<String, Vec<Entry>> = std::collections::BTreeMap::new();
    for entry in entries.into_iter().filter(|e| e.editable()) {
        groups.entry(entry.dir().to_string()).or_default().push(entry);
    }
    if groups.is_empty() {
        return rsx! { p { class: "edit-hint", "No markdown or org files in this space yet." } };
    }
    rsx! {
        for (dir, rows) in groups {
            h3 { if dir.is_empty() { "(root)" } else { "{dir}" } }
            ul {
                for row in rows {
                    li {
                        a { href: "{urls::view_url(&space.encoded, &row.locator)}", "{row.title}" }
                        span { class: "edit-hint", " {fmt_size(row.size)}" }
                    }
                }
            }
        }
    }
}

fn fmt_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit + 1 < UNITS.len() {
        v /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{v:.1} {}", UNITS[unit]) }
}

/// `/s/{space}/{locator}` — read a document, edit it (`?edit=1`), or
/// preview an attachment.
#[component]
pub fn DocPage(encoded: String, locator: Vec<String>, query: DocQuery) -> Element {
    let space = match open_space(&encoded) {
        Ok(s) => s,
        Err(node) => return node,
    };
    let locator = locator.join("/");
    let view_url = urls::view_url(&encoded, &locator);
    let editable = space::is_doc_locator(&locator);

    if !editable {
        let title = std::path::Path::new(&locator)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&locator)
            .to_string();
        let html = match space::render_file(&space, &locator, &title) {
            Ok(h) => h,
            Err(e) => return error_body(e.message()),
        };
        return rsx! {
            Shell {
                space: space.clone(),
                title: title.clone(),
                current: Some(locator.clone()),
                return_to: view_url,
                topbar: topbar_doc(&space, &locator, &title, false, false),
                NzDocBody { html }
            }
        };
    }

    let (content, revision) = match space::read_doc(&space.root, &locator) {
        Ok(v) => v,
        Err(e) => return error_body(e.message()),
    };
    let title = space::doc_title(&space.root, &locator, &content);

    if query.edit.as_deref() == Some("1") {
        // A failed save redirected here with a one-shot token; the
        // stashed submission replaces the on-disk text and carries the
        // on-disk revision so the next save is a deliberate overwrite.
        let restored = query
            .restore
            .as_deref()
            .and_then(crate::data::pending::take)
            .filter(|p| p.locator == locator);
        let (content, revision, notice) = match &restored {
            Some(pending) => (
                pending.content.clone(),
                pending.revision.clone(),
                restore_notice(pending),
            ),
            None => (content, revision, save_notice(&query)),
        };
        return rsx! {
            Shell {
                space: space.clone(),
                title: format!("{title} — edit"),
                current: Some(locator.clone()),
                return_to: view_url,
                topbar: topbar_doc(&space, &locator, &title, true, true),
                Editor {
                    space: space.clone(),
                    locator: locator.clone(),
                    revision,
                    content,
                    notice,
                }
            }
        };
    }

    let html = space::render_document(&space, &locator, &content).unwrap_or_default();
    rsx! {
        Shell {
            space: space.clone(),
            title: title.clone(),
            current: Some(locator.clone()),
            return_to: view_url,
            topbar: topbar_doc(&space, &locator, &title, false, true),
            {save_notice(&query)}
            NzDocBody { html }
            hr {}
            p { class: "edit-hint",
                a { href: "{urls::raw_url(&encoded, &locator)}", "raw source" }
                " · "
                a { href: "{urls::edit_url(&encoded, &locator)}", "edit" }
            }
        }
    }
}

/// `/new/{space}` — create a note.
#[component]
pub fn NewPage(encoded: String) -> Element {
    let space = match open_space(&encoded) {
        Ok(s) => s,
        Err(node) => return node,
    };
    let action = urls::new_action_url(&encoded);
    let root = space.root.display().to_string();
    rsx! {
        Shell {
            space: space.clone(),
            title: "new note".to_string(),
            current: None,
            return_to: urls::space_url(&encoded),
            topbar: topbar_space(&space, "New note"),
            h1 { "New note" }
            form { class: "picker", method: "post", action: "{action}",
                input {
                    r#type: "text",
                    name: "name",
                    placeholder: "notes/my-idea  or  notes/my-idea.org",
                    autocomplete: "off",
                    spellcheck: "false",
                    autofocus: true,
                }
                button { class: "btn primary", r#type: "submit", "Create" }
            }
            p { class: "edit-hint", "Path is relative to {root}. .md is appended when no extension is given." }
        }
    }
}
