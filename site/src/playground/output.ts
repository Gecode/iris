import type { ReadResult, TableEntry, View } from './protocol';
function element<K extends keyof HTMLElementTagNameMap>(tag: K, text?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (text !== undefined) node.textContent = text;
  return node;
}
function table(headers: string[], rows: string[][]): HTMLElement {
  const scroll = element('div'); scroll.className = 'iris-table-scroll';
  const table = element('table');
  const head = element('thead'); const tr = element('tr');
  for (const label of headers) { const th = element('th', label); th.scope = 'col'; tr.append(th); }
  head.append(tr); table.append(head);
  const body = element('tbody');
  for (const cells of rows) { const row = element('tr'); for (const cell of cells) row.append(element('td', cell)); body.append(row); }
  table.append(body); scroll.append(table); return scroll;
}
function scalar(entry: TableEntry, text: string): string { return entry.type === 'string' ? JSON.stringify(text) : text; }
function tablePreview(result: Extract<ReadResult, { kind: 'success' }>): DocumentFragment {
  const fragment = document.createDocumentFragment();
  const scalars = result.table.entries.filter(entry => entry.depth === 0);
  if (scalars.length) fragment.append(table(['Name', 'Type', 'Value'], scalars.map(entry => {
    const row = entry.rows[0];
    return [entry.name, entry.type, row?.kind === 'scalar' ? scalar(entry, row.text) + (row.clipped ? ' … (clipped)' : '') : '(omitted)'];
  })));
  for (const entry of result.table.entries.filter(entry => entry.depth > 0)) {
    const details = element('details'); details.open = true; details.className = 'mt-4 min-w-0';
    details.append(element('summary', `${entry.name}: ${entry.type}, depth ${entry.depth}`));
    details.append(table(['Path', 'Value'], entry.rows.map(row => [JSON.stringify(row.path), row.kind === 'array' ? `Array · ${row.length} ${row.length === 1 ? 'element' : 'elements'}` : scalar(entry, row.text) + (row.clipped ? ' … (clipped)' : '')])));
    if (entry.truncated) details.append(element('p', 'Preview truncated; use JSON or dzn for complete data.'));
    fragment.append(details);
  }
  if (result.table.truncated) fragment.append(element('p', 'Table preview is limited to 64 entries, 1,024 nodes, depth 16 and 64 KiB of text. Complete output is available in the text views.'));
  if (!result.table.entries.length) fragment.append(element('p', 'The reader produced no entries.'));
  return fragment;
}
export function outputRenderer(panel: HTMLElement, actions: HTMLElement, download: HTMLAnchorElement, excerpt: HTMLElement) {
  let downloadUrl: string | undefined;
  let currentText = '';
  function dispose() { if (downloadUrl) URL.revokeObjectURL(downloadUrl); downloadUrl = undefined; }
  return {
    dispose,
    text: () => currentText,
    clear() { dispose(); currentText = ''; panel.replaceChildren(); actions.hidden = true; },
    render(result: Extract<ReadResult, { kind: 'success' }>, view: View) {
      dispose(); actions.hidden = view === 'table'; currentText = '';
      if (view === 'table') { panel.replaceChildren(tablePreview(result)); return; }
      currentText = result[view];
      const bytes = new TextEncoder().encode(currentText);
      let visible = currentText;
      excerpt.hidden = bytes.length <= 32 << 10;
      if (!excerpt.hidden) visible = new TextDecoder().decode(bytes.subarray(0, 32 << 10), { stream: true });
      const pre = element('pre'); pre.tabIndex = 0; pre.className = 'iris-play-code'; pre.append(element('code', visible)); panel.replaceChildren(pre);
      downloadUrl = URL.createObjectURL(new Blob([currentText], { type: 'text/plain;charset=utf-8' }));
      download.href = downloadUrl; download.download = view === 'summary' ? 'summary.txt' : `data.${view}`;
    },
  };
}
