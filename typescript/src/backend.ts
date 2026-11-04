// The only module that calls the WebAssembly glue's exports, once loading.ts
// has installed its instance. It turns the failure envelopes of rust/iris-js
// into JavaScript errors.
import {
  renderDzn as renderDznGlue,
  renderJson as renderJsonGlue,
  SpecHandle,
} from "../wasm/iris_js.js";
import { IrisError } from "./errors.js";
import { isLoaded } from "./loading.js";

export type { SpecHandle };

/** Every handle comes from here, so no other function needs the check. */
export function parseSpec(source: Uint8Array, origin: string): SpecHandle {
  if (!isLoaded()) {
    throw new Error("the WebAssembly module is not loaded; await initialize() first");
  }
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
