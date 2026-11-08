// What builder statements, `source()` and `build()` throw.
import { escapeControls, formatCallSite, type CallSite } from "../call-sites.js";
import type { IrisError } from "../errors.js";

/**
 * The check that failed: `scope`, `provenance`, `redefinition`,
 * `unprintable`, `binding`, or `spec` when the emitted text was rejected.
 */
export type BuildErrorKind =
  | "scope"
  | "provenance"
  | "redefinition"
  | "unprintable"
  | "binding"
  | "spec";

/**
 * Why a reader spec could not be built.
 *
 * `kind` names the builder check that failed, or is `spec` when the emitted
 * text failed to parse; then `error` is the language error, which is also
 * the `cause`. `message` shows the language code and message, the line of
 * emitted text, and the call site.
 */
export class BuildError extends Error {
  override readonly name = "BuildError" as const;
  /** The check that failed. */
  readonly kind: BuildErrorKind;
  /**
   * The message of a builder check, without the call site. For a `spec` error
   * it is the language diagnostic, with its location and excerpt, less the
   * call site line.
   */
  readonly reason: string;
  /** The host code of the statement that failed. */
  readonly callSite: CallSite | null;
  /** The emitted text, when the language rejected it. */
  readonly text: string | null;
  /** The language error, when the emitted text was rejected. */
  readonly error: IrisError | null;

  private constructor(
    kind: BuildErrorKind,
    reason: string,
    callSite: CallSite | null,
    text: string | null,
    error: IrisError | null,
  ) {
    super(display(reason, callSite, text, error), error === null ? undefined : { cause: error });
    this.kind = kind;
    this.reason = reason;
    this.callSite = callSite;
    this.text = text;
    this.error = error;
  }

  /** @internal A failed builder check. */
  static check(kind: BuildErrorKind, reason: string, callSite: CallSite | null): BuildError {
    return new BuildError(kind, reason, callSite, null, null);
  }

  /** @internal The error for emitted text that the language rejected. */
  static fromError(error: IrisError, text: string): BuildError {
    let reason = error.diagnostic;
    if (error.callSite !== null) {
      reason = reason.replace(`\ncall site: ${formatCallSite(error.callSite)}`, "");
    }
    return new BuildError("spec", reason, error.callSite, text, error);
  }

  /** The line of emitted text that the language error points at. */
  get emittedLine(): string | null {
    return emittedLine(this.text, this.error);
  }
}

function emittedLine(text: string | null, error: IrisError | null): string | null {
  const line = error?.specLocation?.line ?? null;
  if (text === null || line === null) {
    return null;
  }
  const lines = text.split("\n");
  if (text.endsWith("\n")) {
    lines.pop();
  }
  const found = lines[line - 1];
  return found === undefined ? null : found.replace(/\r$/u, "");
}

function display(
  reason: string,
  callSite: CallSite | null,
  text: string | null,
  error: IrisError | null,
): string {
  const parts = [error === null ? "" : `${error.code}: `, reason];
  const line = emittedLine(text, error);
  const number = error?.specLocation?.line ?? null;
  if (line !== null && number !== null) {
    parts.push(`\n${String(number).padStart(6)} | ${escapeControls(line)}`);
  }
  if (callSite !== null) {
    parts.push(`\ncall site: ${formatCallSite(callSite)}`);
  }
  return parts.join("");
}
