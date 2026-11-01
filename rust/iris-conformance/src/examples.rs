//! Extract the executable blocks defined by Specification notation §2.3–§2.4.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::canonical::{Outcome, OutcomeKind, validate_params};
use crate::case::Operation;

type Files = BTreeMap<PathBuf, Vec<u8>>;

/// Validate every example before replacing the fully owned destination tree.
/// Check mode only reads files and reports any missing, changed, or stale entry.
pub fn extract(source: &Path, destination: &Path, check: bool) -> Result<String, String> {
    let mut chapters: Vec<PathBuf> = fs::read_dir(source)
        .map_err(|error| format!("{}: {error}", source.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<_, _>>()
        .map_err(|error| error.to_string())?;
    chapters.retain(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"));
    chapters.sort();
    let mut files = Files::new();
    let mut ids = BTreeSet::new();
    for chapter in chapters {
        let text = fs::read_to_string(&chapter).map_err(|error| error.to_string())?;
        extract_chapter(&chapter, &text, &mut ids, &mut files)?;
    }
    if ids.is_empty() {
        return Err("no iris example blocks found".into());
    }
    if check {
        check_tree(destination, &files)?;
    } else {
        replace_tree(source, destination, &files)?;
    }
    let binary_specs = files
        .keys()
        .filter(|path| path.ends_with("spec.bin"))
        .count();
    Ok(format!(
        "{} {} cases ({binary_specs} with spec.bin)",
        if check { "Checked" } else { "Generated" },
        ids.len()
    ))
}

fn replace_tree(source: &Path, destination: &Path, files: &Files) -> Result<(), String> {
    // A mistaken destination must never remove the Specification itself.
    let source = source.canonicalize().map_err(|error| error.to_string())?;
    if destination.exists() {
        let target = destination
            .canonicalize()
            .map_err(|error| error.to_string())?;
        if source.starts_with(&target) {
            return Err("destination must not contain the source directory".into());
        }
        fs::remove_dir_all(destination).map_err(|error| error.to_string())?;
    }
    for (path, bytes) in files {
        let path = destination.join(path);
        fs::create_dir_all(path.parent().expect("generated files have a parent"))
            .map_err(|error| error.to_string())?;
        fs::write(path, bytes).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn extract_chapter(
    chapter: &Path,
    text: &str,
    ids: &mut BTreeSet<String>,
    files: &mut Files,
) -> Result<(), String> {
    let lines: Vec<&str> = text.split_terminator('\n').collect();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start_matches([' ', '\t']);
        let marker = trimmed.as_bytes().first().copied().unwrap_or_default();
        let size = trimmed.bytes().take_while(|byte| *byte == marker).count();
        if !matches!(marker, b'`' | b'~') || size < 3 {
            index += 1;
            continue;
        }
        let info = &trimmed[size..];
        let example = info == "iris example";
        let opening = index;
        if info.trim().starts_with("iris example")
            && (!example || line != trimmed || marker != b'`')
        {
            return Err(format!(
                "{}:{}: example fence must be unindented backticks with info exactly iris example",
                chapter.display(),
                index + 1
            ));
        }
        index += 1;
        while index < lines.len() {
            let closing = lines[index].trim_start_matches([' ', '\t']);
            let run = closing.bytes().take_while(|byte| *byte == marker).count();
            if run >= size && closing[run..].trim().is_empty() {
                if example && closing != lines[index] {
                    return Err(format!(
                        "{}:{}: example closing fence must be unindented",
                        chapter.display(),
                        index + 1
                    ));
                }
                break;
            }
            if example && longest_backtick_run(lines[index]) >= size {
                return Err(format!(
                    "{}:{}: example fence must exceed content backtick runs",
                    chapter.display(),
                    index + 1
                ));
            }
            index += 1;
        }
        if example {
            if index == lines.len() {
                return Err(format!(
                    "{}:{}: unclosed iris example fence",
                    chapter.display(),
                    opening + 1
                ));
            }
            let result = parse_example(&lines[opening + 1..index])
                .and_then(|example| add_example(chapter, example, ids, files));
            result.map_err(|error| format!("{}:{}: {error}", chapter.display(), opening + 1))?;
        }
        index += 1;
    }
    Ok(())
}

fn longest_backtick_run(line: &str) -> usize {
    line.split(|ch| ch != '`').map(str::len).max().unwrap_or(0)
}

struct Example {
    id: String,
    operation: Operation,
    parts: Vec<Part>,
}

struct Part {
    kind: PartKind,
    modifier: Modifier,
    text: String,
}

#[derive(Clone, Copy)]
enum PartKind {
    Spec,
    Input,
    Params,
    Expected(OutcomeKind),
}

#[derive(Clone, Copy)]
enum Modifier {
    Literal,
    NoFinalNewline,
    Hex,
}

fn parse_example(lines: &[&str]) -> Result<Example, String> {
    if lines.iter().any(|line| line.contains('\r')) {
        return Err("CR is forbidden in example bodies; use hex".into());
    }
    let id = lines
        .first()
        .and_then(|line| line.strip_prefix("id: "))
        .filter(|id| is_valid_id(id))
        .ok_or("first header must be id: followed by a valid example ID")?;
    let mut index = 1;
    let operation = if let Some(value) = lines
        .get(index)
        .and_then(|line| line.strip_prefix("operation: "))
    {
        index += 1;
        match value {
            "read" => Operation::Read,
            "parse" => Operation::Parse,
            "catalog" => Operation::Catalog,
            "dzn" => Operation::Dzn,
            _ => return Err(format!("unknown operation {value:?}")),
        }
    } else {
        Operation::Read
    };
    let mut parts = Vec::new();
    while index < lines.len() {
        let (kind, modifier) = parse_marker(lines[index])?;
        index += 1;
        let start = index;
        while index < lines.len() && !lines[index].starts_with("--- ") {
            index += 1;
        }
        let mut text = lines[start..index].join("\n");
        if index > start {
            text.push('\n');
        }
        parts.push(Part {
            kind,
            modifier,
            text,
        });
    }
    validate_parts(operation, &parts)?;
    Ok(Example {
        id: id.into(),
        operation,
        parts,
    })
}

fn is_valid_id(id: &str) -> bool {
    let mut words = id.split('-');
    let first = words.next().unwrap_or_default();
    let rest: Vec<&str> = words.collect();
    !first.is_empty()
        && first.bytes().all(|byte| byte.is_ascii_lowercase())
        && !rest.is_empty()
        && rest.iter().all(|word| {
            !word.is_empty()
                && word
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn parse_marker(line: &str) -> Result<(PartKind, Modifier), String> {
    let marker = line
        .strip_prefix("--- ")
        .ok_or_else(|| format!("unknown/repeated header or invalid marker: {line:?}"))?;
    let (name, modifier) = if let Some(name) = marker.strip_suffix(" no-final-newline") {
        (name, Modifier::NoFinalNewline)
    } else if let Some(name) = marker.strip_suffix(" hex") {
        (name, Modifier::Hex)
    } else {
        (marker, Modifier::Literal)
    };
    let kind = match name {
        "spec" => PartKind::Spec,
        "input" => PartKind::Input,
        "params" => PartKind::Params,
        "expected data" => PartKind::Expected(OutcomeKind::Data),
        "expected parse" => PartKind::Expected(OutcomeKind::Parse),
        "expected dzn" => PartKind::Expected(OutcomeKind::Dzn),
        "expected error" => PartKind::Expected(OutcomeKind::Error),
        _ => return Err(format!("unknown part or modifier: {line:?}")),
    };
    if !matches!(modifier, Modifier::Literal)
        && !matches!(
            kind,
            PartKind::Spec | PartKind::Input | PartKind::Expected(OutcomeKind::Dzn)
        )
    {
        return Err(format!("modifier is not allowed: {line:?}"));
    }
    Ok((kind, modifier))
}

fn validate_parts(operation: Operation, parts: &[Part]) -> Result<(), String> {
    let ranks: Vec<u8> = parts
        .iter()
        .map(|part| match part.kind {
            PartKind::Spec => 0,
            PartKind::Input => 1,
            PartKind::Params => 2,
            PartKind::Expected(_) => 3,
        })
        .collect();
    if ranks.first() != Some(&0)
        || ranks.last() != Some(&3)
        || ranks.windows(2).any(|pair| pair[0] >= pair[1])
        || (operation != Operation::Parse && !ranks.contains(&1))
    {
        return Err("parts must be spec, input (optional for parse), optional params, exactly one expected part".into());
    }
    for part in parts {
        match part.kind {
            PartKind::Params => {
                let empty = validate_params(&part.text)?;
                if operation == Operation::Parse && !empty {
                    return Err("parse params must be absent or empty".into());
                }
            }
            PartKind::Expected(kind) => {
                let compatible = match kind {
                    OutcomeKind::Error => true,
                    OutcomeKind::Parse => operation == Operation::Parse,
                    OutcomeKind::Data => matches!(operation, Operation::Read | Operation::Catalog),
                    OutcomeKind::Dzn => matches!(operation, Operation::Dzn | Operation::Catalog),
                };
                if !compatible {
                    return Err("expected part is incompatible with operation".into());
                }
                Outcome::decode(kind, &decode_part(part)?, "0.1")?;
            }
            _ => {
                decode_part(part)?;
            }
        }
    }
    Ok(())
}

fn decode_part(part: &Part) -> Result<Vec<u8>, String> {
    let mut bytes = part.text.as_bytes().to_vec();
    match part.modifier {
        Modifier::Literal => {}
        Modifier::NoFinalNewline => {
            bytes.pop();
        }
        Modifier::Hex => {
            bytes.retain(|byte| !matches!(byte, b' ' | b'\t' | b'\n'));
            if !bytes.len().is_multiple_of(2)
                || bytes
                    .iter()
                    .any(|byte| !matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            {
                return Err("hex requires pairs of lowercase hexadecimal digits, separated only by ASCII space, tab or LF".into());
            }
            bytes = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| hex_digit(pair[0]) * 16 + hex_digit(pair[1]))
                .collect();
        }
    }
    Ok(bytes)
}

fn hex_digit(byte: u8) -> u8 {
    if byte <= b'9' {
        byte - b'0'
    } else {
        byte - b'a' + 10
    }
}

fn add_example(
    chapter: &Path,
    example: Example,
    ids: &mut BTreeSet<String>,
    files: &mut Files,
) -> Result<(), String> {
    if !ids.insert(example.id.clone()) {
        return Err(format!("duplicate example ID {:?}", example.id));
    }
    let stem = chapter
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or("chapter name must be UTF-8")?;
    let directory = PathBuf::from(format!("{stem}--{}", example.id));
    let source = format!("spec/0.1/{}.md", stem);
    let mut metadata = format!(
        "# Generated by iris-conformance extract-examples; do not edit.\noperation = {:?}\nsection = {:?}\nsource = {:?}\n",
        example.operation.as_str(),
        source,
        format!("spec:{source}#{}", example.id)
    );
    for part in example.parts {
        let bytes = decode_part(&part)?;
        let name = match part.kind {
            PartKind::Spec if std::str::from_utf8(&bytes).is_err() => "spec.bin",
            PartKind::Spec => "spec.iris",
            PartKind::Input => "input.bin",
            PartKind::Params => "params.json",
            PartKind::Expected(OutcomeKind::Error) => "error.json",
            PartKind::Expected(OutcomeKind::Dzn) => {
                if example.operation == Operation::Catalog {
                    metadata.push_str("output = \"dzn\"\n");
                }
                "expected.dzn"
            }
            PartKind::Expected(_) => "expected.json",
        };
        files.insert(directory.join(name), bytes);
    }
    if let Some(known) = known_failure(&example.id) {
        metadata.push_str(&format!(
            "\n[[known_failure]]\ndriver = \"gecode-6.5.0-5e7bbd5c26b0\"\noutcome = {:?}\n",
            known.outcome
        ));
        if let Some((file, expected)) = known.alternate {
            let expected_file = format!("known/gecode-6.5.0-5e7bbd5c26b0/{file}");
            metadata.push_str(&format!("expected_file = {expected_file:?}\n"));
            files.insert(directory.join(expected_file), expected.as_bytes().to_vec());
        }
        metadata.push_str(&format!(
            "reason = {:?}\ndifference = {:?}\n",
            known.reason,
            format!("spec/0.1/appendix-c-differences.md#{}", known.section)
        ));
    }
    files.insert(directory.join("case.toml"), metadata.into_bytes());
    Ok(())
}

struct KnownExampleFailure {
    outcome: &'static str,
    alternate: Option<(&'static str, &'static str)>,
    reason: &'static str,
    section: &'static str,
}

// Deliberate alternate expectations, derived from appendix C rather than a driver run.
// Keep these here so regeneration owns the entire output, including annotations.
fn known_failure(id: &str) -> Option<KnownExampleFailure> {
    match id {
        "input-invalid-utf8-split" => Some(KnownExampleFailure {
            outcome: "unrepresentable",
            alternate: None,
            reason: "Gecode reads a non-UTF-8 word after byte-wise token splitting; the adapter cannot serialize it.",
            section: "c1-unicode-strings",
        }),
        "input-invalid-utf8-comment" | "input-invalid-utf8-tail" => Some(KnownExampleFailure {
            outcome: "data",
            alternate: Some((
                "expected.json",
                "{\"iris\":\"0.1\",\"entries\":[{\"name\":\"x\",\"type\":\"int\",\"depth\":0,\"value\":7}]}\n",
            )),
            reason: "Gecode does not validate UTF-8 in comments or ignored trailing input and reads x = 7.",
            section: "c1-unicode-strings",
        }),
        "reader-invalid-utf8-string" | "reader-invalid-utf8-comment" => Some(KnownExampleFailure {
            outcome: "parse",
            alternate: Some(("expected.json", "{\"iris\":\"0.1\",\"parsed\":true}\n")),
            reason: "Gecode accepts invalid UTF-8 inside reader-spec strings and comments during parsing.",
            section: "c2-reader-spec-source",
        }),
        "dzn-float-precision-gap" => Some(KnownExampleFailure {
            outcome: "dzn",
            alternate: Some((
                "expected.dzn",
                "below = 99999999999999584.0;\nedge = ;\nnegative = ;\nabove = 1e+17;\nvalues = [, 1.0];\n",
            )),
            reason: "The pinned Gecode printer emits empty text for the three float leaves in the C.4 interval.",
            section: "c4-dzn-floats-just-below-1e17",
        }),
        _ => None,
    }
}

fn check_tree(destination: &Path, files: &Files) -> Result<(), String> {
    let expected_directories: BTreeSet<PathBuf> = files
        .keys()
        .flat_map(|path| {
            path.ancestors()
                .skip(1)
                .filter(|path| !path.as_os_str().is_empty())
                .map(Path::to_path_buf)
        })
        .collect();
    let mut actual_files = Files::new();
    let mut actual_directories = BTreeSet::new();
    if fs::symlink_metadata(destination).is_ok_and(|metadata| !metadata.is_dir()) {
        return Err(format!(
            "{}: generated root must be a directory, not a symlink",
            destination.display()
        ));
    }
    if destination.exists() {
        read_tree(
            destination,
            Path::new(""),
            &mut actual_files,
            &mut actual_directories,
        )?;
    }
    let mut differences = Vec::new();
    for (path, bytes) in files {
        match actual_files.get(path) {
            None => differences.push(format!("missing {}", path.display())),
            Some(actual) if actual != bytes => {
                differences.push(format!("changed {}", path.display()))
            }
            _ => {}
        }
    }
    for path in actual_files
        .keys()
        .filter(|path| !files.contains_key(*path))
    {
        differences.push(format!("stale {}", path.display()));
    }
    for path in actual_directories.difference(&expected_directories) {
        differences.push(format!("stale directory {}", path.display()));
    }
    if differences.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} is out of date:\n{}",
            destination.display(),
            differences.join("\n")
        ))
    }
}

fn read_tree(
    root: &Path,
    relative: &Path,
    files: &mut Files,
    directories: &mut BTreeSet<PathBuf>,
) -> Result<(), String> {
    for entry in fs::read_dir(root.join(relative)).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = relative.join(entry.file_name());
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if kind.is_dir() {
            directories.insert(path.clone());
            read_tree(root, &path, files, directories)?;
        } else if kind.is_file() {
            files.insert(
                path,
                fs::read(entry.path()).map_err(|error| error.to_string())?,
            );
        } else {
            return Err(format!(
                "{}: generated tree must contain only regular files and directories",
                entry.path().display()
            ));
        }
    }
    Ok(())
}
