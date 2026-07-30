// Client-side hydration for `<div class="mermaid-block" data-source="…">`.
//
// Renders each block via the official mermaid ESM build served from esm.sh
// (no Node build chain required).
import mermaid from 'https://esm.sh/mermaid@10/dist/mermaid.esm.min.mjs';

mermaid.initialize({ startOnLoad: false });

async function renderAll() {
  const blocks = document.querySelectorAll('div.mermaid-block');
  for (const block of blocks) {
    const source = block.getAttribute('data-source') ?? '';
    const id = 'm-' + Math.random().toString(36).slice(2);
    try {
      const { svg } = await mermaid.render(id, source);
      block.innerHTML = svg;
    } catch (err) {
      block.innerHTML = '<pre class="mermaid-error">' +
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