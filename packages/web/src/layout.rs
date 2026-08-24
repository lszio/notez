//! App shell — top nav bar + 3-column body.

use dioxus::prelude::*;

use crate::pages::{
    CommandPalette, FilesPanel, GraphPanel, PropertiesPanel, SearchTrigger, SpaceDropdown, SpaceSidebar, TreePanel,
};
use crate::space_ctx::{SpaceState, SpaceStatus};

/// PR9:
/// layer for the command palette. Lives in inline `<script>` (not
/// in `use_effect`) because Dioxus fullstack is SSR-only — effects
/// never run client-side. The script also wires the trigger /
/// close / backdrop click handlers because the same constraint
/// leaves the `<button onclick="…">` markup inert.
const PALETTE_KEY_LISTENER_JS: &str = r#"
    <script>
    (function () {
        if (window.__notezPaletteKeybound) { return; }
        window.__notezPaletteKeybound = true;
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
        document.addEventListener('click', function (e) {
            var trigger = e.target && e.target.closest && e.target.closest('.search-trigger');
            if (trigger) { e.preventDefault(); togglePalette(); return; }
            var closeBtn = e.target && e.target.closest && e.target.closest('.palette-close');
            if (closeBtn) { e.preventDefault(); closePalette(); return; }
            // Backdrop click: target === overlay div itself.
            var ov = getOverlay();
            if (ov && e.target === ov) { e.preventDefault(); closePalette(); }
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
                if (ov && ov.classList.contains('is-open')) {
                    e.preventDefault();
                    closePalette();
                }
            }
        });
    })();
    </script>
"#;
#[component]
pub fn Layout(children: Element) -> Element {
    use_context_provider(|| Signal::new(None::<SpaceState>));
    use_context_provider(|| Signal::new(None::<String>));
    // Palette open/closed flag — toggled by the search trigger and
    // read by the modal component. Kept here so any descendant
    // component (not just header siblings) can toggle it.
    use_context_provider(|| Signal::new(false));
    let space_ctx = use_context::<Signal<Option<SpaceState>>>();
    let active_path = space_ctx().map(|s| s.path.clone());
    let active_path_for_side = active_path.clone();
    let active_encoded = space_ctx().map(|s| s.encoded.clone());
    rsx! {
        // PR9: install the ⌘K / Esc listener as an inline script so
        // it runs at page load (no hydration dependency).
        div { dangerous_inner_html: "{PALETTE_KEY_LISTENER_JS}" }
        div { class: "shell",
            {
                let s = space_ctx();
                if let Some(SpaceState { status: SpaceStatus::Error(e), path, .. }) = s.clone() {
                    rsx! {
                        div { class: "banner-err",
                            div { class: "banner-err-inner",
                                span { class: "label", "space error →" }
                                span { class: "mono-sm", "{path}" }
                                span { class: "label", "—" }
                                span { "{e}" }
                            }
                        }
                    }
                } else {
                    rsx! { Fragment {} }
                }
            }

            // ----- Top navigation bar -----
            nav { class: "topnav",
                div { class: "topnav-inner",
                    SpaceSwitcher {
                        active_path: active_path_for_side.clone(),
                        active_encoded: active_encoded.clone(),
                    }
                    SearchTrigger {}
                    div { class: "topnav-spacer" }
                    TopNavActions {
                        active_path: active_path_for_side.clone(),
                        active_encoded: active_encoded.clone(),
                    }
                }
            }

            // ----- 3-column body -----
            div { class: "shell-body",
                aside { class: "col-left",
                    if active_path.is_some() {
                        TreePanel {
                            active_encoded: active_encoded.clone(),
                        }
                        div { class: "left-divider" }
                        FilesPanel {
                            active_encoded: active_encoded.clone(),
                        }
                    } else {
                        SpaceSidebar { active_path: active_path_for_side.clone() }
                    }
                }
                main { class: "col-main",
                    {children}
                }
                aside { class: "col-right",
                    PropertiesPanel {
                        active_encoded: active_encoded.clone(),
                    }
                    div { class: "right-divider" }
                    GraphPanel {
                        active_encoded: active_encoded.clone(),
                    }
                }
            }
            // ----- Command palette overlay (⌘K / Esc handled by the
            // PR9 inline script at the top of the page; the overlay
            // renders here when PALETTE_OPEN is true).
            CommandPalette {}
        }
    }
}

#[component]
fn SpaceSwitcher(active_path: Option<String>, active_encoded: Option<String>) -> Element {
    rsx! {
        div { class: "space-switcher",
            SpaceDropdown {
                active_path,
                active_encoded,
            }
        }
    }
}

#[component]
fn TopNavActions(active_path: Option<String>, active_encoded: Option<String>) -> Element {
    rsx! {
        div { class: "topnav-actions",
            if let (Some(p), Some(enc)) = (active_path.clone(), active_encoded.clone()) {
                form {
                    class: "topnav-form",
                    action: "/api/spaces/scan",
                    method: "post",
                    input {
                        r#type: "hidden",
                        name: "space_root",
                        value: "{p}",
                    }
                    button {
                        class: "topnav-action",
                        r#type: "submit",
                        title: "Re-index the space",
                        "scan"
                    }
                }
                a {
                    class: "topnav-action",
                    href: "{crate::router::route_for_space_list(&p)}",
                    title: "Resource list",
                    "list"
                }
                a {
                    class: "topnav-action",
                    href: "{crate::router::route_for_space_graph(&enc)}",
                    title: "Graph view",
                    "graph"
                }
            } else {
                a { class: "topnav-action", href: "/", "home" }
            }
        }
    }
}


