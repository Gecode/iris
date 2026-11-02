/** Preserve an exact prefix, stopping at complete lines where possible. */
export function examplePreview(source, fullHref) {
  const points = Array.from(source);
  let text = points.slice(0, 2400).join('');
  const lines = text.match(/[^\n]*\n|[^\n]+$/g) ?? [];
  text = lines.slice(0, 24).join('');
  if (text.length < source.length && !text.endsWith('\n') && text.includes('\n')) text = text.slice(0, text.lastIndexOf('\n') + 1);
  return { text, truncated: text.length < source.length, fullHref };
}
