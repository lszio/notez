// Client-side hydration for `<div class="d2-block" data-source="…">`.
//
// The official `@terrastruct/d2` bundle is heavy; we lazy-load it from
// esm.sh only when a `d2-block` is actually present in the document.
async function renderAll() {
  const blocks = document.querySelectorAll('div.d2-block');
  if (blocks.length === 0) return;

  let render;
  try {
    ({ render } = await import('https://esm.sh/@terrastruct/d2@0'));
  } catch (err) {
    for (const block of blocks) {
      block.innerHTML = '<pre class="d2-error">' +
        String(err).replace(/[<>&]/g, (c) =>
          c === '<' ? '&lt;' : c === '>' ? '&gt;' : '&amp;') +
        '</pre>';
    }
    return;
  }

  for (const block of blocks) {
    const source = block.getAttribute('data-source') ?? '';
    try {
      await render(block, source);
    } catch (err) {
      block.innerHTML = '<pre class="d2-error">' +
        String(err).replace(/[<>&]/g, (c) =>
          c === '<' ? '&lt;' : c === '>' ? '&gt;' : '&amp;') +
        '</pre>';
    }
  }
}

if (document.readyState === 'loading') {
  window.addEventListener('DOMContentLoaded', renderAll, { once: true });
} else {
  renderAll();
}