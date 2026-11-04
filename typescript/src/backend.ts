// The only module that calls the WebAssembly glue. It turns the failure
// envelopes of rust/iris-js into JavaScript errors.
import {
  initSync,
  renderDzn as renderDznGlue,
  renderJson as renderJsonGlue,
  SpecHandle,
} from "../wasm/iris_js.js";
import { IrisError } from "./errors.js";

export type { SpecHandle };

/** Instantiates the WebAssembly module; every other function requires it. */
export function instantiate(module: WebAssembly.Module): void {
  initSync({ module });
}

export function parseSpec(source: Uint8Array, origin: string): SpecHandle {
  return call(() => SpecHandle.parse(source, origin));
}

/** `params` holds name/value pairs, flattened in caller order. */
export function readData(
  handle: SpecHandle,
  input: Uint8Array,
  origin: string,
  params: string[],
): Uint8Array {
  return call(() => handle.read(input, origin, params));
}

export function catalogData(
  handle: SpecHandle,
  input: Uint8Array,
  origin: string,
  params: string[],
): Uint8Array {
  return call(() => handle.catalog(input, origin, params));
}

export function renderJson(data: Uint8Array): string {
  return call(() => renderJsonGlue(data));
}

export function renderDzn(data: Uint8Array): string {
  return call(() => renderDznGlue(data));
}

function call<T>(operation: () => T): T {
  try {
    return operation();
  } catch (thrown) {
    // Traps and glue errors are not strings; they propagate unchanged.
    throw typeof thrown === "string" ? failure(thrown) : thrown;
  }
}

function failure(envelope: string): Error {
  const parsed: unknown = JSON.parse(envelope);
  const field = (name: string): string => {
    const value: unknown =
      typeof parsed === "object" && parsed !== null ? Reflect.get(parsed, name) : undefined;
    if (typeof value !== "string") {
      throw new Error(`malformed failure envelope: ${envelope}`);
    }
    return value;
  };
  switch (field("kind")) {
    case "language":
      return IrisError.fromDocument(field("document"), field("diagnostic"));
    case "host":
      return field("category") === "size"
        ? new RangeError(field("message"))
        : new Error(field("message"));
    default:
      throw new Error(`malformed failure envelope: ${envelope}`);
  }
}
