// The runtime-neutral public API, and the gecode-iris/manual entry point.
// The other entry points load the WebAssembly module before re-exporting it.
export { initialize, type ModuleSource } from "./loading.js";
export { Spec, type Params, type ReadOptions } from "./spec.js";
export {
  Data,
  type BigIntValue,
  type Entry,
  type Path,
  type Value,
  type ValueOptions,
  type ValueType,
} from "./data.js";
export { IrisError, type Location } from "./errors.js";
