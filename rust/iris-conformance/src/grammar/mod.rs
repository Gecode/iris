//! Independent recognition of case reader specs using the embedded formal grammar.
mod ebnf;
mod recognize;
mod source;

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use crate::canonical::Outcome;
use crate::case::{self, Case};
use ebnf::Grammar;
use recognize::{Failure, Recognizer};

// A copy of spec/0.1/grammar.ebnf; tests/embedded.rs keeps it in step.
const GRAMMAR: &str = include_str!("../../embedded/0.1/grammar.ebnf");

#[derive(Debug, Default)]
pub struct GrammarReport {
    pub total: usize,
    pub valid: usize,
    pub invalid: usize,
    pub accepted: usize,
    pub rejected: usize,
    pub failures: Vec<String>,
}

impl GrammarReport {
    pub fn exit_code(&self) -> u8 {
        u8::from(!self.failures.is_empty())
    }

    pub fn render(&self) -> String {
        let mut output = String::new();
        for failure in &self.failures {
            writeln!(output, "FAIL {failure}").unwrap();
        }
        writeln!(
            output,
            "Grammar summary: total={} valid={} invalid={} accepted={} rejected={} fail={}",
            self.total,
            self.valid,
            self.invalid,
            self.accepted,
            self.rejected,
            self.failures.len()
        )
        .unwrap();
        output
    }
}

/// Check case roots without invoking an implementation or a driver. Empty roots
/// select the build-time checkout's conformance cases and format cases.
pub fn check_grammar(roots: &[PathBuf]) -> Result<GrammarReport, String> {
    let grammar = Grammar::parse(GRAMMAR)?;
    let defaults;
    let roots = if roots.is_empty() {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        defaults = [
            repository.join("conformance/cases/0.1"),
            repository.join("formats"),
        ];
        &defaults
    } else {
        roots
    };
    let cases = case::discover(roots, None)?
        .iter()
        .map(|path| Case::load(path, "0.1"))
        .collect::<Result<Vec<_>, _>>()?;
    let mut report = GrammarReport::default();
    for case in cases {
        let should_accept = expects_acceptance(&case.expected);
        let bytes =
            fs::read(&case.spec).map_err(|error| format!("{}: {error}", case.spec.display()))?;
        let result = Recognizer::recognize(&grammar, &bytes);
        if let Err(Failure::Configuration(message)) = &result {
            return Err(message.clone());
        }
        let accepted = result.is_ok();
        report.total += 1;
        if should_accept {
            report.valid += 1;
        } else {
            report.invalid += 1;
        }
        if accepted {
            report.accepted += 1;
        } else {
            report.rejected += 1;
        }
        if accepted != should_accept {
            let code = match &case.expected {
                Outcome::Error(error) => error.code.as_str(),
                _ => "success",
            };
            let details = match result {
                Err(Failure::Rejected(rejection)) => {
                    format!("expected acceptance ({code}); {rejection}")
                }
                Ok(()) => format!(
                    "expected rejection ({code}); reader spec was accepted despite its grammar-negative expected code"
                ),
                _ => unreachable!("recognize returns a rejection or a configuration error"),
            };
            report
                .failures
                .push(format!("{}: {details}", case.display_path));
        }
    }
    Ok(report)
}

fn expects_acceptance(outcome: &Outcome) -> bool {
    // Error ownership: Specification errors.md §§11.4/11.6 and appendix A.5.
    // New registry codes must be assessed here; E-SPEC- also covers semantics.
    !matches!(outcome, Outcome::Error(error) if matches!(error.code.as_str(),
        "E-SPEC-UTF8" | "E-SPEC-CHARACTER" | "E-SPEC-STRING" | "E-SPEC-NUMBER" |
        "E-SPEC-SYNTAX" | "E-SPEC-PLACEMENT" | "E-SPEC-PATTERN" |
        "E-SPEC-UNKNOWN-TYPE" | "E-SPEC-NESTING"))
}

#[cfg(test)]
mod tests;
