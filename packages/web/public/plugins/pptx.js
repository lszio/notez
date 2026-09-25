/* notez attachment viewer plugin: PPTX.
 *
 * Renders a `.pptx` attachment inline by fetching the raw bytes (same
 * origin) and feeding them to the vendored `pptx-preview` library.
 * The library's UMD dist exposes the `pptxPreview` global; this
 * plugin loads that script on demand.
 *
 * Contract: register an async `(el, src) => void` renderer on
 * `window.notezViewers["pptx"]`. App.js calls it after the plugin
 * script is loaded.
 */
(function () {
  "use strict";
  var VENDOR_URL = "/vendor/pptx-preview.umd.js";
  function loadVendor() {
    if (window.pptxPreview && typeof window.pptxPreview.init === "function") {
      return Promise.resolve(window.pptxPreview);
    }
    return new Promise(function (resolve, reject) {
      var s = document.createElement("script");
      s.src = VENDOR_URL;
      s.onload = function () {
        if (window.pptxPreview && typeof window.pptxPreview.init === "function") {
          resolve(window.pptxPreview);
        } else {
          reject(new Error("pptx vendor script missing init"));
        }
      };
      s.onerror = function () { reject(new Error("failed to load " + VENDOR_URL)); };
      document.head.appendChild(s);
    });
  }

  if (!window.notezViewers) window.notezViewers = {};
  window.notezViewers.pptx = function (el, src) {
    return loadVendor().then(function (pptxPreview) {
      return fetch(src).then(function (r) {
        if (!r.ok) throw new Error("fetch " + r.status);
        return r.arrayBuffer();
      }).then(function (buf) {
        // `mode: "list"` stacks every slide; for a 16:9 deck that's
        // more useful in a tall preview pane than the single-slide
        // carousel.
        var viewer = pptxPreview.init(el, { mode: "list", renderer: "html" });
        return viewer.preview(buf);
      });
    });
  };
})();
