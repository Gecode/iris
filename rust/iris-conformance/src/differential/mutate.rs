//! The `mutate` strategy: a conformance case with one to three seeded
//! token-level mutations of its spec, input or parameters.

use std::fs;
use std::path::{Path, PathBuf};

use super::rng::Rng;
use crate::case::{self, CaseMetadata};

/// The files of a case that seed mutation.
pub(super) struct Seed {
    spec: Vec<u8>,
    input: Vec<u8>,
    params: Option<String>,
}

/// A mutated spec, input and parameters.
pub(super) struct Mutated {
    pub(super) spec: Vec<u8>,
    pub(super) input: Vec<u8>,
    pub(super) params: Option<String>,
}

/// The cases under Specification chapter 10 drive a work budget to its
/// limit by design, which makes them slow under sanitizers; as in
/// `rust/fuzz/seed-corpus.py`, they seed nothing.
const WORK_BUDGET_CHAPTER: &str = "spec/0.1/10-work-budgets.md";

/// Load the seeds from every case under `roots`.
pub(super) fn load_seeds(roots: &[PathBuf]) -> Result<Vec<Seed>, String> {
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    let mut seeds = Vec::new();
    for path in case::discover(roots, None)? {
        let seed = load_seed(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        seeds.extend(seed);
    }
    Ok(seeds)
}

fn load_seed(path: &Path) -> Result<Option<Seed>, String> {
    let metadata_path = path.join("case.toml");
    if metadata_path.exists() {
        let text = fs::read_to_string(&metadata_path).map_err(|error| error.to_string())?;
        let metadata: CaseMetadata = toml::from_str(&text).map_err(|error| error.to_string())?;
        if metadata
            .section
            .is_some_and(|section| section.starts_with(WORK_BUDGET_CHAPTER))
        {
            return Ok(None);
        }
    }
    let Some(spec) = read_first(path, &["spec.iris", "spec.bin"])? else {
        return Ok(None);
    };
    let input = read_first(path, &["input.txt", "input.bin"])?.unwrap_or_default();
    let params = read_first(path, &["params.json"])?
        .map(|bytes| String::from_utf8(bytes).map_err(|error| error.to_string()))
        .transpose()?;
    Ok(Some(Seed {
        spec,
        input,
        params,
    }))
}

fn read_first(directory: &Path, names: &[&str]) -> Result<Option<Vec<u8>>, String> {
    for name in names {
        let path = directory.join(name);
        if path.is_file() {
            return fs::read(path).map(Some).map_err(|error| error.to_string());
        }
    }
    Ok(None)
}

pub(super) fn mutate(rng: &mut Rng, seeds: &[Seed]) -> Mutated {
    let seed = rng.pick(seeds);
    let mut mutated = Mutated {
        spec: seed.spec.clone(),
        input: seed.input.clone(),
        params: seed.params.clone(),
    };
    for _ in 0..rng.between(1, 3) {
        match rng.below(10) {
            0..=4 => mutate_spec(rng, &mut mutated.spec),
            5..=8 => mutate_input(rng, &mut mutated.input),
            _ => mutated.params = mutate_params(rng, mutated.params.as_deref()),
        }
    }
    mutated
}

const BOUNDARY_INTEGERS: [&str; 9] = [
    "0",
    "1",
    "-1",
    "2147483646",
    "2147483647",
    "2147483648",
    "9223372036854775807",
    "9223372036854775808",
    "-9223372036854775808",
];
const BOUNDARY_NUMBERS: [&str; 10] = [
    "-0",
    "+007",
    "1e309",
    "5e-324",
    "0e-9999",
    "2.2250738585072014e-308",
    "1.e2",
    "+.5",
    "1E2",
    "9007199254740993",
];
const BASE_TYPES: [&str; 5] = ["int", "float", "num", "word", "rest"];
const OPERATORS: [&str; 13] = [
    "+", "-", "*", "/", "%", "^", "<", "<=", ">", "==", "!=", "&&", "||",
];
const BLANKS: [&[u8]; 7] = [b" ", b"\r", b"\t", b"\x0b", b"\x0c", b"\n", b"\r\n"];
const INVALID_UTF8: [&[u8]; 4] = [b"\xff", b"\xc3", b"\xed\xa0\x80", b"\xf4\x90\x80\x80"];
const SETTINGS: [&str; 6] = [
    "comment \"#\"\n",
    "ignore trailing\n",
    "separators \",\"\n",
    "tokens \":\"\n",
    "stop \"%\"\n",
    "format mutated\n",
];

fn mutate_spec(rng: &mut Rng, spec: &mut Vec<u8>) {
    match rng.below(8) {
        0 => replace_token(rng, spec, is_integer_token, &BOUNDARY_INTEGERS),
        1 => replace_token(rng, spec, is_float_token, &BOUNDARY_NUMBERS),
        2 => replace_token(rng, spec, |token| BASE_TYPES.contains(&token), &BASE_TYPES),
        3 => replace_token(rng, spec, |token| OPERATORS.contains(&token), &OPERATORS),
        4 => replace_token(
            rng,
            spec,
            |token| token == ".." || token == "..<",
            &["..", "..<"],
        ),
        5 => duplicate_line(rng, spec),
        6 => delete_line(rng, spec),
        _ => {
            let setting = rng.pick(&SETTINGS);
            spec.splice(0..0, setting.bytes());
        }
    }
}

fn mutate_input(rng: &mut Rng, input: &mut Vec<u8>) {
    match rng.below(8) {
        0 => replace_token(rng, input, is_integer_token, &BOUNDARY_INTEGERS),
        1 => replace_token(rng, input, is_number_token, &BOUNDARY_NUMBERS),
        2 => duplicate_line(rng, input),
        3 => delete_line(rng, input),
        4 => {
            let tokens = token_spans(input);
            if !tokens.is_empty() {
                let (start, end) = tokens[rng.below(tokens.len())];
                if rng.chance(50) {
                    input.drain(start..end);
                } else {
                    let copy = input[start..end].to_vec();
                    input.splice(end..end, copy.into_iter().chain(*b" "));
                }
            }
        }
        5 => {
            let position = rng.below(input.len() + 1);
            let blank = *rng.pick(&BLANKS);
            input.splice(position..position, blank.iter().copied());
        }
        6 => {
            let end = rng.below(input.len() + 1);
            input.truncate(end);
        }
        _ => {
            let position = rng.below(input.len() + 1);
            let invalid = *rng.pick(&INVALID_UTF8);
            input.splice(position..position, invalid.iter().copied());
        }
    }
}

/// Drop a parameter or change one's value. Parameters that are not a JSON
/// object of strings stay as they are.
fn mutate_params(rng: &mut Rng, params: Option<&str>) -> Option<String> {
    let text = params?;
    let Ok(mut map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(text)
    else {
        return Some(text.into());
    };
    let keys: Vec<String> = map.keys().cloned().collect();
    if keys.is_empty() {
        return Some(text.into());
    }
    let key = rng.pick(&keys).clone();
    if rng.chance(40) {
        map.remove(&key);
    } else {
        let value = rng.pick(&[
            "0",
            "-1",
            "+01",
            "1e3",
            "x",
            "",
            " 2",
            "9223372036854775808",
        ]);
        map.insert(key, serde_json::Value::String((*value).into()));
    }
    Some(serde_json::Value::Object(map).to_string())
}

/// Replace a randomly chosen token that `selects` accepts with one of
/// `replacements`.
fn replace_token(
    rng: &mut Rng,
    bytes: &mut Vec<u8>,
    selects: impl Fn(&str) -> bool,
    replacements: &[&str],
) {
    let candidates: Vec<(usize, usize)> = token_spans(bytes)
        .into_iter()
        .filter(|&(start, end)| std::str::from_utf8(&bytes[start..end]).is_ok_and(&selects))
        .collect();
    if candidates.is_empty() {
        return;
    }
    let (start, end) = candidates[rng.below(candidates.len())];
    let replacement = rng.pick(replacements);
    bytes.splice(start..end, replacement.bytes());
}

/// The spans of the names, numbers, operators and ranges in `bytes`. A
/// number starts with a digit and runs over digits, dots, exponents and an
/// exponent's sign, but stops before `..`; operators are maximal runs of
/// operator characters; `..` and `..<` are spans of their own.
fn token_spans(bytes: &[u8]) -> Vec<(usize, usize)> {
    let is_name = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let is_operator = |byte: u8| b"<>=!&|*/%^+-".contains(&byte);
    let mut spans = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let rest = &bytes[start..];
        let length = if rest.starts_with(b"..") {
            if rest[2..].starts_with(b"<") { 3 } else { 2 }
        } else if rest[0].is_ascii_digit() {
            number_length(rest)
        } else if is_name(rest[0]) {
            rest.iter().take_while(|&&byte| is_name(byte)).count()
        } else if is_operator(rest[0]) {
            rest.iter().take_while(|&&byte| is_operator(byte)).count()
        } else {
            start += 1;
            continue;
        };
        spans.push((start, start + length));
        start += length;
    }
    spans
}

fn number_length(bytes: &[u8]) -> usize {
    let mut length = 0;
    while length < bytes.len() {
        let byte = bytes[length];
        let after_exponent = length > 0 && matches!(bytes[length - 1], b'e' | b'E');
        let continues = byte.is_ascii_digit()
            || matches!(byte, b'e' | b'E')
            || (byte == b'.' && !bytes[length..].starts_with(b".."))
            || (after_exponent && matches!(byte, b'+' | b'-'));
        if !continues {
            break;
        }
        length += 1;
    }
    length
}

fn is_integer_token(token: &str) -> bool {
    let digits = token.strip_prefix(['+', '-']).unwrap_or(token);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_float_token(token: &str) -> bool {
    token.parse::<f64>().is_ok() && !is_integer_token(token) && token.contains(['.', 'e', 'E'])
}

fn is_number_token(token: &str) -> bool {
    is_integer_token(token) || is_float_token(token)
}

fn line_spans(bytes: &[u8]) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = 0;
    for (position, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            spans.push((start, position + 1));
            start = position + 1;
        }
    }
    if start < bytes.len() {
        spans.push((start, bytes.len()));
    }
    spans
}

fn duplicate_line(rng: &mut Rng, bytes: &mut Vec<u8>) {
    let lines = line_spans(bytes);
    if lines.is_empty() {
        return;
    }
    let (start, end) = lines[rng.below(lines.len())];
    let mut copy = bytes[start..end].to_vec();
    if copy.last() != Some(&b'\n') {
        copy.insert(0, b'\n');
    }
    bytes.splice(end..end, copy);
}

fn delete_line(rng: &mut Rng, bytes: &mut Vec<u8>) {
    let lines = line_spans(bytes);
    if lines.is_empty() {
        return;
    }
    let (start, end) = lines[rng.below(lines.len())];
    bytes.drain(start..end);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_spans_split_ranges_and_signs() {
        let text = b"x: -3..<n + 1e-3";
        let tokens: Vec<&[u8]> = token_spans(text)
            .into_iter()
            .map(|(start, end)| &text[start..end])
            .collect();
        assert!(tokens.contains(&&b"..<"[..]));
        assert!(tokens.contains(&&b"-"[..]));
        assert!(tokens.contains(&&b"+"[..]));
        assert!(tokens.contains(&&b"3"[..]));
        assert!(tokens.contains(&&b"1e-3"[..]));
    }
}
