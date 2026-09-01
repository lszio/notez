//! App shell — sidebar + topbar + content slot.
//!
//! v2 note-workspace shell (replaces the topnav + app-rail + fixed
//! three-column shell):
//!
//! - **Sidebar** (`.sidebar`): brand, space switcher, quick search,
//!   primary nav (Home / Journal / Files / Graph / Activity), the
//!   file tree, and recent files for the active space. On narrow
//!   screens it becomes a slide-in drawer (`.drawer-open-nav` on the
//!   shell, toggled by `.drawer-toggle-nav`; delegated click handling
//!   lives in `public/index.html`).
//! - **Topbar** (`.topbar`): drawer toggle, current space name, the
//!   new-note menu, and (once a page opts in) the inspector drawer
//!   toggle. Pages render their own breadcrumbs inside `.content`.
//! - **Content slot**: the routed page. The note page renders its own
//!   right rail (outline / linked mentions / properties / local
//!   graph) so everything is SSR-deterministic.
//!
//! Dioxus fullstack is SSR-only for events: every interactive bit is
//! either a plain link, a POST form, or delegated vanilla JS.

use dioxus::prelude::*;

use crate::pages::{
    FilesPanel, IntentPalette, SearchTrigger, SpaceDropdown, TreePanel,
};
use crate::space_ctx::{SpaceState, SpaceStatus};

/// Ctrl/Cmd-K + Esc + trigger clicks for the command palette, plus
/// drawer toggling and outline jumps. Inline `<script>` because
/// `use_effect` never runs on the client (no hydration).
pub const SHELL_ENHANCE_JS: &str = r#"
    <script>
    (function () {
        if (window.__notezShellEnhance) { return; }
        window.__notezShellEnhance = true;
        var overlay = null;
        function getOverlay() {
            if (overlay && document.body.contains(overlay)) return overlay;
            overlay = document.querySelector('.palette-overlay');
            return overlay;
        }
        function openPalette() {
            var ov = getOverlay();
            if (!ov) return;
            ov.classList.add('is-open');
            var input = ov.querySelector('.palette-input');
            if (input) { setTimeout(function () { input.focus(); input.select && input.select(); }, 0); }
        }
        function closePalette() {
            var ov = getOverlay();
            if (!ov) return;
            ov.classList.remove('is-open');
        }
        function togglePalette() {
            var ov = getOverlay();
            if (!ov) return;
            if (ov.classList.contains('is-open')) { closePalette(); }
            else { openPalette(); }
        }
        function drawer(side) {
            var shell = document.querySelector('.shell');
            if (!shell) return;
            var cls = side === 'right' ? 'drawer-open-inspector' : 'drawer-open-nav';
            shell.classList.toggle(cls);
            var btn = document.querySelector(side === 'right' ? '.drawer-toggle-inspector' : '.drawer-toggle-nav');
            if (btn) { btn.setAttribute('aria-expanded', shell.classList.contains(cls) ? 'true' : 'false'); }
        }
        function closeDrawers() {
            var shell = document.querySelector('.shell');
            if (shell) { shell.classList.remove('drawer-open-nav', 'drawer-open-inspector'); }
        }
        document.addEventListener('click', function (e) {
            var t = e.target;
            var trigger = t && t.closest && t.closest('.search-trigger');
            if (trigger) { e.preventDefault(); togglePalette(); return; }
            var closeBtn = t && t.closest && t.closest('.palette-close');
            if (closeBtn) { e.preventDefault(); closePalette(); return; }
            var ov = getOverlay();
            if (ov && t === ov) { e.preventDefault(); closePalette(); return; }
            if (t && t.closest && t.closest('.drawer-toggle-nav')) { e.preventDefault(); drawer('nav'); return; }
            if (t && t.closest && t.closest('.drawer-toggle-inspector')) { e.preventDefault(); drawer('right'); return; }
            // Links close the mobile drawer after navigation.
            if (t && t.closest && t.closest('.sidebar a')) { closeDrawers(); return; }
            var backdrop = t && t.closest && t.closest('.drawer-backdrop');
            if (backdrop) { closeDrawers(); return; }
            // Outline jump: scroll to the idx-th heading of the note body.
            var outlineBtn = t && t.closest && t.closest('[data-outline-idx]');
            if (outlineBtn) {
                e.preventDefault();
                var body = document.querySelector('.note-body');
                if (!body) return;
                var heads = body.querySelectorAll('h1,h2,h3,h4,h5,h6');
                var idx = parseInt(outlineBtn.getAttribute('data-outline-idx'), 10);
                if (heads && idx < heads.length) { heads[idx].scrollIntoView({ behavior: 'smooth', block: 'start' }); }
            }
        });
        document.addEventListener('keydown', function (e) {
            var meta = e.metaKey || e.ctrlKey;
            if (meta && (e.key === 'k' || e.key === 'K')) {
                e.preventDefault();
                togglePalette();
                return;
            }
            if (e.key === 'Escape') {
                var ov = getOverlay();
                if (ov && ov.classList.contains('is-open')) { closePalette(); return; }
                closeDrawers();
            }
        });
    })();
    </script>
