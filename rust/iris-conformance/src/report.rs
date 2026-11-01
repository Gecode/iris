use std::fmt::Write;

use crate::canonical::{Data, Outcome, ValueToken};
use crate::case::{KnownFailure, KnownFailureOutcome};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CaseStatus {
    Pass,
    Fail,
    KnownFailure,
    Unrepresentable,
    UnexpectedPass,
}

#[derive(Debug)]
pub struct CaseResult {
    pub path: String,
    pub status: CaseStatus,
    pub details: Vec<String>,
}

#[derive(Debug, Default)]
pub struct RunReport {
    pub cases: Vec<CaseResult>,
}

impl RunReport {
    pub fn exit_code(&self) -> u8 {
        u8::from(
            self.cases
                .iter()
                .any(|case| matches!(case.status, CaseStatus::Fail | CaseStatus::UnexpectedPass)),
        )
    }

    pub fn render(&self) -> String {
        let mut text = String::new();
        let mut counts = [0; 5];
        for case in &self.cases {
            let (label, index) = match case.status {
                CaseStatus::Pass => ("PASS", 0),
                CaseStatus::KnownFailure => ("XFAIL", 1),
                CaseStatus::Fail => ("FAIL", 2),
                CaseStatus::UnexpectedPass => ("XPASS", 3),
                CaseStatus::Unrepresentable => ("UNREPRESENTABLE", 4),
            };
            counts[index] += 1;
            writeln!(text, "{label} {}", case.path).expect("writing to String cannot fail");
            for detail in &case.details {
                writeln!(text, "  {detail}").expect("writing to String cannot fail");
            }
        }
        writeln!(
            text,
            "Summary: total={} pass={} xfail={} fail={} xpass={} unrepresentable={}",
            self.cases.len(),
            counts[0],
            counts[1],
            counts[2],
            counts[3],
            counts[4]
        )
        .expect("writing to String cannot fail");
        text
    }
}

pub(crate) fn classify(
    expected: &Outcome,
    known: Option<&KnownFailure>,
    actual: &Outcome,
) -> (CaseStatus, Vec<String>) {
    if expected == actual {
        return match known {
            None => (CaseStatus::Pass, Vec::new()),
            Some(known) => (
                CaseStatus::UnexpectedPass,
                vec![format!(
                    "primary matched; stale known failure for {}: {}",
                    known.driver, known.reason
                )],
            ),
        };
    }
    if let Some(known) = known
        && let KnownFailureOutcome::Alternate(alternate) = &known.expected
        && alternate == actual
    {
        return (CaseStatus::KnownFailure, vec![known_failure_details(known)]);
    }
    let mut details = differences(expected, actual);
    if let Some(known) = known {
        let mismatch = match &known.expected {
            KnownFailureOutcome::Alternate(alternate) => differences(alternate, actual).join("; "),
            KnownFailureOutcome::Unrepresentable => "actual outcome is representable".into(),
        };
        details.push(format!(
            "alternate for {} did not match: {}",
            known.driver, mismatch
        ));
    }
    (CaseStatus::Fail, details)
}

pub(crate) fn known_failure_details(known: &KnownFailure) -> String {
    let mut explanation = format!("{}: {}", known.driver, known.reason);
    if let Some(link) = &known.difference {
        explanation.push_str(&format!(" ({link})"));
    }
    explanation
}

