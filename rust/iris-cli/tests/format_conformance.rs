use iris_conformance::case::{self, Case, Operation, OutputFormat};
use iris_syntax::{FormatError, FormatOptions, LanguageVersion, ParseError, format};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path, path::PathBuf, process::Command};

#[test]
fn formatting_preserves_conformance_results() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let paths = case::discover(
        &[
            repository.join("conformance/cases/0.1"),
            repository.join("formats"),
        ],
        None,
    )
    .unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let formatted_spec = temporary.path().join("formatted.iris");
    let mut checked = BTreeMap::new();
    let mut syntax_errors = 0;
    let mut invalid_utf8 = 0;
    for path in &paths {
        let case = Case::load(path, "0.1").unwrap();
        let source = fs::read(&case.spec).unwrap();
        let formatted = match format(
            &source,
            &case.spec.to_string_lossy(),
            LanguageVersion::V0_1,
            FormatOptions::default(),
        ) {
            Ok(formatted) => formatted,
            Err(FormatError::Syntax(_)) => {
                syntax_errors += 1;
                continue;
            }
            Err(FormatError::Source(ParseError::InvalidUtf8(_))) => {
                invalid_utf8 += 1;
                continue;
            }
            Err(error) => panic!("{}: {error}", case.display_path),
        };
        fs::write(&formatted_spec, formatted).unwrap();
        assert_eq!(
            run_case(&case, &case.spec),
            run_case(&case, &formatted_spec),
            "{} ({})",
            case.display_path,
            case.operation.as_str()
        );
        *checked.entry(case.operation.as_str()).or_insert(0usize) += 1;
    }
    let count: usize = checked.values().sum();
    println!(
        "same-result formatting: total={}, checked={count} {checked:?}, skipped={} (syntax errors={syntax_errors}, invalid UTF-8={invalid_utf8})",
        paths.len(),
        syntax_errors + invalid_utf8
    );
    assert_eq!(count + syntax_errors + invalid_utf8, paths.len());
    for operation in ["parse", "read", "catalog", "dzn"] {
        assert!(checked[operation] > 0, "no {operation} cases checked");
    }
}

#[derive(Debug, PartialEq)]
enum CaseResult {
    Success(Vec<u8>),
    Error {
        code: String,
        record_path: Option<Vec<usize>>,
    },
}

