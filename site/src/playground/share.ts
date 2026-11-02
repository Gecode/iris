import type { ReadState, Parameter, View } from './protocol.ts';
export const URL_LIMIT = 16384;
export const STATE_LIMIT = 256 << 10;
const encoder = new TextEncoder();
const views: View[] = ['table', 'json', 'dzn', 'summary'];
function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
function text(value: unknown, limit: number): value is string {
  return typeof value === 'string' && encoder.encode(value).length <= limit;
}
export function validateState(value: unknown): ReadState {
  if (!record(value) || value.language !== '0.1' || !text(value.spec, 1 << 20) || !text(value.input, 1 << 20)
    || !text(value.specOrigin, 256) || !text(value.inputOrigin, 256) || !Array.isArray(value.params)
    || value.params.length > 1024 || !views.some(view => view === value.view)) throw new Error('Invalid playground state or language version.');
  const params: Parameter[] = [];
  let bytes = 0;
  for (const pair of value.params) {
    if (!record(pair) || !text(pair.name, 64 << 10) || !text(pair.value, 64 << 10)) throw new Error('Parameters must be name/value strings.');
    // The parameter fields are single-line inputs, which would silently drop line breaks.
    if (/[\r\n]/.test(pair.name + pair.value)) throw new Error('Parameter names and values must not contain line breaks.');
    bytes += encoder.encode(pair.name).length + encoder.encode(pair.value).length;
    params.push({ name: pair.name, value: pair.value });
  }
  if (bytes > 64 << 10) throw new Error('Parameters exceed 64 KiB.');
  const view = views.find(view => view === value.view);
  if (!view) throw new Error('Unknown output view.');
  return { language: '0.1', spec: value.spec, input: value.input, specOrigin: value.specOrigin, inputOrigin: value.inputOrigin, params, view };
}
async function collect(stream: ReadableStream<Uint8Array>, limit: number): Promise<Uint8Array> {
  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  let length = 0;
  try {
    for (;;) {
      const chunk = await reader.read();
      if (chunk.done) break;
      length += chunk.value.length;
      if (length > limit) throw new Error('Shared state exceeds 256 KiB.');
      chunks.push(chunk.value);
    }
  } finally { await reader.cancel(); reader.releaseLock(); }
  const bytes = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
  return bytes;
}
export async function encodeState(state: ReadState): Promise<string> {
  validateState(state);
  if (typeof CompressionStream === 'undefined') throw new Error('Sharing requires browser Compression Streams support.');
  const bytes = encoder.encode(JSON.stringify(state));
  if (bytes.length > STATE_LIMIT) throw new Error('Shared state exceeds 256 KiB.');
  const compressed = await collect(new Blob([bytes]).stream().pipeThrough(new CompressionStream('gzip')), STATE_LIMIT);
  let binary = '';
  for (const byte of compressed) binary += String.fromCharCode(byte);
  const hash = '#v1=' + btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, '');
  if (hash.length > URL_LIMIT) throw new Error('Shared URL exceeds 16,384 characters.');
  return hash;
}
export async function decodeState(hash: string): Promise<ReadState> {
  if (hash.length > URL_LIMIT) throw new Error('Shared URL exceeds 16,384 characters.');
  if (!hash.startsWith('#v1=')) throw new Error('Unknown shared-state version.');
  const encoded = hash.slice(4);
  if (!/^[\w-]+$/.test(encoded) || encoded.length % 4 === 1) throw new Error('Invalid shared-state encoding.');
  if (typeof DecompressionStream === 'undefined') throw new Error('Restoring links requires browser Compression Streams support.');
  try {
    const binary = atob(encoded.replaceAll('-', '+').replaceAll('_', '/'));
    const bytes = Uint8Array.from(binary, ch => ch.charCodeAt(0));
    const inflated = await collect(new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip')), STATE_LIMIT);
    return validateState(JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(inflated)));
  } catch (error) { throw new Error(`Cannot restore shared state: ${error instanceof Error ? error.message : 'corrupt data'}`); }
}
