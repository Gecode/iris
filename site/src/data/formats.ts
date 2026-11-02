import { formats } from '../generated/formats';
export { formats };

export type ReadResult =
  | { kind: 'success'; json: string; dzn: string; summary: string }
  | { kind: 'error'; json: string; diagnostic: string };
export interface FormatCase {
  id: string;
  input: { path: string; text: string };
  params: readonly { name: string; value: string }[];
  commands: { readJson: string; readDzn: string; readSummary: string; errorJson: string | null };
  result: ReadResult;
  catalog: { command: string; text: string; dznCommand: string; dzn: string } | null;
  downloads: Readonly<Record<string, string>>;
}
export interface Format {
  id: string;
  slug: string;
  title: string;
  description: string;
  spec: { path: string; text: string };
  readme: { path: string; text: string };
  homeCase: string;
  cases: readonly FormatCase[];
}
export function formatBySlug(slug: string): Format {
  const format = formats.find(format => format.slug === slug);
  if (!format) throw new Error(`Unknown format: ${slug}`);
  return format;
}
export function caseById(format: Format, id: string): FormatCase {
  const example = format.cases.find(example => example.id === id);
  if (!example) throw new Error(`Unknown case: ${format.id}/${id}`);
  return example;
}
