import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile, stat, mkdir } from 'node:fs/promises';
import { resolve, extname } from 'node:path';
import { pathToFileURL } from 'node:url';
import { gzipSync } from 'node:zlib';
import { siteDirectory, buildExampleTools, collectFormatExamples } from './format-examples.mjs';

const { chromium } = await import(process.env.PLAYWRIGHT_MODULE ? pathToFileURL(process.env.PLAYWRIGHT_MODULE).href : 'playwright');
const { formats } = await collectFormatExamples(buildExampleTools());
const screenshotDirectory = process.env.PLAYGROUND_SCREENSHOTS;
if (screenshotDirectory) await mkdir(screenshotDirectory, { recursive: true });
const dist = resolve(siteDirectory, 'dist');
let server, browser;
try {
  let origin = process.env.PLAYGROUND_URL;
  if (!origin) {
    server = createServer(async (req, res) => {
      try {
        const path = resolve(dist, '.' + decodeURIComponent(new URL(req.url, 'http://localhost').pathname));
        if (path !== dist && !path.startsWith(dist + '/')) { res.writeHead(403).end(); return; }
        const file = (await stat(path)).isDirectory() ? resolve(path, 'index.html') : path;
        const bytes = await readFile(file);
        const mime = { '.wasm': 'application/wasm', '.js': 'text/javascript', '.html': 'text/html', '.css': 'text/css', '.json': 'application/json', '.woff2': 'font/woff2', '.svg': 'image/svg+xml', '.png': 'image/png' }[extname(file)] ?? 'application/octet-stream';
        res.writeHead(200, { 'Content-Type': mime }); res.end(bytes);
      } catch { res.writeHead(404).end(); }
    });
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    origin = `http://127.0.0.1:${server.address().port}`;
  }
  browser = await chromium.launch({ channel: process.env.BROWSER_CHANNEL || undefined, headless: true });
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, reducedMotion: 'reduce' });
  const errors = [], requests = [];
  context.on('page', page => page.on('pageerror', error => errors.push(error.message)));
  context.on('request', request => requests.push({ url: request.url(), method: request.method(), body: request.postData() }));
  const page = await context.newPage();
  const spec = page.locator('#play-spec'), input = page.locator('#play-input');
  const result = page.locator('[data-result]');
  async function success() { await assertStatus('Success'); }
  async function assertStatus(text) { await page.waitForFunction(text => document.querySelector('[data-status]')?.textContent === text, text); }
  async function fill(source, text) { await spec.fill(source); await input.fill(text); await page.getByRole('button', { name: 'Run', exact: true }).click(); }
  async function view(name) { await page.getByRole('tab', { name, exact: true }).click(); }
  async function screenshot(path) {
    await page.evaluate(() => {
      if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
      for (const source of document.querySelectorAll('textarea')) { source.scrollTop = 0; source.scrollLeft = 0; source.dispatchEvent(new Event('scroll')); }
      window.scrollTo(0, 0);
    });
    await page.screenshot({ path, fullPage: true });
  }
  await page.goto(`${origin}/playground/`); await success();
  const wasmResponse = await context.request.get(requests.find(request => request.url.endsWith('.wasm')).url);
  assert.match(wasmResponse.headers()['content-type'], /application\/wasm/);
  assert.equal(await page.locator('main').count(), 1); assert.equal(await page.locator('h1').count(), 1);
  const integers = ['9007199254740993', '9223372036854775807', '-9223372036854775808'];
  await fill('a: int\nb: int\nc: int\n', integers.join(' ')); await success();
  for (const integer of integers) assert((await result.textContent()).includes(integer));
  for (const name of ['JSON', 'dzn']) {
    await view(name);
    for (const integer of integers) assert((await result.textContent()).includes(integer));
    const download = await page.locator('[data-download]').getAttribute('href');
    const text = await page.evaluate(async url => await (await fetch(url)).text(), download);
    for (const integer of integers) assert(text.includes(integer));
  }
  await fill('x:int', '7'); await success();
  await page.getByRole('button', { name: 'Format reader spec' }).click(); await success();
  assert.equal(await spec.inputValue(), 'x: int\n');
  await fill('x:missing', ''); await assertStatus('Spec error');
  await page.getByRole('button', { name: 'Format reader spec' }).click();
  await page.waitForFunction(() => document.querySelector('#play-spec').value === 'x: missing\n');
  await assertStatus('Spec error');
  await fill('x:', ''); await assertStatus('Spec error');
  await page.getByRole('button', { name: 'Format reader spec' }).click(); await assertStatus('Formatting failed');
  assert.equal(await spec.inputValue(), 'x:');
  await fill('x: int', 'bad'); await assertStatus('Read error');
  assert((await page.locator('[data-diagnostic-text]').textContent()).includes('^'));
  // Edited sources no longer claim to be the example files they were loaded from.
  assert.equal(await page.locator('[data-spec-origin]').textContent(), 'spec.iris');
  assert.equal(await page.locator('[data-input-origin]').textContent(), 'input.txt');
  assert(!(await page.locator('[data-diagnostic-text]').textContent()).includes('formats/'));
  assert.equal(await result.textContent(), 'No data. See the diagnostic below.');
  // Real normative budget failure remains separate from the host watchdog.
  await fill('repeat 1048577 { }', ''); await assertStatus('Read error');
  assert.equal(await page.locator('[data-error-code]').textContent(), 'E-BUDGET');
  // Latest edits win, including a format request superseded by typing.
  await spec.fill('x:int'); await page.getByRole('button', { name: 'Format reader spec' }).click();
  await spec.fill('latest: int'); await input.fill('9007199254740993'); await success();
  assert.equal(await spec.inputValue(), 'latest: int');
  assert((await result.textContent()).includes('9007199254740993'));
  await spec.focus(); await spec.press('End'); await spec.press('Enter');
  await spec.pressSequentially('# 🌸 blå'); await success();
  assert((await spec.inputValue()).endsWith('# 🌸 blå'));

  // Overlay losslessly follows native text, scroll, selection and forced colors.
  await fill('# 🌸\n' + 'x: int\n' + '# scrolling line\n'.repeat(40), '1'); await success();
  await page.waitForFunction(() => document.querySelector('.iris-editor').dataset.highlighted === 'true');
  assert.equal(await page.locator('.iris-editor-overlay code').textContent(), (await spec.inputValue()) + '\n');
  await spec.evaluate(node => { node.scrollTop = 200; node.dispatchEvent(new Event('scroll')); node.setSelectionRange(0, 4); });
  assert.equal(await spec.evaluate(node => node.scrollTop), await page.locator('.iris-editor-overlay').evaluate(node => node.scrollTop));
  await page.emulateMedia({ forcedColors: 'active' });
  assert.notEqual(await spec.evaluate(node => getComputedStyle(node).color), 'rgba(0, 0, 0, 0)');
  await page.emulateMedia({ forcedColors: 'none' });
  await spec.focus(); await spec.press('Tab'); assert.equal(await page.locator('#play-input').evaluate(node => node === document.activeElement), true);
  await view('Table'); const tableTab = page.getByRole('tab', { name: 'Table', exact: true }); await tableTab.focus(); await tableTab.press('ArrowRight');
  assert.equal(await page.getByRole('tab', { name: 'Table', exact: true }).getAttribute('aria-selected'), 'true');
  await page.keyboard.press('Enter'); assert.equal(await page.getByRole('tab', { name: 'JSON', exact: true }).getAttribute('aria-selected'), 'true');

  // Composition keeps native text visible and waits for compositionend.
  await spec.dispatchEvent('compositionstart');
  await spec.fill('# 🌸 IME\nx: int'); await input.fill('12');
  assert.equal(await page.locator('.iris-editor').getAttribute('data-highlighted'), null);
  await spec.dispatchEvent('compositionend'); await success();

  // A stalled worker drives the production five-second deadline without a giant fixture.
  let substituted = false;
  await context.route('**/_astro/worker-*.js', async route => {
    if (!substituted) { substituted = true; await route.fulfill({ contentType: 'text/javascript', body: 'self.onmessage = () => {}; self.postMessage({kind:"ready"});' }); }
    else await route.continue();
  });
  await page.reload(); await assertStatus('Reading…');
  await spec.fill('responsive: int'); await input.fill('42');
  assert.equal(await spec.inputValue(), 'responsive: int'); // typing while the stalled worker is in flight
  await success(); // edit terminated it and loaded the real replacement
  // Re-enter with one stalled worker, allow the actual watchdog to fire.
  substituted = false; await page.reload(); await assertStatus('Reading…');
  await view('JSON');
  await page.waitForFunction(() => document.querySelector('[data-status]')?.textContent === 'Timed out · reader ready', null, { timeout: 20000 });
  assert((await page.locator('[data-notice]').textContent()).includes('Timed out'));
  await fill('recovered: int', '43'); await success();
  substituted = false; await page.reload(); await assertStatus('Reading…');
  await page.getByRole('button', { name: 'Stop', exact: true }).click();
  await page.waitForFunction(() => document.querySelector('[data-status]')?.textContent === 'Stopped · reader ready');
  await fill('after_stop: int', '44'); await success();
  await context.unroute('**/_astro/worker-*.js');

  await context.route('**/_astro/worker-*.js', route => route.fulfill({ contentType: 'text/javascript', body: 'self.postMessage({kind:"failed", message:"Reader initialization failed"});' }));
  await page.reload(); await assertStatus('Reader unavailable');
  assert(await spec.isEditable());
  await context.unroute('**/_astro/worker-*.js');
  await page.getByRole('button', { name: 'Retry / Run', exact: true }).click(); await success();

  // Shared links retain Unicode, exact parameter strings, instance and view.
  const sharedSpec = '# 🌸 blå\nparam p = 0\ninstances { name: word\nx: int }\n';
  const sharedInput = 'blå 9007199254740993\nröd 9223372036854775807';
  await spec.fill(sharedSpec); await input.fill(sharedInput);
  await page.getByRole('button', { name: 'Add parameter', exact: true }).click();
  await page.getByLabel('Parameter name', { exact: true }).fill('p');
  await page.getByLabel('Parameter value', { exact: true }).fill('9007199254740993');
  await page.getByLabel('Instance (number or name)', { exact: true }).fill('blå');
  await success(); await view('JSON');
  await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin });
  await page.getByRole('button', { name: 'Copy link', exact: true }).click();
  await page.waitForFunction(() => !document.querySelector('[data-share-field]').hidden);
  const sharedUrl = await page.locator('#play-share-url').inputValue(); assert(sharedUrl.includes('#v1='));
  await page.waitForFunction(() => document.querySelector('[data-notice]').textContent.startsWith('Link copied.'));
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()), sharedUrl);
  const fresh = await browser.newContext({ viewport: { width: 390, height: 844 } });
  const restored = await fresh.newPage(); await restored.goto(sharedUrl);
  await restored.waitForFunction(() => document.querySelector('[data-status]')?.textContent === 'Success');
  assert.equal(await restored.locator('#play-spec').inputValue(), sharedSpec); assert.equal(await restored.locator('#play-input').inputValue(), sharedInput);
  assert.equal(await restored.getByLabel('Parameter value', { exact: true }).inputValue(), '9007199254740993');
  assert.equal(await restored.locator('#play-instance').inputValue(), 'blå');
  assert.equal(await restored.getByRole('tab', { name: 'JSON', exact: true }).getAttribute('aria-selected'), 'true');
  assert((await restored.locator('[data-result]').textContent()).includes('9007199254740993'));
  await fresh.close();
  await page.locator('#play-instance').fill('2'); await success();
  await page.evaluate(() => { navigator.clipboard.writeText = async () => { throw new DOMException('Clipboard denied', 'NotAllowedError'); }; });
  assert((await result.textContent()).includes('9223372036854775807'));
  await page.getByRole('button', { name: 'Copy link', exact: true }).click();
  await page.waitForFunction(previous => location.href !== previous, sharedUrl);
  await page.waitForFunction(() => document.querySelector('[data-notice]').textContent.startsWith('Link ready.'));
  await page.goBack(); await page.waitForFunction(() => document.querySelector('#play-instance').value === 'blå'); await success(); assert.equal(await instanceValue(page), 'blå');
  await page.goForward(); await page.waitForFunction(() => document.querySelector('#play-instance').value === '2'); await success(); assert.equal(await instanceValue(page), '2');
  // A parameter value with a line break is valid state, but the single-line fields cannot hold it.
  const multiline = { language: '0.1', spec: 'param p = ""\nlet result = p\n', input: '', specOrigin: 'spec.iris', inputOrigin: 'input.txt', params: [{ name: 'p', value: 'first\nsecond' }], view: 'table' };
  const multilineHash = '#v1=' + gzipSync(Buffer.from(JSON.stringify(multiline))).toString('base64url');
  for (const [hash, message] of [[multilineHash, 'must not contain line breaks'], ['#v1=broken', 'Cannot restore'], ['#v2=abcd', 'Unknown shared-state version'], ['#v1=' + 'a'.repeat(16400), 'Shared URL exceeds']]) {
    const before = await spec.inputValue(); await page.evaluate(hash => { location.hash = hash; }, hash);
    await page.waitForFunction(([hash, message]) => location.hash === hash && document.querySelector('[data-notice]').textContent.includes(message), [hash, message]);
    assert.equal(await spec.inputValue(), before);
  }

  // Every preset fetches current complete sources, including failure/named cases.
  for (const format of formats) for (const example of format.cases) {
    await page.goto(`${origin}/playground/?example=${format.id}/${example.id}`);
    await assertStatus(example.result.kind === 'success' ? 'Success' : 'Read error');
    assert.equal(await spec.inputValue(), format.spec.text); assert.equal(await input.inputValue(), example.input.text);
    assert.equal(await page.locator('[data-spec-origin]').textContent(), format.spec.path);
    assert.equal(await instanceValue(page), example.params.find(pair => pair.name === 'instance')?.value ?? '');
  }
  for (const path of ['/', ...formats.map(format => `/formats/${format.slug}/`)]) {
    await page.goto(origin + path);
    const expected = path === '/' ? formats.filter(format => format.homeCase).length : formats.find(format => path === `/formats/${format.slug}/`).cases.length;
    assert.equal(await page.locator('[data-playground-link]').count(), expected);
  }
  await page.goto(`${origin}/playground/`); await success();
  for (const width of [1440, 390, 320]) for (const theme of ['light', 'dark']) {
    await page.setViewportSize({ width, height: width === 1440 ? 1000 : 844 });
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; localStorage.setItem('starlight-theme', theme); }, theme);
    await page.evaluate(() => document.fonts.ready);
    await fill('a: int\nb: int\nrepeat 2 { list x: int until 0 }', '9007199254740993 9223372036854775807 1 2 0 3 0'); await success(); await view('Table');
    const boxes = await page.locator('.iris-play-editors > div').evaluateAll(nodes => nodes.map(node => { const r = node.getBoundingClientRect(); return { x: r.x, y: r.y }; }));
    assert.equal(boxes[0].y === boxes[1].y, width === 1440);
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, `overflow ${width}/${theme}/table`);
    if (screenshotDirectory && width !== 320) await screenshot(`${screenshotDirectory}/playground-table-${width}-${theme}.png`);
    await view('JSON');
    if (screenshotDirectory && width !== 320) await screenshot(`${screenshotDirectory}/playground-json-${width}-${theme}.png`);
    await fill('x: int', 'bad'); await assertStatus('Read error');
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, `overflow ${width}/${theme}/error`);
    if (screenshotDirectory && width !== 320) await screenshot(`${screenshotDirectory}/playground-error-${width}-${theme}.png`);
  }
  const noJs = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 390, height: 844 } });
  const staticPage = await noJs.newPage(); await staticPage.goto(`${origin}/playground/`);
  assert((await staticPage.locator('noscript').textContent()).includes('requires JavaScript and WebAssembly'));
  assert(await staticPage.locator('#play-spec').inputValue());
  assert(await staticPage.locator('[data-run]').isDisabled()); await noJs.close();
  const noCompression = await browser.newContext();
  await noCompression.addInitScript(() => { globalThis.CompressionStream = undefined; globalThis.DecompressionStream = undefined; });
  const unsupported = await noCompression.newPage(); await unsupported.goto(`${origin}/playground/`);
  await unsupported.waitForFunction(() => document.querySelector('[data-status]')?.textContent === 'Success');
  await unsupported.getByRole('button', { name: 'Copy link', exact: true }).click();
  await unsupported.waitForFunction(() => document.querySelector('[data-notice]').textContent.includes('Compression Streams'));
  await noCompression.close();
  assert.deepEqual(errors, []);
  assert(requests.every(request => request.method === 'GET' && !request.body), 'Sources were sent to a server');
  console.log(`Browser acceptance passed: exact integers; latest edits/typing; Format; check/read carets; budget/Stop/timeout recovery; sharing/history; ${formats.reduce((n, f) => n + f.cases.length, 0)} presets; keyboard/overlay; 1440/390/320 light/dark; no JS. WASM MIME application/wasm. ${screenshotDirectory ? `Screenshots: ${screenshotDirectory}` : ''}`);
  await context.close();
} finally { await browser?.close(); if (server) await new Promise(resolve => server.close(resolve)); }
async function instanceValue(page) { return page.locator('#play-instance').inputValue(); }
