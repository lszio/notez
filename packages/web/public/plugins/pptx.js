/* notez attachment viewer plugin: PPTX.
 *
 * Renders a `.pptx` attachment inline by fetching the raw bytes (same
 * origin) and feeding them to the vendored `pptx-preview` library.
 * The library's UMD dist exposes the `pptxPreview` global; this
 * plugin loads that script on demand.
 *
 * The library sizes the slide canvas from `init` options; without
 * width/height the wrapper keeps height 0 and renders nothing
 * visible. The deck's own aspect ratio (`<p:sldSz cx cy>` from
 * `ppt/presentation.xml`) decides the canvas — a 4:3 deck crammed
 * into a 16:9 canvas is visibly stretched.
 *
 * Contract: register an async `(el, src) => void` renderer on
 * `window.notezViewers["pptx"]`. App.js calls it after the plugin
 * script is loaded.
 */
(function () {
  "use strict";
  var JSZIP_URL = "/vendor/jszip.min.js";
  var VENDOR_URL = "/vendor/pptx-preview.umd.js";
  function loadScript(src, present) {
    if (present()) return Promise.resolve();
    return new Promise(function (resolve, reject) {
      var s = document.createElement("script");
      s.src = src;
      s.onload = function () {
        if (present()) resolve();
        else reject(new Error(src + " loaded without the expected global"));
      };
      s.onerror = function () { reject(new Error("failed to load " + src)); };
      document.head.appendChild(s);
    });
  }
  function loadVendor() {
    if (window.pptxPreview && typeof window.pptxPreview.init === "function") {
      return Promise.resolve(window.pptxPreview);
    }
    return loadScript(JSZIP_URL, function () { return !!window.JSZip; })
      .then(function () {
        return loadScript(VENDOR_URL, function () {
          return window.pptxPreview && typeof window.pptxPreview.init === "function";
        });
      })
      .then(function () { return window.pptxPreview; });
  }
  // Pull the deck's own slide dimensions out of `ppt/presentation.xml`
  // so the canvas isn't forced to a wrong aspect. Returns
  // `cy/cx` (height-per-width) or null when the xml doesn't carry
  // a recognisable `sldSz`.
  function slideAspect(buf) {
    if (!window.JSZip) return Promise.resolve(null);
    return window.JSZip.loadAsync(buf).then(function (zip) {
      var entry = zip.file("ppt/presentation.xml");
      if (!entry) return null;
      return entry.async("string").then(function (xml) {
        var m = xml.match(/<p:sldSz[^>]*\bcx="(\d+)"[^>]*\bcy="(\d+)"|<p:sldSz[^>]*\bcy="(\d+)"[^>]*\bcx="(\d+)"/);
        if (!m) return null;
        var cx = parseInt(m[1] || m[4], 10);
        var cy = parseInt(m[2] || m[3], 10);
        if (!(cx > 0 && cy > 0)) return null;
        return cy / cx;
      });
    }).catch(function () { return null; });
  }

  if (!window.notezViewers) window.notezViewers = {};
  window.notezViewers.pptx = function (el, src) {
    return loadVendor().then(function (pptxPreview) {
      return fetch(src).then(function (r) {
        if (!r.ok) throw new Error("fetch " + r.status);
        return r.arrayBuffer();
      }).then(function (buf) {
        // `mode: "list"` stacks every slide; the single-slide
        // carousel (`"slide"` mode) forces extra user clicks for a
        // document preview.
        var width = Math.max(320, Math.round(el.getBoundingClientRect().width)) || 960;
        // Default to 16:9; override once we've parsed the deck.
        var aspect = 9 / 16;
        return slideAspect(buf).then(function (a) {
          if (a) aspect = a;
          var viewer = pptxPreview.init(el, {
            mode: "list",
            renderer: "html",
            width: width,
            height: Math.round(width * aspect),
          });
          return viewer.preview(buf);
        });
      });
    });
  };
})();