fn run_case(case: &Case, spec: &Path) -> CaseResult {
    let mut command = Command::new(env!("CARGO_BIN_EXE_iris-driver"));
    command.arg(case.operation.as_str()).arg(spec);
    if let Some(input) = &case.input {
        command.arg(input);
    }
    if let Some(params) = &case.params {
        command.args(["--params", params]);
    }
    if case.operation == Operation::Catalog {
        command.args([
            "--format",
            match case.output {
                OutputFormat::Json => "json",
                OutputFormat::Dzn => "dzn",
            },
        ]);
    }
    let output = command.output().unwrap();
    match output.status.code() {
        Some(1) => {
            let error: Value = serde_json::from_slice(&output.stdout).unwrap();
            // Spec locations move; the error code and record path must not.
            CaseResult::Error {
                code: error["code"].as_str().unwrap().to_owned(),
                record_path: serde_json::from_value(error["record_path"].clone()).unwrap(),
            }
        }
        Some(0) if case.operation == Operation::Catalog && !spec_mentions_summary(case) => {
            CaseResult::Success(normalize_catalog(&output.stdout, case.output))
        }
        Some(0) => CaseResult::Success(output.stdout),
        _ => panic!(
            "{}: driver failed: {}",
            case.display_path,
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

/// A check message that spells out a generated summary would be normalized
/// with it, so such specs compare their catalogs byte for byte.
fn spec_mentions_summary(case: &Case) -> bool {
    let source = fs::read(&case.spec).unwrap();
    source
        .windows(GENERATED_PREFIX.len())
        .any(|window| window == GENERATED_PREFIX.as_bytes())
}

const GENERATED_PREFIX: &str = "check on line ";
const GENERATED_SUFFIX: &str = " of the spec";

/// Replaces the spec line in each generated check summary (errors.md
/// §11.2.1). Only a whole failure entry of exactly that form changes, so
/// check messages that merely contain the phrase are still compared.
fn normalize_catalog(output: &[u8], format: OutputFormat) -> Vec<u8> {
    match format {
        OutputFormat::Json => {
            let mut catalog: Value = serde_json::from_slice(output).unwrap();
            for entry in catalog["entries"].as_array_mut().unwrap() {
                if entry["name"] == "failed" {
                    for failure in entry["value"].as_array_mut().unwrap() {
                        if is_generated_summary(failure.as_str().unwrap()) {
                            *failure = Value::String(normalized_summary());
                        }
                    }
                }
            }
            serde_json::to_vec(&catalog).unwrap()
        }
        OutputFormat::Dzn => std::str::from_utf8(output)
            .unwrap()
            .split_inclusive('\n')
            .map(|line| match line.strip_prefix("failed = [") {
                Some(elements) => format!("failed = [{}", normalize_dzn_failures(elements)),
                None => line.to_owned(),
            })
            .collect::<String>()
            .into_bytes(),
    }
}

/// Normalizes the dzn string elements that follow `failed = [`. Between
/// elements there are only separators, and every quote inside an element is
/// escaped, so each unescaped quote starts the next element.
fn normalize_dzn_failures(elements: &str) -> String {
    let mut normalized = String::new();
    let mut remaining = elements;
    loop {
        let Some(start) = remaining.find('"') else {
            normalized.push_str(remaining);
            return normalized;
        };
        let Some(length) = dzn_string_length(&remaining[start..]) else {
            normalized.push_str(remaining);
            return normalized;
        };
        normalized.push_str(&remaining[..start]);
        let element = &remaining[start..start + length];
        if is_generated_summary(&element[1..element.len() - 1]) {
            normalized.push('"');
            normalized.push_str(&normalized_summary());
            normalized.push('"');
        } else {
            normalized.push_str(element);
        }
        remaining = &remaining[start + length..];
    }
}

/// Returns the byte length of the dzn string literal at the start of `text`,
/// including both quotes.
fn dzn_string_length(text: &str) -> Option<usize> {
    let mut escaped = false;
    for (offset, byte) in text.bytes().enumerate().skip(1) {
        match byte {
            _ if escaped => escaped = false,
            b'\\' => escaped = true,
            b'"' => return Some(offset + 1),
            _ => {}
        }
    }
    None
}

fn is_generated_summary(text: &str) -> bool {
    text.strip_prefix(GENERATED_PREFIX)
        .and_then(|rest| rest.strip_suffix(GENERATED_SUFFIX))
        .is_some_and(|line| !line.is_empty() && line.bytes().all(|b| b.is_ascii_digit()))
}

fn normalized_summary() -> String {
    format!("{GENERATED_PREFIX}N{GENERATED_SUFFIX}")
}

#[test]
fn only_whole_generated_summaries_are_normalized() {
    let json = br#"{"entries":[{"name":"failed","value":["check on line 8 of the spec","literal check on line 12 of the spec",""]}]}"#;
    let expected = br#"{"entries":[{"name":"failed","value":["check on line N of the spec","literal check on line 12 of the spec",""]}]}"#;
    assert_eq!(
        normalize_catalog(json, OutputFormat::Json),
        serde_json::to_vec(&serde_json::from_slice::<Value>(expected).unwrap()).unwrap()
    );
    let dzn = "n = 1;\nfailed = [\"check on line 8 of the spec\", \"literal check on line 12 of the spec\", \"a \\\"check on line 3 of the spec\\\"\", \"\"];\n";
    let expected = "n = 1;\nfailed = [\"check on line N of the spec\", \"literal check on line 12 of the spec\", \"a \\\"check on line 3 of the spec\\\"\", \"\"];\n";
    assert_eq!(
        String::from_utf8(normalize_catalog(dzn.as_bytes(), OutputFormat::Dzn)).unwrap(),
        expected
    );
}