"#;

#[component]
pub fn Layout(children: Element) -> Element {
    use_context_provider(|| Signal::new(None::<SpaceState>));
    use_context_provider(|| Signal::new(None::<String>));
    use_context_provider(|| Signal::new(false));
    let space_ctx = use_context::<Signal<Option<SpaceState>>>();
    let active_path = space_ctx().map(|s| s.path.clone());
    let active_encoded = space_ctx().map(|s| s.encoded.clone());
    let space_leaf = active_path
        .as_deref()
        .and_then(|p| std::path::Path::new(p).file_name().and_then(|s| s.to_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| "notez".to_string());
    let space_ready = matches!(space_ctx().as_ref().map(|s| &s.status), Some(SpaceStatus::Ready(_)));

    rsx! {
        div { dangerous_inner_html: "{SHELL_ENHANCE_JS}" }
        div { class: "shell",
            aside { id: "col-left", class: "sidebar", "aria-label": "Workspace sidebar",
                div { class: "sb-brand",
                    a { class: "sb-wordmark", href: "/", "Notez" }
                    ThemeToggle {}
                }
                div { class: "sb-switch",
                    SpaceDropdown {
                        active_path: active_path.clone(),
                        active_encoded: active_encoded.clone(),
                    }
                }
                div { class: "sb-search",
                    SearchTrigger {}
                }
                nav { class: "sb-nav", "aria-label": "Sections",
                    a { class: if active_path.is_none() { "sb-link is-current" } else { "sb-link" }, href: "/",
                        span { class: "sb-link-icon", "⌂" }
                        span { "Home" }
                    }
                    if let Some(enc) = active_encoded.clone() {
                        a { class: "sb-link", href: "{crate::router::route_for_space_journal(&crate::router::decode_space(&enc))}",
                            span { class: "sb-link-icon", "🗓" }
                            span { "Journal" }
                        }
                        a { class: "sb-link", href: "{crate::router::route_for_space_files(&crate::router::decode_space(&enc))}",
                            span { class: "sb-link-icon", "▤" }
                            span { "Files" }
                        }
                        a { class: "sb-link", href: "{crate::router::route_for_space_graph(&enc)}",
                            span { class: "sb-link-icon", "◎" }
                            span { "Graph" }
                        }
                        a { class: "sb-link", href: "{crate::router::route_for_space_activity(&crate::router::decode_space(&enc))}",
                            span { class: "sb-link-icon", "≡" }
                            span { "Activity" }
                        }
                    }
                }
                div { class: "sb-tree",
                    if space_ready {
                        TreePanel { active_encoded: active_encoded.clone() }
                        FilesPanel { active_encoded: active_encoded.clone() }
                    } else if active_path.is_none() {
                        p { class: "sb-hint", "Open or register a source to browse its notes." }
                    }
                }
            }

            div { class: "main-col",
                div { class: "topbar",
                    button {
                        class: "drawer-toggle drawer-toggle-nav",
                        r#type: "button",
                        "aria-controls": "col-left",
                        "aria-expanded": "false",
                        "aria-label": "Toggle sidebar",
                        title: "Sidebar",
                        "☰"
                    }
                    span { class: "topbar-space", "{space_leaf}" }
                    div { class: "topbar-spacer" }
                    if space_ready {
                        details { class: "new-note",
                            summary { class: "btn btn-accent new-note-btn", "+ note" }
                            form { class: "new-note-form", method: "post", action: "/api/sources/document/create",
                                input { r#type: "hidden", name: "source_root", value: "{active_path.clone().unwrap_or_default()}" }
                                label { class: "field",
                                    span { class: "field-label", "Note title" }
                                    input { name: "title", r#type: "text", required: true, placeholder: "my-next-idea", "aria-label": "New note title" }
                                }
                                button { class: "btn btn-accent", r#type: "submit", "Create" }
                            }
                        }
                    }
                }
                main { class: "content",
                    {children}
                }
            }

            div { class: "drawer-backdrop" }
            IntentPalette {}
        }
    }
}

#[component]
fn ThemeToggle() -> Element {
    rsx! {
        button {
            class: "theme-toggle",
            r#type: "button",
            "data-theme-toggle": "true",
            "aria-label": "Theme: system",
            title: "Theme: light / dark / system",
            "◐"
        }
    }
}
