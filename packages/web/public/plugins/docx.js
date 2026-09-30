/* notez attachment viewer plugin: DOCX.
 *
 * Renders a `.docx` attachment inline by fetching the raw bytes (same
 * origin) and feeding them to the vendored `docx-preview` library.
 * The library's UMD dist exposes the `docx` global; this plugin loads
 * that script on demand so a notez instance without the plugin does
 * not pay the size cost.
 *
 * docx-preview 0.4.x's UMD build reads its only runtime dependency
 * (jszip) from `window.JSZip`, so the vendored jszip is loaded first
 * (`/vendor/jszip.min.js`) — without it the renderer throws
 * "Cannot read properties of undefined (reading 'loadAsync')".
 *
 * Contract: register an async `(el, src) => void` renderer on
 * `window.notezViewers["docx"]`. App.js calls it after the plugin
 * script is loaded.
 */
(function () {
  "use strict";
  var JSZIP_URL = "/vendor/jszip.min.js";
  var VENDOR_URL = "/vendor/docx-preview.min.js";
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
    if (window.docx && typeof window.docx.renderAsync === "function") {
      return Promise.resolve(window.docx);
    }
    return loadScript(JSZIP_URL, function () { return !!window.JSZip; })
      .then(function () {
        return loadScript(VENDOR_URL, function () {
          return window.docx && typeof window.docx.renderAsync === "function";
        });
      })
      .then(function () { return window.docx; });
  }

  if (!window.notezViewers) window.notezViewers = {};
  window.notezViewers.docx = function (el, src) {
    return loadVendor().then(function (docx) {
      return fetch(src).then(function (r) {
        if (!r.ok) throw new Error("fetch " + r.status);
        return r.arrayBuffer();
      }).then(function (buf) {
        // docx-preview renders width-bound to the host element; CSS in
        // workspace.css keeps the host responsive.
        return docx.renderAsync(buf, el, undefined, {
          className: "preview-docx-render",
          inWrapper: false,
          ignoreWidth: true,
          ignoreHeight: true,
          breakPages: true
        });
      });
    });
  };
})();
