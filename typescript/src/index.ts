// The runtime-neutral public API. Entry points instantiate the WebAssembly
// module before re-exporting it.
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
