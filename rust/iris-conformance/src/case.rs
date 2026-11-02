use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::canonical::{Outcome, OutcomeKind, validate_params};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Parse,
    #[default]
    Read,
    Catalog,
    Dzn,
}

impl Operation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Read => "read",
            Self::Catalog => "catalog",
            Self::Dzn => "dzn",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Json,
    Dzn,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseMetadata {
    #[serde(default)]
    pub operation: Operation,
    pub output: Option<OutputFormat>,
    pub title: Option<String>,
    pub section: Option<String>,
    pub source: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub known_failure: Vec<KnownFailureMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnownFailureMetadata {
    pub driver: String,
    pub outcome: KnownFailureKind,
    pub expected_file: Option<PathBuf>,
    pub reason: String,
    pub difference: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KnownFailureKind {
    Data,
    Parse,
    Dzn,
    Error,
    Unrepresentable,
}

#[derive(Debug)]
pub enum KnownFailureOutcome {
    Alternate(Outcome),
    Unrepresentable,
}

#[derive(Debug)]
pub struct Case {
    pub path: PathBuf,
    pub display_path: String,
    pub operation: Operation,
    pub output: OutputFormat,
    pub spec: PathBuf,
    pub input: Option<PathBuf>,
    pub params: Option<String>,
    pub expected: Outcome,
    pub known_failures: Vec<KnownFailure>,
}

#[derive(Debug)]
pub struct KnownFailure {
    pub driver: String,
    pub expected: KnownFailureOutcome,
    pub reason: String,
    pub difference: Option<String>,
}

impl Case {
    pub fn load(path: &Path, version: &str) -> Result<Self, String> {
        let path = path.canonicalize().map_err(|error| error.to_string())?;
        let display_path = display_path(&path)?;
        let result = load_case(&path, version);
        result.map_err(|error| format!("{display_path}: {error}"))
    }

    pub fn success_kind(&self) -> OutcomeKind {
        success_kind(self.operation, self.output)
    }
}

fn load_case(path: &Path, version: &str) -> Result<Case, String> {
    let metadata = load_metadata(path)?;
    let output = output_format(metadata.operation, metadata.output)?;
    let spec = spec_path(path)?;
    let input = input_path(path, metadata.operation)?;
    let params = load_params(path, metadata.operation)?;
    let success = success_kind(metadata.operation, output);
    let expected = load_primary(path, success, version)?;
    let known_failures =
        load_known_failures(path, metadata.known_failure, &expected, success, version)?;
    Ok(Case {
        path: path.into(),
        display_path: display_path(path)?,
        operation: metadata.operation,
        output,
        spec,
        input,
        params,
        expected,
        known_failures,
    })
}

fn spec_path(path: &Path) -> Result<PathBuf, String> {
    let specs: Vec<PathBuf> = ["spec.iris", "spec.bin"]
        .into_iter()
        .map(|name| path.join(name))
        .filter(|file| fs::symlink_metadata(file).is_ok())
        .collect();
    if specs.len() != 1 {
        return Err("exactly one spec.iris or spec.bin is required".into());
    }
    let spec = &specs[0];
    if !spec.is_file() {
        return Err("spec must be a file or file symlink".into());
    }
    if spec.ends_with("spec.iris") {
        let bytes = fs::read(spec).map_err(|error| error.to_string())?;
        std::str::from_utf8(&bytes)
            .map_err(|error| format!("spec.iris must be valid UTF-8 (use spec.bin): {error}"))?;
    }
    Ok(specs.into_iter().next().expect("one spec was validated"))
}

fn load_metadata(path: &Path) -> Result<CaseMetadata, String> {
    let metadata_path = path.join("case.toml");
    let metadata: CaseMetadata = if metadata_path.exists() {
        toml::from_str(&fs::read_to_string(metadata_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?
    } else {
        CaseMetadata::default()
    };
    for (key, value) in [
        ("title", &metadata.title),
        ("section", &metadata.section),
        ("source", &metadata.source),
    ] {
        if value.as_ref().is_some_and(|text| text.trim().is_empty()) {
            return Err(format!("{key} must be nonempty"));
        }
    }
    Ok(metadata)
}

fn output_format(
    operation: Operation,
    requested: Option<OutputFormat>,
) -> Result<OutputFormat, String> {
    let output = requested.unwrap_or(if operation == Operation::Dzn {
        OutputFormat::Dzn
    } else {
        OutputFormat::Json
    });
    match (operation, output) {
        (Operation::Parse | Operation::Read, OutputFormat::Dzn)
        | (Operation::Dzn, OutputFormat::Json) => {
            return Err("incompatible operation and output".into());
        }
        _ => {}
    }
    Ok(output)
}

fn input_path(path: &Path, operation: Operation) -> Result<Option<PathBuf>, String> {
    let inputs: Vec<PathBuf> = ["input.txt", "input.bin"]
        .into_iter()
        .map(|name| path.join(name))
        .filter(|file| file.exists())
        .collect();
    if inputs.len() > 1 || (operation != Operation::Parse && inputs.len() != 1) {
        return Err("exactly one input.txt or input.bin is required (optional for parse)".into());
    }
    for input in &inputs {
        if !input.is_file() {
            return Err("input must be a file".into());
        }
    }
    if operation == Operation::Parse {
        return Ok(None);
    }
    Ok(inputs.into_iter().next())
}

fn load_params(path: &Path, operation: Operation) -> Result<Option<String>, String> {
    let params_path = path.join("params.json");
    let params = if params_path.exists() {
        let text = fs::read_to_string(params_path).map_err(|error| error.to_string())?;
        let empty = validate_params(&text)?;
        if operation == Operation::Parse && !empty {
            return Err("parse parameters must be absent or empty".into());
        }
        Some(text)
    } else {
        None
    };
    Ok(params)
}

fn load_primary(path: &Path, success: OutcomeKind, version: &str) -> Result<Outcome, String> {
    let files: Vec<&str> = ["expected.json", "expected.dzn", "error.json"]
        .into_iter()
        .filter(|name| path.join(name).exists())
        .collect();
    if files.len() != 1 {
        return Err("exactly one expected.json, expected.dzn or error.json is required".into());
    }
    let kind = match files[0] {
        "error.json" => OutcomeKind::Error,
        "expected.dzn" if success == OutcomeKind::Dzn => OutcomeKind::Dzn,
        "expected.json" if success != OutcomeKind::Dzn => success,
        _ => return Err("expectation file is incompatible with operation/output".into()),
    };
    load_expectation(&path.join(files[0]), kind, version)
}

fn success_kind(operation: Operation, output: OutputFormat) -> OutcomeKind {
    match (operation, output) {
        (Operation::Parse, _) => OutcomeKind::Parse,
        (_, OutputFormat::Dzn) => OutcomeKind::Dzn,
        _ => OutcomeKind::Data,
    }
}

fn load_expectation(path: &Path, kind: OutcomeKind, version: &str) -> Result<Outcome, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Outcome::decode(kind, &bytes, version).map_err(|error| format!("{}: {error}", path.display()))
}

fn load_known_failures(
    path: &Path,
    annotations: Vec<KnownFailureMetadata>,
    primary: &Outcome,
    success: OutcomeKind,
    version: &str,
) -> Result<Vec<KnownFailure>, String> {
    let mut drivers = BTreeSet::new();
    let mut known = Vec::new();
    for annotation in annotations {
        if annotation.driver.trim().is_empty() || annotation.reason.trim().is_empty() {
            return Err("known_failure driver and reason must be nonempty".into());
        }
        if !drivers.insert(annotation.driver.clone()) {
            return Err(format!(
                "duplicate known_failure driver {:?}",
                annotation.driver
            ));
        }
        if annotation.driver.starts_with("gecode-")
            && !annotation
                .difference
                .as_deref()
                .is_some_and(is_difference_link)
        {
            return Err(
                "gecode- driver requires difference = spec/0.1/appendix-c-differences.md#c<N>-..."
                    .into(),
            );
        }
        if annotation
            .difference
            .as_ref()
            .is_some_and(|text| text.trim().is_empty())
        {
            return Err("difference must be nonempty when supplied".into());
        }
        let expected = load_known_outcome(path, &annotation, primary, success, version)?;
        known.push(KnownFailure {
            driver: annotation.driver,
            expected,
            reason: annotation.reason,
            difference: annotation.difference,
        });
    }
    Ok(known)
}

fn load_known_outcome(
    path: &Path,
    annotation: &KnownFailureMetadata,
    primary: &Outcome,
    success: OutcomeKind,
    version: &str,
) -> Result<KnownFailureOutcome, String> {
    let kind = match annotation.outcome {
        KnownFailureKind::Unrepresentable => {
            if !annotation.driver.starts_with("gecode-") {
                return Err("unrepresentable requires a gecode- driver".into());
            }
            if annotation.difference.as_deref()
                != Some("spec/0.1/appendix-c-differences.md#c1-unicode-strings")
            {
                return Err(
                    "unrepresentable requires the C.1 Unicode strings difference link".into(),
                );
            }
            if annotation.expected_file.is_some() {
                return Err("unrepresentable must not supply expected_file".into());
            }
            return Ok(KnownFailureOutcome::Unrepresentable);
        }
        KnownFailureKind::Data => OutcomeKind::Data,
        KnownFailureKind::Parse => OutcomeKind::Parse,
        KnownFailureKind::Dzn => OutcomeKind::Dzn,
        KnownFailureKind::Error => OutcomeKind::Error,
    };
    let file = annotation
        .expected_file
        .as_ref()
        .ok_or("known_failure alternate requires expected_file")?;
    if file.as_os_str().is_empty()
        || file
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err("known_failure expected_file must be case-relative without ..".into());
    }
    if kind != OutcomeKind::Error && kind != success {
        return Err("known_failure outcome incompatible with operation/output".into());
    }
    let expected = load_expectation(&path.join(file), kind, version)?;
    if &expected == primary {
        return Err("known_failure alternate must differ from primary outcome".into());
    }
    Ok(KnownFailureOutcome::Alternate(expected))
}

fn is_difference_link(link: &str) -> bool {
    let Some(anchor) = link.strip_prefix("spec/0.1/appendix-c-differences.md#c") else {
        return false;
    };
    let Some((number, slug)) = anchor.split_once('-') else {
        return false;
    };
    !number.is_empty()
        && number.bytes().all(|byte| byte.is_ascii_digit())
        && !slug.is_empty()
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

pub fn discover(roots: &[PathBuf], filter: Option<&str>) -> Result<Vec<PathBuf>, String> {
    let mut cases = BTreeSet::new();
    for root in roots {
        if !fs::symlink_metadata(root)
            .map_err(|error| format!("{}: {error}", root.display()))?
            .is_dir()
        {
            return Err(format!(
                "{}: root must be a directory, not a symlink",
                root.display()
            ));
        }
        discover_directory(root, &mut cases)?;
    }
    let mut selected = Vec::new();
    for path in cases {
        if filter.is_none_or(|pattern| display_path(&path).is_ok_and(|text| text.contains(pattern)))
        {
            selected.push(path);
        }
    }
    if selected.is_empty() {
        return Err("zero selected cases".into());
    }
    Ok(selected)
}

fn discover_directory(path: &Path, cases: &mut BTreeSet<PathBuf>) -> Result<(), String> {
    if ["spec.iris", "spec.bin", "case.toml"]
        .iter()
        .any(|name| fs::symlink_metadata(path.join(name)).is_ok())
    {
        cases.insert(path.canonicalize().map_err(|error| error.to_string())?);
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            discover_directory(&entry.path(), cases)?;
        }
    }
    Ok(())
}

pub(crate) fn display_path(path: &Path) -> Result<String, String> {
    let cwd = std::env::current_dir()
        .map_err(|error| error.to_string())?
        .canonicalize()
        .map_err(|error| error.to_string())?;
    Ok(path
        .strip_prefix(cwd)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/"))
}
