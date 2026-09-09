//! Space picker for `/` when no default space is configured.
//!
//! Deliberately plain HTML: the page has no interactivity beyond a
//! form, and keeping it out of the Dioxus tree lets `/` redirect
//! straight into the default space without a render.

use crate::data::html::esc;
use crate::data::space::Space;
use crate::data::urls;

pub fn picker_page(spaces: &[Space]) -> String {
    let mut rows = String::new();
    for space in spaces {
        rows.push_str(&format!(
            "<a class=\"space-row\" href=\"{url}\"><span>{name}</span><span class=\"path\">{path}</span></a>",
            url = urls::space_url(&space.encoded),
            name = esc(&space.name),
            path = esc(&space.root.to_string_lossy()),
        ));
    }
    if rows.is_empty() {
        rows.push_str("<div class=\"edit-hint\">No space registered yet.</div>");
    }
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>spaces · notez</title>
<link rel="stylesheet" href="/app.css?v={version}">
</head>
<body>
<main class="content picker">
<h1>Spaces</h1>
<div class="spaces">{rows}</div>
<form method="post" action="/register">
<input type="text" name="path" placeholder="/home/you/Notes" autocomplete="off" spellcheck="false">
<button class="btn primary" type="submit">Open</button>
</form>
<p class="edit-hint">Registers the directory in <code>~/.config/notez/config.toml</code>.</p>
</main>
</body>
</html>
"#,
        version = crate::data::ASSET_VERSION,
        rows = rows,
    )
}
