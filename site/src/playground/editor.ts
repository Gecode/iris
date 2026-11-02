import type { ColorRun } from './protocol';
/** Native editing owns selection, undo and IME. Colours are disposable presentation. */
export function highlightEditor(textarea: HTMLTextAreaElement, overlay: HTMLElement) {
  const wrapper = textarea.parentElement;
  function clear() { wrapper?.removeAttribute('data-highlighted'); }
  function syncScroll() { const viewport = overlay.parentElement; if (viewport) { viewport.scrollTop = textarea.scrollTop; viewport.scrollLeft = textarea.scrollLeft; } }
  textarea.addEventListener('scroll', syncScroll);
  textarea.addEventListener('input', clear);
  textarea.addEventListener('compositionstart', clear);
  return {
    clear,
    apply(source: string, runs: ColorRun[]) {
      clear();
      if (source !== textarea.value || runs.length === 0) return;
      const fragment = document.createDocumentFragment();
      let offset = 0;
      for (const run of runs) {
        if (run.start !== offset || run.end < run.start || run.end > source.length) return;
        const span = document.createElement('span');
        span.dataset.color = run.kind;
        span.textContent = source.slice(run.start, run.end);
        fragment.append(span);
        offset = run.end;
      }
      if (offset !== source.length) return;
      // A pre otherwise drops the final empty line, unlike a native textarea.
      if (source.endsWith('\n')) fragment.append(document.createTextNode('\n'));
      overlay.replaceChildren(fragment);
      wrapper?.setAttribute('data-highlighted', 'true');
      syncScroll();
    },
  };
}
