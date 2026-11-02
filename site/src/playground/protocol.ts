export type View = 'table' | 'json' | 'dzn' | 'summary';
export interface Parameter { name: string; value: string }
export interface ReadState {
  language: '0.1'; spec: string; input: string; specOrigin: string; inputOrigin: string;
  params: Parameter[]; view: View;
}
export interface ColorRun { start: number; end: number; kind: string }
export type HostFailure = { kind: 'host' | 'unsupported'; message: string };
export type LanguageFailure = { kind: 'language'; code: string; diagnostic: string; errorJson?: string; range?: { start: number; end: number }; colors?: ColorRun[] };
export type CheckResult = { kind: 'checked'; diagnostic: string; colors: ColorRun[] } | LanguageFailure | HostFailure;
export type TableRow = { path: number[]; kind: 'array'; length: number } | { path: number[]; kind: 'scalar'; text: string; clipped: boolean };
export interface TableEntry { name: string; type: 'int' | 'float' | 'string'; depth: number; rows: TableRow[]; truncated: boolean }
export type ReadResult = { kind: 'success'; json: string; dzn: string; summary: string; table: { entries: TableEntry[]; truncated: boolean } } | LanguageFailure | HostFailure;
export type FormatResult = { kind: 'formatted'; spec: string } | { kind: 'formatError'; diagnostic: string } | HostFailure;
export type Request =
  | { kind: 'evaluate'; generation: number; id: number; state: ReadState }
  | { kind: 'format'; generation: number; id: number; spec: string; specOrigin: string };
export type Response =
  | { kind: 'ready' }
  | { kind: 'failed'; message: string }
  | { kind: 'checked'; generation: number; id: number; result: CheckResult }
  | { kind: 'result'; generation: number; id: number; result: ReadResult }
  | { kind: 'formatted'; generation: number; id: number; result: FormatResult };
