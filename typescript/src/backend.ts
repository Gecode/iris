// The only module that calls the WebAssembly glue's exports, once loading.ts
// has installed its instance. It turns the failure envelopes of rust/iris-js
// into JavaScript errors, replaces the instance after a trap or a read that
// leaves memory past MEMORY_THRESHOLD, and parses specs again after a
// replacement.
import {
  instanceMemory,
  renderDzn as renderDznGlue,
  renderJson as renderJsonGlue,
  SpecHandle,
} from "../wasm/iris_js.js";
import { IrisError, IrisInternalError } from "./errors.js";
import { completeReplacement, instanceGeneration, isLoaded, replaceInstance } from "./loading.js";

/**
 * The size of WebAssembly memory, in bytes, past which a read or catalog call
 * replaces the instance, because WebAssembly memory never shrinks.
 */
export const MEMORY_THRESHOLD: number = 64 * 1024 * 1024;

/**
 * A reader spec bound to the live instance. It keeps the spec's bytes and
 * origin, and parses them again when its handle belongs to a replaced instance.
 */
export class BoundSpec {
  readonly #source: Uint8Array;
  readonly #origin: string;
  #handle: SpecHandle;
  #generation = 0;

  /** Every handle comes from here, so no other function needs the check. */
  constructor(source: Uint8Array, origin: string) {
    if (!isLoaded()) {
      throw new Error("the WebAssembly module is not loaded; await initialize() first");
    }
    this.#source = source;
    this.#origin = origin;
    this.#handle = call(() => this.#parse());
  }

  /** `params` holds name/value pairs, flattened in caller order. */
  read(input: Uint8Array, origin: string, params: string[]): Uint8Array {
    return measured(() => this.#current().read(input, origin, params));
  }

  catalog(input: Uint8Array, origin: string, params: string[]): Uint8Array {
    return measured(() => this.#current().catalog(input, origin, params));
  }

  dispose(): void {
    call(() => {
      // A handle from a replaced instance must never be freed into the new one.
      if (this.#generation === instanceGeneration()) {
        this.#handle.free();
      }
    });
  }

  #current(): SpecHandle {
    if (this.#generation !== instanceGeneration()) {
      this.#handle = this.#parse();
    }
    return this.#handle;
  }

  #parse(): SpecHandle {
    const handle = SpecHandle.parse(this.#source, this.#origin);
    this.#generation = instanceGeneration();
    return handle;
  }
}

export function renderJson(data: Uint8Array): string {
  return call(() => renderJsonGlue(data));
}

export function renderDzn(data: Uint8Array): string {
  return call(() => renderDznGlue(data));
}

/** The byte length of the live instance's memory. */
function memoryBytes(): number {
  const memory: unknown = instanceMemory();
  if (!(memory instanceof WebAssembly.Memory)) {
    throw new Error("the WebAssembly instance did not return its memory");
  }
  return memory.buffer.byteLength;
}

/** Runs a read or catalog call, then releases memory that grew past the threshold. */
function measured<T>(operation: () => T): T {
  try {
    return call(operation);
  } finally {
    if (exceedsThreshold()) {
      replaceOrDefer();
    }
  }
}

/** Whether memory is past the threshold, or cannot be measured. */
function exceedsThreshold(): boolean {
  try {
    return memoryBytes() > MEMORY_THRESHOLD;
  } catch {
    return true;
  }
}

/**
 * Replaces the instance after a call. It never throws, so the call's own
 * result or error always reaches the caller. A failed replacement stays
 * pending: completeReplacement retries it before the next call, and that call
 * throws the failure if the retry fails too, when nothing else is being thrown.
 */
function replaceOrDefer(): void {
  try {
    replaceInstance();
  } catch {
    // replaceInstance left the replacement pending.
  }
}

function call<T>(operation: () => T): T {
  completeReplacement();
  try {
    return operation();
  } catch (thrown) {
    if (thrown instanceof WebAssembly.RuntimeError) {
      throw trapped(thrown);
    }
    // Glue errors are not strings; they propagate unchanged.
    throw typeof thrown === "string" ? failure(thrown) : thrown;
  }
}

/** The instance's state is unknown after a trap, so it is replaced. */
function trapped(trap: WebAssembly.RuntimeError): IrisInternalError {
  replaceOrDefer();
  return new IrisInternalError(
    `the WebAssembly module trapped (${trap.message}); later calls use a new instance`,
    { cause: trap },
  );
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
