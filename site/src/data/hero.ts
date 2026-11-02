export interface ByteRange { start: number; end: number; text: string }
export interface DataTarget { entry: string; path: readonly number[] }
export interface HeroRelation {
  id: string;
  statement: 'comment' | 'stop' | 'header' | 'clause';
  input: ByteRange | null;
  spec: ByteRange;
  data: readonly DataTarget[];
  label: string;
}
const list = { start: 147, end: 179, text: 'list clause: -vars..vars until 0' };
export const heroRelations: readonly HeroRelation[] = [
  { id: 'comment', statement: 'comment', input: { start: 0, end: 34, text: 'c Hand-made three-variable formula' }, spec: { start: 19, end: 30, text: 'comment "c"' }, data: [], label: 'Comment skipped; no data stored.' },
  { id: 'stop', statement: 'stop', input: null, spec: { start: 31, end: 39, text: 'stop "%"' }, data: [], label: 'No stop marker in this input.' },
  { id: 'header', statement: 'header', input: { start: 35, end: 44, text: 'p cnf 3 3' }, spec: { start: 76, end: 127, text: 'line "p cnf {vars:0..gecode.int_max} {clauses:0..}"' }, data: [{ entry: 'vars', path: [] }, { entry: 'clauses', path: [] }], label: 'The header reads the variable and clause counts.' },
  ...[
    { start: 45, end: 51, text: '1 -2 0' },
    { start: 52, end: 60, text: '2 3 -1 0' },
    { start: 61, end: 65, text: '-3 0' },
  ].map((input, index): HeroRelation => ({ id: `clause-${index}`, statement: 'clause', input, spec: list, data: [{ entry: 'clause', path: [index] }], label: `Clause ${index + 1}: store the literals; discard the terminating zero.` })),
];
export const headerCaptures = [
  { entry: 'vars', input: { start: 41, end: 42, text: '3' }, spec: { start: 88, end: 112, text: '{vars:0..gecode.int_max}' } },
  { entry: 'clauses', input: { start: 43, end: 44, text: '3' }, spec: { start: 113, end: 126, text: '{clauses:0..}' } },
] as const;

export function byteSlice(source: string, range: ByteRange): string {
  const bytes = new TextEncoder().encode(source);
  if (!Number.isInteger(range.start) || !Number.isInteger(range.end) || range.start < 0 || range.end <= range.start || range.end > bytes.length) throw new Error(`Invalid hero byte range: ${range.start}..${range.end}`);
  const decoder = new TextDecoder('utf-8', { fatal: true });
  decoder.decode(bytes.slice(0, range.start));
  decoder.decode(bytes.slice(range.end));
  const text = decoder.decode(bytes.slice(range.start, range.end));
  if (text !== range.text) throw new Error(`Hero range ${range.start}..${range.end}: expected ${JSON.stringify(range.text)}, got ${JSON.stringify(text)}`);
  return text;
}

export interface HeroEntry { name: string; type: 'int'; depth: number; value: number | number[][] }
export function heroEntries(json: string): HeroEntry[] {
  const document: unknown = JSON.parse(json);
  if (!document || typeof document !== 'object' || !('entries' in document) || !Array.isArray(document.entries)) throw new Error('Missing hero entries');
  return document.entries.map((entry: unknown) => {
    if (!entry || typeof entry !== 'object' || !('name' in entry) || typeof entry.name !== 'string' || !('type' in entry) || entry.type !== 'int' || !('depth' in entry) || typeof entry.depth !== 'number' || !('value' in entry)) throw new Error('Invalid hero entry');
    const value: unknown = entry.value;
    if (typeof value === 'number' && Number.isSafeInteger(value) && entry.depth === 0) return { name: entry.name, type: 'int', depth: entry.depth, value };
    if (Array.isArray(value) && value.every(row => Array.isArray(row) && row.every(Number.isSafeInteger)) && entry.depth === 2) return { name: entry.name, type: 'int', depth: entry.depth, value };
    throw new Error('Hero requires safe integer scalars and rows');
  });
}
