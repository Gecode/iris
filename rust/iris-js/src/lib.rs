//! WebAssembly boundary of the `gecode-iris` npm package.
//!
//! A [`SpecHandle`] parses a reader spec once and reads many inputs. Reads
//! return data in the binary layout of [`transfer`], which JavaScript copies
//! into its own objects, and [`render_json`] and [`render_dzn`] render that
//! layout through gecode-iris-core on request.
//!
//! Every failure is a JSON envelope in the `Err` string, which JavaScript
//! receives as a thrown string:
//!
//! - `{"kind":"language","document":D,"diagnostic":T}`, where `D` is the
//!   canonical error document as text and `T` the error's display text;
//! - `{"kind":"host","category":C,"message":M}`, where `C` is `size`,
//!   `unsupported` or `internal`.
//!
//! The API is ordinary Rust on native targets, so it is tested without a
//! JavaScript engine.
//!
//! The wasm32 export `instance_memory` is internal to the package and absent
//! from its published declarations: it gives the instance's memory, which the
//! package measures after each read. With the off-by-default `fault-injection`
//! feature, the internal export `armTrap` lets the package's tests force the
//! trap that a bug would cause; the published module never has it.
pub mod transfer;

use gecode_iris_core::{Data, Failure, HostError, Params, Spec};
use serde_json::json;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

/// A parsed reader spec, read any number of times.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct SpecHandle {
    spec: Spec,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl SpecHandle {
    pub fn parse(source: &[u8], origin: &str) -> Result<SpecHandle, String> {
        Spec::parse(source, origin)
            .map(|spec| Self { spec })
            .map_err(failure_envelope)
    }

    /// Reads one instance. `params` holds name/value pairs, flattened in caller order.
    pub fn read(&self, input: &[u8], origin: &str, params: Vec<String>) -> Result<Vec<u8>, String> {
        trap_if_armed();
        let data = self.spec.read(input, origin, &to_params(params)?);
        encode(data.map_err(failure_envelope)?)
    }

    /// Lists the instances of an input, with the same arguments as `read`.
    pub fn catalog(
        &self,
        input: &[u8],
        origin: &str,
        params: Vec<String>,
    ) -> Result<Vec<u8>, String> {
        trap_if_armed();
        let data = self.spec.catalog(input, origin, &to_params(params)?);
        encode(data.map_err(failure_envelope)?)
    }
}

/// The canonical JSON document of encoded data, ending in a newline.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = renderJson))]
pub fn render_json(data: &[u8]) -> Result<String, String> {
    let document = decode(data)?.to_json().map_err(internal)?;
    String::from_utf8(document).map_err(internal)
}

/// The dzn text of encoded data, as defined in the Specification's appendix B.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = renderDzn))]
pub fn render_dzn(data: &[u8]) -> Result<String, String> {
    let document = decode(data)?.to_dzn().map_err(internal)?;
    String::from_utf8(document).map_err(internal)
}

/// Internal: the instance's `WebAssembly.Memory`.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(js_name = instanceMemory)]
pub fn instance_memory() -> wasm_bindgen::JsValue {
    wasm_bindgen::memory()
}

#[cfg(all(target_arch = "wasm32", feature = "fault-injection"))]
static TRAP_ARMED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Internal, for the package's tests: the next `read` or `catalog` traps with
/// `unreachable`, as a panic does under `panic = "abort"`.
#[cfg(all(target_arch = "wasm32", feature = "fault-injection"))]
#[wasm_bindgen(js_name = armTrap)]
pub fn arm_trap() {
    TRAP_ARMED.store(true, std::sync::atomic::Ordering::Relaxed);
}

fn trap_if_armed() {
    #[cfg(all(target_arch = "wasm32", feature = "fault-injection"))]
    if TRAP_ARMED.swap(false, std::sync::atomic::Ordering::Relaxed) {
        core::arch::wasm32::unreachable();
    }
}

fn to_params(flattened: Vec<String>) -> Result<Params, String> {
    if !flattened.len().is_multiple_of(2) {
        return Err(host("internal", "parameters must be name/value pairs"));
    }
    let mut params = Params::new();
    let mut pairs = flattened.into_iter();
    while let (Some(name), Some(value)) = (pairs.next(), pairs.next()) {
        params.set(name, value);
    }
    Ok(params)
}

fn encode(data: Data) -> Result<Vec<u8>, String> {
    transfer::encode(&data).map_err(|error| host("size", &error.to_string()))
}

fn decode(bytes: &[u8]) -> Result<Data, String> {
    transfer::decode(bytes).map_err(internal)
}

fn failure_envelope(failure: Failure) -> String {
    match failure {
        Failure::Language(error) => match error.to_json() {
            Ok(document) => json!({
                "kind": "language",
                "document": String::from_utf8_lossy(&document),
                "diagnostic": error.to_string(),
            })
            .to_string(),
            Err(error) => internal(error),
        },
        Failure::Host(error @ HostError::SourceTooLarge { .. }) => host("size", &error.to_string()),
        Failure::Unsupported(feature) => host("unsupported", &feature.to_string()),
        other => internal(other),
    }
}

fn internal(error: impl std::fmt::Display) -> String {
    host("internal", &error.to_string())
}

fn host(category: &str, message: &str) -> String {
    json!({"kind": "host", "category": category, "message": message}).to_string()
}
