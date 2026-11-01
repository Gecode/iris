//! A standalone conformance runner; it has no implementation-crate dependency.
pub mod canonical;
pub mod case;
pub mod driver;
pub mod examples;
pub mod report;

use std::path::PathBuf;
use std::time::Duration;

use canonical::{Outcome, OutcomeKind};
use case::{Case, KnownFailureOutcome};
use driver::{DriverCommand, DriverOutput};
pub use report::{CaseResult, CaseStatus, RunReport};

#[derive(Debug)]
pub struct RunOptions {
    pub driver: String,
    pub driver_id: Option<String>,
    pub roots: Vec<PathBuf>,
    pub filter: Option<String>,
    pub version: String,
    pub timeout: Duration,
}

#[derive(Debug)]
pub struct RunError(pub String);

impl std::fmt::Display for RunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for RunError {}

pub fn run(options: &RunOptions) -> Result<RunReport, RunError> {
    validate_options(options).map_err(RunError)?;
    let command = DriverCommand::parse(&options.driver).map_err(RunError)?;
    let paths = case::discover(&options.roots, options.filter.as_deref()).map_err(RunError)?;
    let cases: Vec<Case> = paths
        .iter()
        .map(|path| Case::load(path, &options.version))
        .collect::<Result<_, _>>()
        .map_err(RunError)?;
    let mut report = RunReport::default();
    for case in cases {
        report.cases.push(run_case(&command, &case, options));
    }
    Ok(report)
}

fn run_case(command: &DriverCommand, case: &Case, options: &RunOptions) -> CaseResult {
    let known = options
        .driver_id
        .as_ref()
        .and_then(|id| case.known_failures.iter().find(|known| &known.driver == id));
    let result = command.invoke(case, &options.version, options.timeout);
    let (status, details) = match (result, known) {
        (Ok(output), Some(known))
            if matches!(known.expected, KnownFailureOutcome::Unrepresentable)
                && output.status.code() == Some(3)
                && output.stdout.is_empty()
                && output.stderr
                    == b"iris-gecode-driver: cannot serialize non-UTF-8 reference string\n" =>
        {
            (
                CaseStatus::Unrepresentable,
                vec![report::known_failure_details(known)],
            )
        }
        (Ok(output), _) => match decode_output(case, &output, &options.version) {
            Ok(actual) => {
                let (status, mut details) = report::classify(&case.expected, known, &actual);
                if matches!(status, CaseStatus::Fail | CaseStatus::UnexpectedPass)
                    && !output.stderr.is_empty()
                {
                    details.push(format!("stderr: {}", report::excerpt(&output.stderr)));
                }
                (status, details)
            }
            Err(error) => (
                CaseStatus::Fail,
                vec![format!(
                    "{}: {error}; {}; stdout: {}; stderr: {}",
                    case.operation.as_str(),
                    output.status,
                    report::excerpt(&output.stdout),
                    report::excerpt(&output.stderr)
                )],
            ),
        },
        (Err(error), _) => (
            CaseStatus::Fail,
            vec![format!(
                "{}: {}; stdout: {}; stderr: {}",
                case.operation.as_str(),
                error.message,
                report::excerpt(&error.stdout),
                report::excerpt(&error.stderr)
            )],
        ),
    };
    CaseResult {
        path: case.display_path.clone(),
        status,
        details,
    }
}

fn validate_options(options: &RunOptions) -> Result<(), String> {
    if options.version != "0.1" {
        return Err(format!(
            "unsupported language version {:?}; supported: 0.1",
            options.version
        ));
    }
    if options.timeout.is_zero() {
        return Err("timeout must be positive".into());
    }
    if options
        .driver_id
        .as_ref()
        .is_some_and(|id| id.trim().is_empty())
    {
        return Err("driver ID must be nonempty".into());
    }
    Ok(())
}

fn decode_output(case: &Case, output: &DriverOutput, version: &str) -> Result<Outcome, String> {
    let kind = match output.status.code() {
        Some(0) => case.success_kind(),
        Some(1) => OutcomeKind::Error,
        Some(2) => return Err("driver misuse (exit 2)".into()),
        Some(3) => return Err("host/adapter failure (exit 3)".into()),
        Some(code) => return Err(format!("unsupported driver exit {code}")),
        None => return Err("driver terminated by signal".into()),
    };
    Outcome::decode(kind, &output.stdout, version)
        .map_err(|error| format!("invalid {:?} output: {error}", kind))
}
