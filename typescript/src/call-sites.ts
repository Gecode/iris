/** A place in the host program: where a spec builder statement was called. */
export interface CallSite {
  /** The file or URL of the calling code, as the JavaScript engine names it. */
  readonly file: string;
  /** The line of the call, counting from 1. */
  readonly line: number;
  /** The column of the call, counting from 1. */
  readonly column: number;
  /** The name of the calling function, or `null` for top-level code. */
  readonly function: string | null;
}

/** `file:line:column`, with control characters escaped. */
export function formatCallSite(site: CallSite): string {
  return escapeControls(`${site.file}:${site.line}:${site.column}`);
}

/**
 * Escapes control characters as the Rust reader does, so that a terminal
 * never acts on them.
 */
export function escapeControls(text: string): string {
  let result = "";
  for (const character of text) {
    const code = character.charCodeAt(0);
    result +=
      code < 0x20 || (code >= 0x7f && code <= 0x9f)
        ? (ESCAPES[character] ?? `\\u{${code.toString(16)}}`)
        : character;
  }
  return result;
}

const ESCAPES: Readonly<Record<string, string>> = { "\t": "\\t", "\r": "\\r", "\n": "\\n" };

/**
 * The call site of each statement in a built spec's text, by the UTF-8 byte
 * offset where the statement starts. The package does not export it.
 */
export class CallSites {
  readonly #starts: readonly number[];
  readonly #sites: readonly (CallSite | null)[];

  constructor(starts: readonly number[], sites: readonly (CallSite | null)[]) {
    this.#starts = starts;
    this.#sites = sites;
  }

  /** The call site of the statement that holds byte `offset`. */
  at(offset: number): CallSite | null {
    // The number of statements that start at or before the offset.
    let low = 0;
    let high = this.#starts.length;
    while (low < high) {
      const middle = (low + high) >>> 1;
      if ((this.#starts[middle] ?? Infinity) <= offset) {
        low = middle + 1;
      } else {
        high = middle;
      }
    }
    return low === 0 ? null : (this.#sites[low - 1] ?? null);
  }
}
