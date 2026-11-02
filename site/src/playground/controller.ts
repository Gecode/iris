import type { ReadState, ReadResult, CheckResult, View, Response, Request, Parameter } from './protocol';
import { validateState, encodeState, decodeState, URL_LIMIT } from './share';
import { highlightEditor } from './editor';
import { outputRenderer } from './output';

export function mountPlayground(root: HTMLElement) {
  function get<T extends HTMLElement>(selector: string): T {
    const node = root.querySelector<T>(selector);
    if (!node) throw new Error(`Missing playground control ${selector}`);
    return node;
  }
  const spec = get<HTMLTextAreaElement>('#play-spec');
  const input = get<HTMLTextAreaElement>('#play-input');
  const instance = get<HTMLInputElement>('#play-instance');
  const parameters = get('[data-parameters]');
  const status = get('[data-status]');
  const notice = get('[data-notice]');
  const run = get<HTMLButtonElement>('[data-run]');
  const stop = get<HTMLButtonElement>('[data-stop]');
  const format = get<HTMLButtonElement>('[data-format]');
  const share = get<HTMLButtonElement>('[data-share]');
  const output = get('[data-output]');
  const panel = get('[data-result]');
  const stale = get('[data-stale]');
  const diagnostic = get('[data-diagnostic]');
  const focusError = get<HTMLButtonElement>('[data-focus-error]');
  const tabs = [...root.querySelectorAll<HTMLButtonElement>('[data-view]')];
  const highlight = highlightEditor(spec, get('.iris-editor-overlay code'));
  const renderer = outputRenderer(panel, get('[data-text-actions]'), get<HTMLAnchorElement>('[data-download]'), get('[data-excerpt]'));
  const initial = validateState(JSON.parse(root.dataset.initial ?? '{}'));
  let specOrigin = initial.specOrigin;
  let inputOrigin = initial.inputOrigin;
  let view: View = initial.view;
  let revision = 0;
  let generation = 0;
  let nextId = 0;
  let active: { id: number; revision: number; source: string } | undefined;
  let worker: Worker | undefined;
  let ready = false;
  let composing = false;
  let pending: 'evaluate' | 'format' | undefined;
  let debounce: ReturnType<typeof setTimeout> | undefined;
  let deadline: ReturnType<typeof setTimeout> | undefined;
  let result: Extract<ReadResult, { kind: 'success' }> | undefined;
  let errorRange: { start: number; end: number } | undefined;
  let disposed = false;
  let parameterId = 0;
  let restoreId = 0;

  function say(message: string) { status.textContent = message; }
  function inform(message: string) { notice.textContent = message; notice.hidden = !message; }
  function buttons() {
    run.disabled = Boolean(worker) && !ready;
    run.textContent = worker ? 'Run' : 'Retry / Run';
    format.disabled = !ready;
    stop.disabled = !active && !pending;
    share.disabled = false;
  }
  function terminate() {
    clearTimeout(deadline); deadline = undefined;
    worker?.terminate(); worker = undefined; ready = false; active = undefined;
    generation++;
    output.setAttribute('aria-busy', 'false');
    buttons();
  }
  function clearResult() { result = undefined; renderer.clear(); stale.hidden = true; }
  function showDiagnostic(title: string, failure: Exclude<CheckResult, { kind: 'checked' }> | { kind: 'formatError'; diagnostic: string }) {
    clearResult(); diagnostic.hidden = false;
    panel.replaceChildren(Object.assign(document.createElement('p'), { className: 'm-0 text-sm text-iris-muted', textContent: 'No data. See the diagnostic below.' }));
    get('#play-diagnostic-title').textContent = title;
    get('[data-diagnostic-text]').textContent = 'diagnostic' in failure ? failure.diagnostic : failure.message;
    const code = get<HTMLAnchorElement>('[data-error-code]');
    code.textContent = failure.kind === 'language' ? failure.code : '';
    if (failure.kind === 'language') code.href = `/specification/0.1/errors/#${failure.code.toLowerCase()}`;
    else code.removeAttribute('href');
    errorRange = failure.kind === 'language' ? failure.range : undefined;
    focusError.hidden = !errorRange;
  }
  function finish() { clearTimeout(deadline); active = undefined; output.setAttribute('aria-busy', 'false'); buttons(); }
  function unavailable(message: string) {
    pending = undefined; terminate();
    say('Reader unavailable'); inform(`${message} Your sources are preserved. Retry / Run, use the CLI, or browse the format library.`);
  }
  function createWorker(afterReady = 'Ready') {
    if (worker || disposed) return;
    const currentGeneration = ++generation;
    say('Loading reader…');
    try { worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' }); }
    catch (error) { unavailable(String(error)); return; }
    const currentWorker = worker;
    buttons();
    deadline = setTimeout(() => { if (worker === currentWorker) unavailable('Reader startup exceeded 15 seconds.'); }, 15000);
    worker.onerror = event => { event.preventDefault(); if (worker === currentWorker) unavailable(event.message || 'The reader worker failed.'); };
    worker.onmessageerror = () => { if (worker === currentWorker) unavailable('The reader worker returned an unreadable message.'); };
    worker.onmessage = (event: MessageEvent<Response>) => {
      if (worker !== currentWorker || generation !== currentGeneration) return;
      const message = event.data;
      if (message.kind === 'failed') { unavailable(message.message); return; }
      if (message.kind === 'ready') {
        clearTimeout(deadline); ready = true; say(afterReady); buttons();
        if (pending) execute(pending);
        return;
      }
      if (!active || message.generation !== generation || message.id !== active.id || active.revision !== revision) return;
      if (message.kind === 'checked') {
        if ('colors' in message.result && message.result.colors) highlight.apply(active.source, message.result.colors);
        if (message.result.kind !== 'checked') { showDiagnostic('Spec diagnostics', message.result); say('Spec error'); finish(); }
        return;
      }
      if (message.kind === 'formatted') {
        if (message.result.kind === 'formatted') {
          if (spec.value !== message.result.spec) markEdited('spec');
          spec.value = message.result.spec; highlight.clear(); revision++;
          finish(); execute('evaluate');
        } else { showDiagnostic('Spec diagnostics', message.result); say('Formatting failed'); finish(); }
        return;
      }
      if (message.result.kind === 'success') {
        result = message.result; diagnostic.hidden = true; stale.hidden = true;
        renderer.render(result, view); say('Success');
      } else { showDiagnostic('Read error', message.result); say('Read error'); }
      finish();
    };
  }
  function draft(): ReadState {
    const pairs: Parameter[] = [...parameters.querySelectorAll<HTMLElement>('[data-parameter]')].map(row => {
      const name = row.querySelector<HTMLInputElement>('[data-name]')?.value ?? '';
      const value = row.querySelector<HTMLInputElement>('[data-value]')?.value ?? '';
      if (!name) throw new Error('Enter a name for each parameter, or remove the empty row.');
      if (name === 'instance') throw new Error('Use the Instance field for the reserved instance parameter.');
      return { name, value };
    });
    if (instance.value) pairs.push({ name: 'instance', value: instance.value });
    return validateState({ language: '0.1', spec: spec.value, input: input.value, specOrigin, inputOrigin, params: pairs, view });
  }
  function execute(kind: 'evaluate' | 'format') {
    clearTimeout(debounce); debounce = undefined;
    if (composing || disposed) return;
    let state: ReadState;
    try {
      state = kind === 'evaluate' ? draft() : validateState({ language: '0.1', spec: spec.value, input: '', specOrigin, inputOrigin, params: [], view });
    } catch (error) {
      pending = undefined;
      if (active) terminate();
      inform(error instanceof Error ? error.message : String(error));
      if (!worker) createWorker('Check sources and parameters');
      else say('Check sources and parameters');
      buttons(); return;
    }
    if (active) terminate();
    pending = kind;
    if (!worker) createWorker();
    if (!ready || !worker) return;
    pending = undefined;
    inform(''); diagnostic.hidden = true;
    const id = ++nextId;
    active = { id, revision, source: state.spec };
    const request: Request = kind === 'evaluate'
      ? { kind, generation, id, state }
      : { kind, generation, id, spec: state.spec, specOrigin: state.specOrigin };
    output.setAttribute('aria-busy', 'true'); stale.hidden = !result;
    say(kind === 'evaluate' ? 'Reading…' : 'Formatting…'); buttons();
    deadline = setTimeout(() => {
      pending = undefined; terminate();
      inform('Timed out after 5 seconds. Edit the sources or press Run to try again.');
      createWorker('Timed out · reader ready');
    }, 5000);
    worker.postMessage(request);
  }
  // Once edited, a source is no longer the example file it was loaded from.
  function markEdited(source: 'spec' | 'input') {
    if (source === 'spec') { specOrigin = 'spec.iris'; get('[data-spec-origin]').textContent = specOrigin; }
    else { inputOrigin = 'input.txt'; get('[data-input-origin]').textContent = inputOrigin; }
  }
  function changed() {
    revision++; highlight.clear();
    clearTimeout(debounce); pending = undefined;
    if (active) terminate();
    stale.hidden = !result;
    if (composing) return;
    say('Waiting for edits…');
    debounce = setTimeout(() => execute('evaluate'), 200);
  }
  function addParameter(pair: Parameter = { name: '', value: '' }) {
    const row = document.createElement('div'); row.dataset.parameter = ''; row.className = 'iris-param-row';
    const id = ++parameterId;
    for (const key of ['name', 'value'] as const) {
      const field = document.createElement('div'); field.className = 'min-w-0';
      const label = document.createElement('label'); label.htmlFor = `play-param-${id}-${key}`; label.textContent = key === 'name' ? 'Parameter name' : 'Parameter value'; label.className = 'block text-sm';
      const control = document.createElement('input'); control.id = label.htmlFor; control.type = 'text'; control.dataset[key] = ''; control.value = pair[key]; control.spellcheck = false; control.className = 'iris-play-input w-full';
      field.append(label, control); row.append(field);
    }
    const remove = document.createElement('button'); remove.type = 'button'; remove.className = 'iris-play-button'; remove.textContent = 'Remove parameter';
    remove.addEventListener('click', () => { row.remove(); changed(); });
    row.append(remove); parameters.append(row);
    return row;
  }
  function selectView(next: View) {
    view = next;
    for (const tab of tabs) { const selected = tab.dataset.view === next; tab.setAttribute('aria-selected', String(selected)); tab.tabIndex = selected ? 0 : -1; }
    panel.setAttribute('aria-labelledby', `play-tab-${next}`);
    if (result) renderer.render(result, view);
  }
  function apply(state: ReadState) {
    clearTimeout(debounce); pending = undefined; terminate(); revision++;
    spec.value = state.spec; input.value = state.input; specOrigin = state.specOrigin; inputOrigin = state.inputOrigin;
    get('[data-spec-origin]').textContent = specOrigin; get('[data-input-origin]').textContent = inputOrigin;
    parameters.replaceChildren(); instance.value = '';
    for (const pair of state.params) { if (pair.name === 'instance') instance.value = pair.value; else addParameter(pair); }
    highlight.clear(); selectView(state.view); clearResult(); diagnostic.hidden = true; inform('');
  }
  async function restore() {
    const id = ++restoreId;
    const atRevision = revision;
    const url = new URL(location.href);
    try {
      let state: ReadState;
      if (url.hash) {
        if (url.href.length > URL_LIMIT) throw new Error('Shared URL exceeds 16,384 characters.');
        state = await decodeState(url.hash);
      } else if (url.searchParams.has('example')) {
        const key = url.searchParams.get('example') ?? '';
        if (!/^[a-z0-9]+(?:-[a-z0-9]+)*\/[a-z0-9]+(?:-[a-z0-9]+)*$/.test(key)) throw new Error('Invalid example selector.');
        const response = await fetch(`/examples/formats/${key.split('/')[0]}/cases/${key.split('/')[1]}/case.json`);
        if (!response.ok) throw new Error('Example could not be loaded.');
        const bundle = await response.json();
        state = validateState({ language: '0.1', spec: bundle.spec?.text, input: bundle.input?.text, specOrigin: bundle.spec?.path, inputOrigin: bundle.input?.path, params: bundle.params, view: 'table' });
      } else state = initial;
      if (id !== restoreId || revision !== atRevision || location.href !== url.href || disposed) return;
      apply(state); execute('evaluate');
    } catch (error) {
      if (id !== restoreId || revision !== atRevision) return;
      if (!worker) createWorker();
      inform(error instanceof Error ? error.message : String(error));
    }
  }
  run.addEventListener('click', () => execute('evaluate'));
  stop.addEventListener('click', () => { clearTimeout(debounce); pending = undefined; terminate(); stale.hidden = !result; createWorker('Stopped · reader ready'); });
  format.addEventListener('click', () => execute('format'));
  share.addEventListener('click', async () => {
    const atRevision = revision;
    try {
      const state = draft(); const hash = await encodeState(state);
      if (revision !== atRevision || disposed) { inform('Sources changed while creating the link. Press Copy link again.'); return; }
      const url = new URL(location.href); url.searchParams.delete('example'); url.hash = hash;
      if (url.href.length > URL_LIMIT) throw new Error('Shared URL exceeds 16,384 characters.');
      history.pushState(null, '', url);
      const field = get<HTMLInputElement>('#play-share-url'); field.value = url.href; get('[data-share-field]').hidden = false;
      try { await navigator.clipboard.writeText(url.href); inform('Link copied. It contains your reader spec, input and parameters.'); }
      catch { inform('Link ready. Copy it from the Shared link field.'); }
    } catch (error) { inform(error instanceof Error ? error.message : String(error)); }
  });
  get('[data-copy-output]').addEventListener('click', async () => { try { await navigator.clipboard.writeText(renderer.text()); inform('Output copied exactly.'); } catch { inform('Clipboard unavailable. Download the complete output.'); } });
  get('[data-add-param]').addEventListener('click', () => { const row = addParameter(); row.querySelector('input')?.focus(); changed(); });
  focusError.addEventListener('click', () => { if (errorRange) { spec.focus(); spec.setSelectionRange(errorRange.start, errorRange.end); } });
  spec.addEventListener('input', () => markEdited('spec'));
  input.addEventListener('input', () => markEdited('input'));
  for (const source of [spec, input, parameters, instance]) source.addEventListener('input', changed);
  for (const source of [spec, input, parameters, instance]) {
    source.addEventListener('compositionstart', () => { composing = true; changed(); });
    source.addEventListener('compositionend', () => { composing = false; changed(); });
  }
  tabs.forEach((tab, index) => {
    tab.addEventListener('click', () => { const next = tab.dataset.view; if (next === 'table' || next === 'json' || next === 'dzn' || next === 'summary') { revision++; if (active) active.revision = revision; selectView(next); } });
    tab.addEventListener('keydown', event => {
      let next: number;
      switch (event.key) {
        case 'ArrowRight': next = (index + 1) % tabs.length; break;
        case 'ArrowLeft': next = (index + tabs.length - 1) % tabs.length; break;
        case 'Home': next = 0; break;
        case 'End': next = tabs.length - 1; break;
        default: return;
      }
      event.preventDefault(); tabs.forEach((item, i) => { item.tabIndex = i === next ? 0 : -1; }); tabs[next]?.focus();
    });
  });
  function dispose() { disposed = true; clearTimeout(debounce); pending = undefined; terminate(); renderer.dispose(); window.removeEventListener('hashchange', restore); window.removeEventListener('popstate', restore); }
  window.addEventListener('pagehide', dispose);
  window.addEventListener('pageshow', event => {
    if (!event.persisted) return;
    disposed = false;
    window.addEventListener('hashchange', restore); window.addEventListener('popstate', restore);
    if (result) renderer.render(result, view);
    execute('evaluate');
  });
  window.addEventListener('hashchange', restore); window.addEventListener('popstate', restore);
  apply(initial); buttons(); void restore();
}
