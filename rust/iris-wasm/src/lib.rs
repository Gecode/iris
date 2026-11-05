//! String-only browser boundary. Canonical documents and all scalar values stay exact.
mod presentation;
mod preview;

use gecode_iris_core::{Failure, Params, Spec};
use gecode_iris_syntax::{FormatOptions, LanguageVersion};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, Write};

const SOURCE_LIMIT: usize = 1 << 20;
const PARAMETER_LIMIT: usize = 64 << 10;
const ORIGIN_LIMIT: usize = 256;
const OUTPUT_LIMIT: usize = 1 << 20;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SpecRequest {
    language: String,
    spec: String,
    spec_origin: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReadRequest {
    language: String,
    spec: String,
    spec_origin: String,
    input: String,
    input_origin: String,
    params: Vec<Parameter>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parameter {
    name: String,
    value: String,
}

/// Read both sources in memory, returning complete CLI documents or one failure.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn read(request_json: &str) -> String {
    respond(|| {
        let request: ReadRequest = decode(request_json)?;
        validate_spec(&request.language, &request.spec, &request.spec_origin)?;
        if request.input.len() > SOURCE_LIMIT || request.input_origin.len() > ORIGIN_LIMIT {
            return Err(host(
                "resource",
                "Input or origin exceeds the playground size limit.",
            ));
        }
        if request.params.len() > 1024
            || request
                .params
                .iter()
                .map(|p| p.name.len() + p.value.len())
                .sum::<usize>()
                > PARAMETER_LIMIT
        {
            return Err(host(
                "resource",
                "Parameters exceed the playground size limit.",
            ));
        }
        let mut params = Params::new();
        for parameter in request.params {
            params.set(parameter.name, parameter.value);
        }
        let spec = Spec::parse_version(
            request.spec.as_bytes(),
            &request.spec_origin,
            LanguageVersion::V0_1,
        )
        .map_err(language_failure)?;
        let data = spec
            .read(request.input.as_bytes(), &request.input_origin, &params)
            .map_err(language_failure)?;
        let mut json = CappedWriter::default();
        data.write_json(&mut json).map_err(output_failure)?;
        let mut dzn = CappedWriter::default();
        data.write_dzn(&mut dzn).map_err(output_failure)?;
        let mut summary = CappedWriter::default();
        let format_name = spec.format_name().filter(|name| !name.is_empty());
        gecode_iris_core::summary::write_summary(
            &mut summary,
            &request.input_origin,
            format_name.as_deref(),
            &data,
        )
        .map_err(output_failure)?;
        Ok(
            json!({"kind":"success", "json":json.text(), "dzn":dzn.text(), "summary":summary.text(), "table":preview::table(&data)}),
        )
    })
}

/// Check in Specification order and return source colour runs and CLI diagnostics.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn check(request_json: &str) -> String {
    respond(|| {
        let request: SpecRequest = decode(request_json)?;
        validate_spec(&request.language, &request.spec, &request.spec_origin)?;
        let parsed = gecode_iris_syntax::parse(
            request.spec.as_bytes(),
            &request.spec_origin,
            LanguageVersion::V0_1,
        )
        .map_err(|error| host("resource", &error.to_string()))?;
        let colors = presentation::colors(&parsed, &request.spec);
        let analysis = gecode_iris_sema::analyze(&parsed);
        if let Some(error) = analysis.first_error() {
            let core_error = gecode_iris_core::Error::from_spec(error);
            return Ok(
                json!({"kind":"language", "code":error.code.as_str(), "diagnostic":format!("{}\n", gecode_iris_sema::render_error(error, request.spec.as_bytes())), "range":presentation::range(error.range, &request.spec), "specLocation":core_error.spec_location(), "errorJson":error_json(&core_error), "colors":colors}),
            );
        }
        let suffix = parsed
            .format_name()
            .filter(|name| !name.is_empty())
            .map_or_else(String::new, |name| format!(" (format {name})"));
        Ok(
            json!({"kind":"checked", "diagnostic":format!("{}: ok{suffix}\n", request.spec_origin), "colors":colors}),
        )
    })
}

/// Format syntax at width 100; semantic validity is deliberately independent.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub fn format(request_json: &str) -> String {
    respond(|| {
        let request: SpecRequest = decode(request_json)?;
        validate_spec(&request.language, &request.spec, &request.spec_origin)?;
        match gecode_iris_syntax::format(
            request.spec.as_bytes(),
            &request.spec_origin,
            LanguageVersion::V0_1,
            FormatOptions::default(),
        ) {
            Ok(spec) => Ok(json!({"kind":"formatted", "spec":spec})),
            Err(error) => Ok(
                json!({"kind":"formatError", "diagnostic":format!("{}\n", gecode_iris_sema::render_format_error(error, request.spec.as_bytes(), &request.spec_origin))}),
            ),
        }
    })
}

fn respond(operation: impl FnOnce() -> Result<Value, Value>) -> String {
    serde_json::to_string(&operation().unwrap_or_else(|error| error))
        .expect("serializable envelope")
}
fn decode<T: for<'de> Deserialize<'de>>(text: &str) -> Result<T, Value> {
    if text.len() > 14 * SOURCE_LIMIT {
        return Err(host(
            "resource",
            "Request exceeds the playground size limit.",
        ));
    }
    serde_json::from_str(text).map_err(|error| host("request", &error.to_string()))
}
fn validate_spec(language: &str, spec: &str, origin: &str) -> Result<(), Value> {
    if language != "0.1" {
        return Err(host(
            "request",
            "Unsupported language version; expected 0.1.",
        ));
    }
    if spec.len() > SOURCE_LIMIT || origin.len() > ORIGIN_LIMIT {
        return Err(host(
            "resource",
            "Reader spec or origin exceeds the playground size limit.",
        ));
    }
    Ok(())
}
fn host(category: &str, message: &str) -> Value {
    json!({"kind":"host", "category":category, "message":message})
}
fn error_json(error: &gecode_iris_core::Error) -> String {
    String::from_utf8(error.to_json().expect("serializable diagnostic")).expect("JSON is UTF-8")
}
fn output_failure(_: impl std::fmt::Display) -> Value {
    host(
        "resource",
        "Output exceeds the playground limit of 1 MiB per representation.",
    )
}
fn language_failure(error: Failure) -> Value {
    match error {
        Failure::Language(error) => {
            json!({"kind":"language", "code":error.code.as_str(), "errorJson":error_json(&error), "diagnostic":format!("{error}\n"), "inputLocation":error.input_location(), "specLocation":error.spec_location(), "inputSpan":error.input_span().map(|span| json!({"start":span.bytes.start, "end":span.bytes.end})), "specSpan":error.spec_span().map(|span| json!({"start":usize::from(span.start()), "end":usize::from(span.end())}))})
        }
        Failure::Unsupported(error) => json!({"kind":"unsupported", "message":error.to_string()}),
        other => host("resource", &other.to_string()),
    }
}

#[derive(Default)]
struct CappedWriter(Vec<u8>);
impl CappedWriter {
    fn text(self) -> String {
        String::from_utf8(self.0).expect("renderers emit UTF-8")
    }
}
impl Write for CappedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > OUTPUT_LIMIT - self.0.len() {
            return Err(io::Error::other("output limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
