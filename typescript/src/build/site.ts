// Call sites: where in the host program each statement was called.
//
// Every public method that writes a statement or checks one calls `caller()`
// first, so the frame two above `caller` is the code that called the method.
// A fixed depth works when the package is bundled into the caller's file, as
// a worker bundle is, where frames cannot be told apart by file.
import type { CallSite } from "../call-sites.js";

/** The call that `trackCaller` helpers pass on, while one runs. */
let tracked: CallSite | null | undefined;

/**
 * Makes a helper record its caller as the call site of its statements.
 *
 * Statements record the line of user code that called them. A helper that
 * writes statements for its caller can pass on that caller's line, as Rust's
 * `#[track_caller]` does, by being wrapped in `trackCaller`. When tracked
 * helpers call each other, the outermost caller is recorded.
 */
export function trackCaller<A extends unknown[], R>(helper: (...args: A) => R): (...args: A) => R {
  if (typeof helper !== "function") {
    throw new TypeError("trackCaller takes a function");
  }
  return function tracking(this: unknown, ...args: A): R {
    if (tracked !== undefined) {
      return Reflect.apply(helper, this, args);
    }
    // Frames: stackFrames, tracking, and the helper's caller.
    tracked = stackFrames()[2] ?? null;
    try {
      return Reflect.apply(helper, this, args);
    } finally {
      tracked = undefined;
    }
  };
}

/**
 * @internal The code that called the public method that calls this, or the
 * caller of the outermost running `trackCaller` helper.
 */
export function caller(): CallSite | null {
  if (tracked !== undefined) {
    return tracked;
  }
  // Frames: stackFrames, caller, the public method, and its caller.
  return stackFrames()[3] ?? null;
}

/** The frames of the current stack, innermost first, starting with this function's. */
function stackFrames(): CallSite[] {
  const limit: unknown = Reflect.get(Error, "stackTraceLimit");
  const raised = typeof limit === "number" && limit < 8;
  if (raised) {
    Reflect.set(Error, "stackTraceLimit", 8);
  }
  let stack: unknown;
  try {
    stack = new Error().stack;
  } finally {
    if (raised) {
      Reflect.set(Error, "stackTraceLimit", limit);
    }
  }
  if (typeof stack !== "string") {
    return [];
  }
  const frames: CallSite[] = [];
  for (const line of stack.split("\n")) {
    const frame = parseFrame(line);
    if (frame !== null) {
      frames.push(frame);
    }
  }
  return frames;
}

// V8: `    at fn (file:1:2)` or `    at file:1:2`, with `async ` or `new ` before `fn`.
const V8_FRAME = /^\s*at (?:(.+?) \((.+):(\d+):(\d+)\)|(.+):(\d+):(\d+))$/u;
// SpiderMonkey and JavaScriptCore: `fn@file:1:2` or `@file:1:2`.
const GECKO_FRAME = /^(.*?)@(.+):(\d+):(\d+)$/u;

/**
 * One frame of a stack trace, or `null` for a line that is not one. The
 * package does not export it; the call-site tests import it.
 */
export function parseFrame(line: string): CallSite | null {
  const v8 = V8_FRAME.exec(line);
  if (v8 !== null) {
    const [, name, file, row, column, bareFile, bareRow, bareColumn] = v8;
    return site(name ?? null, file ?? bareFile, row ?? bareRow, column ?? bareColumn);
  }
  const gecko = GECKO_FRAME.exec(line);
  if (gecko !== null) {
    const [, name, file, row, column] = gecko;
    return site(name === "" ? null : (name ?? null), file, row, column);
  }
  return null;
}

function site(
  name: string | null,
  file: string | undefined,
  row: string | undefined,
  column: string | undefined,
): CallSite | null {
  if (file === undefined || row === undefined || column === undefined) {
    return null;
  }
  const functionName = name?.replace(/^(?:async |new )/u, "") ?? null;
  return Object.freeze({
    file,
    line: Number(row),
    column: Number(column),
    function: functionName,
  });
}
