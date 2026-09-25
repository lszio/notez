/* notez attachment viewer plugin: DOCX.
 *
 * Renders a `.docx` attachment inline by fetching the raw bytes (same
 * origin) and feeding them to the vendored `docx-preview` library.
 * The library's UMD dist exposes the `docx` global; this plugin loads
 * that script on demand so a notez instance without the plugin does
 * not pay the size cost.
 *
 * Contract: register an async `(el, src) => void` renderer on
 * `window.notezViewers["docx"]`. App.js calls it after the plugin
 * script is loaded.
 */
(function () {
  "use strict";
  var VENDOR_URL = "/vendor/docx-preview.min.js";
  function loadVendor() {
    if (window.docx && typeof window.docx.renderAsync === "function") {
      return Promise.resolve(window.docx);
    }
    return new Promise(function (resolve, reject) {
      var s = document.createElement("script");
      s.src = VENDOR_URL;
      s.onload = function () {
        if (window.docx && typeof window.docx.renderAsync === "function") {
          resolve(window.docx);
        } else {
          reject(new Error("docx vendor script missing renderAsync"));
        }
      };
      s.onerror = function () { reject(new Error("failed to load " + VENDOR_URL)); };
      document.head.appendChild(s);
    });
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