pub fn differences(expected: &Outcome, actual: &Outcome) -> Vec<String> {
    match (expected, actual) {
        (Outcome::Data(expected), Outcome::Data(actual)) => data_differences(expected, actual),
        (Outcome::Dzn(expected), Outcome::Dzn(actual)) if expected != actual => {
            vec![byte_difference(expected, actual)]
        }
        (Outcome::Error(expected), Outcome::Error(actual)) => {
            let mut details = Vec::new();
            if expected.code != actual.code {
                details.push(format!(
                    "error code: expected {:?}, actual {:?}",
                    expected.code, actual.code
                ));
            }
            if expected.line != actual.line {
                details.push(format!(
                    "error primary line: expected {:?}, actual {:?}",
                    expected.line, actual.line
                ));
            }
            if expected.column != actual.column {
                details.push(format!(
                    "error primary column: expected {:?}, actual {:?}",
                    expected.column, actual.column
                ));
            }
            if expected.record_path != actual.record_path {
                details.push(format!(
                    "error record_path: expected {:?}, actual {:?}",
                    expected.record_path, actual.record_path
                ));
            }
            details
        }
        _ if expected == actual => Vec::new(),
        _ => vec![format!(
            "outcome: expected {:?}, actual {:?}",
            expected.kind(),
            actual.kind()
        )],
    }
}

fn data_differences(expected: &Data, actual: &Data) -> Vec<String> {
    let mut details = Vec::new();
    if expected.entries.len() != actual.entries.len() {
        details.push(format!(
            "entry count: expected {}, actual {}",
            expected.entries.len(),
            actual.entries.len()
        ));
    }
    for (index, (expected, actual)) in expected.entries.iter().zip(&actual.entries).enumerate() {
        let entry = format!("entry[{index}] {:?}", expected.name);
        if expected.name != actual.name {
            details.push(format!(
                "{entry} name: expected {:?}, actual {:?}",
                expected.name, actual.name
            ));
        }
        if expected.value_type != actual.value_type {
            details.push(format!(
                "{entry} type: expected {:?}, actual {:?}",
                expected.value_type, actual.value_type
            ));
        }
        if expected.depth != actual.depth {
            details.push(format!(
                "{entry} depth: expected {}, actual {}",
                expected.depth, actual.depth
            ));
        }
        if let Some(position) = expected
            .value
            .iter()
            .zip(&actual.value)
            .position(|(left, right)| left != right)
        {
            details.push(format!(
                "{entry} value{}: expected {}, actual {}",
                token_path(&expected.value, position),
                token_excerpt(&expected.value[position]),
                token_excerpt(&actual.value[position])
            ));
        }
    }
    details
}

fn token_excerpt(token: &ValueToken) -> String {
    match token {
        ValueToken::Array(length) => format!("array length {length}"),
        ValueToken::Int(value) => value.to_string(),
        ValueToken::Float(value) => value.clone(),
        ValueToken::String(value) => excerpt(value.as_bytes()),
    }
}

fn token_path(tokens: &[ValueToken], position: usize) -> String {
    let mut arrays: Vec<(usize, usize)> = Vec::new();
    for token in &tokens[..position] {
        if let ValueToken::Array(length) = token
            && *length > 0
        {
            arrays.push((*length, 0));
            continue;
        }
        while let Some((length, next)) = arrays.last_mut() {
            *next += 1;
            if *next < *length {
                break;
            }
            arrays.pop();
        }
    }
    arrays
        .iter()
        .map(|(_, index)| format!("[{index}]"))
        .collect()
}

fn byte_difference(expected: &[u8], actual: &[u8]) -> String {
    let offset = expected
        .iter()
        .zip(actual)
        .position(|(left, right)| left != right)
        .unwrap_or(expected.len().min(actual.len()));
    let prefix = &expected[..offset];
    let line = prefix.iter().filter(|byte| **byte == b'\n').count() + 1;
    let column = prefix
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(offset + 1, |last| offset - last);
    let start = offset.saturating_sub(20);
    format!(
        "dzn byte {offset} (line {line}, byte column {column}): expected {} (length {}), actual {} (length {})",
        excerpt(&expected[start..]),
        expected.len(),
        excerpt(&actual[start..]),
        actual.len()
    )
}

pub(crate) fn excerpt(bytes: &[u8]) -> String {
    let mut text = String::from("\"");
    for byte in bytes.iter().take(240) {
        text.extend(std::ascii::escape_default(*byte).map(char::from));
    }
    if bytes.len() > 240 {
        text.push_str("...");
    }
    text.push('"');
    text
}
