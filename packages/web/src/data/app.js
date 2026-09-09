/* notez workspace islands.
 *
 * The pages are server-rendered; this file only adds what HTML cannot
 * do on its own:
 *   1. sidebar filter + tree state
 *   2. editor toolbar, drafts, live preview, shortcuts
 *   3. "changed on disk" watch pill
 * Every feature degrades gracefully: with JS disabled the pages still
 * read, navigate and save through plain links and forms.
 */
(function () {
  "use strict";

  var $ = function (sel, root) { return (root || document).querySelector(sel); };
  var $$ = function (sel, root) { return Array.prototype.slice.call((root || document).querySelectorAll(sel)); };
  // The shell carries the active space (body attributes are not
  // available to the SSR component tree).
  var spaceAttr = function () {
    var el = document.querySelector("[data-space]");
    return el ? el.getAttribute("data-space") : "";
  };

  /* ---------------------------------------------------------------- 1. sidebar */

  function initSidebar() {
    var filter = $("#filter");
    var tree = $("#tree");
    if (filter && tree) {
      var rows = $$("[data-k]", tree);
      var files = $$(".tree-file", tree);
      var count = $("#count");
      var apply = function () {
        var q = filter.value.trim().toLowerCase();
        var shown = 0;
        rows.forEach(function (row) {
          var hit = !q || row.getAttribute("data-k").indexOf(q) !== -1;
          row.hidden = !hit;
          if (hit && row.classList.contains("tree-file")) shown++;
        });
        // A folder stays visible when it matches or has a visible
        // descendant.
        $$("details", tree).reverse().forEach(function (d) {
          if (!q) { d.hidden = false; return; }
          var selfHit = d.getAttribute("data-k").indexOf(q) !== -1;
          d.hidden = !(selfHit || $("[data-k]:not([hidden])", d));
        });
        if (count) count.textContent = q ? shown + "/" + files.length : String(files.length);
      };
      filter.addEventListener("input", apply);
      apply();
    }

    // Remember which folders are open, per space.
    var spaceKey = spaceAttr();
    var storeKey = "notez.tree." + (spaceKey || "default");
    var open = {};
    try { open = JSON.parse(localStorage.getItem(storeKey) || "{}"); } catch (e) { open = {}; }
    $$("#tree details[data-dir]").forEach(function (d) {
      var path = d.getAttribute("data-dir");
      if (path in open) d.open = open[path];
      d.addEventListener("toggle", function () {
        open[path] = d.open;
        try { localStorage.setItem(storeKey, JSON.stringify(open)); } catch (e) {}
      });
    });

    document.addEventListener("keydown", function (e) {
      var tag = (document.activeElement && document.activeElement.tagName) || "";
      if (e.key === "/" && !/^(INPUT|TEXTAREA|SELECT)$/.test(tag)) {
        if (filter) { e.preventDefault(); filter.focus(); }
      }
    });
  }

  /* ---------------------------------------------------------------- 2. editor */

  var MARKERS = {
    bold: ["*", "*"],
    italic: ["/", "/"],
    code: ["=", "="],
    strike: ["+", "+"],
  };

  function initEditor() {
    var form = $("#editor-form");
    var area = $("#editor");
    if (!form || !area) return;

    var locator = form.getAttribute("data-locator") || "";
    var space = form.getAttribute("data-space") || "";
    var draftKey = "notez.draft." + space + "/" + locator;
    var initial = area.value;
    var dirty = false;
    var state = $("#editor-state");

    function setDirty(next) {
      dirty = next;
      if (state) state.textContent = next ? "unsaved changes" : "saved";
      if (next) {
        try { localStorage.setItem(draftKey, area.value); } catch (e) {}
      } else {
        try { localStorage.removeItem(draftKey); } catch (e) {}
      }
    }

    // Offer a draft from a previous session when it differs from disk.
    try {
      var draft = localStorage.getItem(draftKey);
      if (draft !== null && draft !== initial) {
        var bar = $("#draft-bar");
        if (bar) {
          bar.hidden = false;
          var restore = $("#draft-restore");
          if (restore) restore.addEventListener("click", function (e) {
            e.preventDefault();
            area.value = draft;
            setDirty(true);
            bar.hidden = true;
          });
          var drop = $("#draft-discard");
          if (drop) drop.addEventListener("click", function (e) {
            e.preventDefault();
            try { localStorage.removeItem(draftKey); } catch (err) {}
            bar.hidden = true;
          });
        }
      }
    } catch (e) {}

    area.addEventListener("input", function () { setDirty(area.value !== initial); });
    form.addEventListener("submit", function () {
      dirty = false;
      try { localStorage.removeItem(draftKey); } catch (e) {}
    });
    window.addEventListener("beforeunload", function (e) {
      if (dirty) { e.preventDefault(); e.returnValue = ""; }
    });

    // --- selection helpers

    function surround(before, after) {
      var s = area.selectionStart, t = area.selectionEnd;
      var selected = area.value.slice(s, t) || "text";
      var next = before + selected + after;
      area.setRangeText(next, s, t, "select");
      area.selectionStart = s + before.length;
      area.selectionEnd = s + before.length + selected.length;
      setDirty(true);
    }

    function prefixLines(prefix, togglePrefixes) {
      var s = area.selectionStart, t = area.selectionEnd;
      var start = area.value.lastIndexOf("\n", s - 1) + 1;
      var end = area.value.indexOf("\n", t);
      if (end === -1) end = area.value.length;
      var block = area.value.slice(start, end);
      var lines = block.split("\n");
      var already = lines.every(function (l) { return togglePrefixes.some(function (p) { return l.trim().indexOf(p) === 0; }); });
      var out = lines.map(function (l) {
        if (already) {
          for (var i = 0; i < togglePrefixes.length; i++) {
            var p = togglePrefixes[i];
            var idx = l.indexOf(p);
            if (idx !== -1) return l.slice(0, idx) + l.slice(idx + p.length);
          }
          return l;
        }
        return prefix + l;
      }).join("\n");
      area.setRangeText(out, start, end, "end");
      setDirty(true);
    }

    function insertLink() {
      var s = area.selectionStart, t = area.selectionEnd;
      var selected = area.value.slice(s, t) || "label";
      var url = window.prompt("link target (relative to this file)", "https://");
      if (url === null) return;
      var next = "[[file:" + url + "][" + selected + "]]";
      area.setRangeText(next, s, t, "end");
      setDirty(true);
    }

    var ACTIONS = {
      bold: function () { surround("*", "*"); },
      italic: function () { surround("/", "/"); },
      code: function () { surround("=", "="); },
      strike: function () { surround("+", "+"); },
      h1: function () { prefixLines("* ", ["* "]); },
      h2: function () { prefixLines("** ", ["** "]); },
      h3: function () { prefixLines("*** ", ["*** "]); },
      list: function () { prefixLines("- ", ["- ", "+ ", "* "]); },
      ordered: function () { prefixLines("1. ", ["1. "]); },
      checkbox: function () { prefixLines("- [ ] ", ["- [ ] ", "- [x] "]); },
      quote: function () { prefixLines("#+begin_quote\n", ["#+begin_quote"]); },
      link: insertLink,
      table: function () {
        var s = area.selectionStart, t = area.selectionEnd;
        var table = "| 列 1 | 列 2 |\n|------+------|\n|      |      |\n";
        area.setRangeText(table, s, t, "end");
        setDirty(true);
      },
    };

    $$("[data-action]", document).forEach(function (btn) {
      btn.addEventListener("click", function (e) {
        e.preventDefault();
        var fn = ACTIONS[btn.getAttribute("data-action")];
        if (fn) { fn(); area.focus(); }
      });
    });

    area.addEventListener("keydown", function (e) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        // requestSubmit fires the submit event, so the dirty guard is
        // cleared before the navigation starts.
        if (form.requestSubmit) form.requestSubmit(); else form.submit();
        return;
      }
      if (e.ctrlKey || e.metaKey) {
        var key = e.key.toLowerCase();
        var map = { b: "bold", i: "italic", e: "code", k: "link" };
        if (map[key]) { e.preventDefault(); ACTIONS[map[key]](); }
        return;
      }
      if (e.key === "Tab") {
        e.preventDefault();
        var s = area.selectionStart, t = area.selectionEnd;
        if (e.shiftKey) {
          var lineStart = area.value.lastIndexOf("\n", s - 1) + 1;
          if (area.value.slice(lineStart, lineStart + 2) === "  ") {
            area.setRangeText("", lineStart, lineStart + 2, "preserve");
          }
        } else {
          area.setRangeText("  ", s, t, "end");
        }
        setDirty(true);
      }
    });

    // --- live preview

    var preview = $("#preview");
    if (preview) {
      var timer = null;
      var render = function () {
        var body = new URLSearchParams();
        body.set("encoded", space);
        body.set("locator", locator);
        body.set("content", area.value);
        fetch("/api/render", {
          method: "POST",
          headers: { "Content-Type": "application/x-www-form-urlencoded" },
          body: body.toString(),
        })
          .then(function (r) { return r.ok ? r.text() : Promise.reject(r.status); })
          .then(function (html) { preview.innerHTML = html; })
          .catch(function () { preview.textContent = "preview unavailable"; });
      };
      var schedule = function () {
        clearTimeout(timer);
        timer = setTimeout(render, 250);
      };
      area.addEventListener("input", schedule);
      render();
    }
  }

  /* ---------------------------------------------------------------- 3. watch pill */

  function initStatus() {
    var pill = $("#watch-pill");
    var space = spaceAttr();
    if (!pill || !space) return;
    var current = pill.getAttribute("data-fingerprint") || "";
    var editor = $("#editor");

    setInterval(function () {
      fetch("/api/space-status?encoded=" + encodeURIComponent(space), { headers: { Accept: "application/json" } })
        .then(function (r) { return r.ok ? r.json() : null; })
        .then(function (s) {
          if (!s) return;
          if (s.fingerprint && s.fingerprint !== current) {
            pill.hidden = false;
            pill.textContent = "changed on disk — reload";
          }
        })
        .catch(function () {});
    }, 5000);

    pill.addEventListener("click", function (e) {
      e.preventDefault();
      if (editor && editor.value !== editor.defaultValue) {
        if (!window.confirm("Discard unsaved changes and reload?")) return;
      }
      location.reload();
    });
  }

  function boot() {
    initSidebar();
    initEditor();
    initStatus();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    boot();
  }
})();
