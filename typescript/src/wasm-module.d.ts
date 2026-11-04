// workerd and edge runtimes compile a statically imported .wasm file.
declare module "*.wasm" {
  const module: WebAssembly.Module;
  export default module;
}
