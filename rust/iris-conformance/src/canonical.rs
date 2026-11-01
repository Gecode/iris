//! Boundary validation for Specification chapter 3 and the canonical schemas.
use std::collections::HashSet;

use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Data(Data),
    Parse,
    Dzn(Vec<u8>),
    Error(ErrorKey),
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OutcomeKind {
    Data,
    Parse,
    Dzn,
    Error,
}

impl Outcome {
    pub fn decode(kind: OutcomeKind, bytes: &[u8], version: &str) -> Result<Self, String> {
        match kind {
            OutcomeKind::Data => decode_data(bytes, version).map(Self::Data),
            OutcomeKind::Parse => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct ParseSuccess {
                    iris: String,
                    parsed: bool,
                }
                let success: ParseSuccess = decode_json(bytes)?;
                if success.iris != version || !success.parsed {
                    return Err(
                        "parse success must contain selected iris version and parsed=true".into(),
                    );
                }
                Ok(Self::Parse)
            }
            OutcomeKind::Dzn => Ok(Self::Dzn(bytes.to_vec())),
            OutcomeKind::Error => decode_error(bytes).map(Self::Error),
        }
    }

    pub fn kind(&self) -> OutcomeKind {
        match self {
            Self::Data(_) => OutcomeKind::Data,
            Self::Parse => OutcomeKind::Parse,
            Self::Dzn(_) => OutcomeKind::Dzn,
            Self::Error(_) => OutcomeKind::Error,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Data {
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    pub value_type: ValueType,
    pub depth: usize,
    // Preorder tokens avoid recursive ownership and an artificial nesting limit.
    pub value: Vec<ValueToken>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ValueType {
    Int,
    Float,
    String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ValueToken {
    Array(usize),
    Int(i64),
    Float(String),
    String(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawData {
    iris: String,
    entries: Vec<RawEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    name: String,
    #[serde(rename = "type")]
    value_type: ValueType,
    depth: usize,
    value: Box<RawValue>,
}

fn decode_data(bytes: &[u8], version: &str) -> Result<Data, String> {
    let raw: RawData = decode_json(bytes)?;
    if raw.iris != version {
        return Err(format!(
            "iris version: expected {version:?}, got {:?}",
            raw.iris
        ));
    }
    let mut names = HashSet::new();
    let mut entries = Vec::new();
    for entry in raw.entries {
        if !names.insert(entry.name.clone()) {
            return Err(format!("duplicate entry name {:?}", entry.name));
        }
        let value = decode_value(&entry.value, entry.depth, entry.value_type)
            .map_err(|error| format!("entry {:?}: {error}", entry.name))?;
        entries.push(Entry {
            name: entry.name,
            value_type: entry.value_type,
            depth: entry.depth,
            value,
        });
    }
    Ok(Data { entries })
}

fn decode_value(
    raw: &RawValue,
    depth: usize,
    value_type: ValueType,
) -> Result<Vec<ValueToken>, String> {
    let mut pending = vec![(raw, depth)];
    let mut tokens = Vec::new();
    while let Some((value, remaining)) = pending.pop() {
        if remaining > 0 {
            let children: Vec<&RawValue> =
                decode_json(value.get().as_bytes()).map_err(|error| {
                    format!("expected array at remaining depth {remaining}: {error}")
                })?;
            tokens.push(ValueToken::Array(children.len()));
            pending.extend(
                children
                    .into_iter()
                    .rev()
                    .map(|child| (child, remaining - 1)),
            );
        } else {
            let token = match value_type {
                ValueType::Int => {
                    let number: i64 = decode_json(value.get().as_bytes())?;
                    if number.to_string() != value.get() {
                        return Err(format!("noncanonical integer {}", value.get()));
                    }
                    ValueToken::Int(number)
                }
                ValueType::Float => {
                    let number: f64 = decode_json(value.get().as_bytes())?;
                    if !number.is_finite() || canonical_float(number) != value.get() {
                        return Err(format!("noncanonical float {}", value.get()));
                    }
                    ValueToken::Float(value.get().into())
                }
                ValueType::String => ValueToken::String(decode_json(value.get().as_bytes())?),
            };
            tokens.push(token);
        }
    }
    Ok(tokens)
}

/// Render Ryū's shortest significand using the fixed/scientific choice in §3.5.
fn canonical_float(number: f64) -> String {
    if number == 0.0 {
        return if number.is_sign_negative() {
            "-0.0"
        } else {
            "0"
        }
        .into();
    }
    let mut buffer = ryu::Buffer::new();
    let rendered = buffer.format_finite(number.abs());
    let (mantissa, exponent) = rendered.split_once('e').unwrap_or((rendered, "0"));
    let exponent: i32 = exponent.parse().expect("Ryū exponent is an integer");
    let point = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let mut digits = mantissa.replace('.', "");
    let leading = digits.bytes().take_while(|byte| *byte == b'0').count();
    digits.drain(..leading);
    let decimal_point = point + exponent - leading as i32;
    while digits.ends_with('0') {
        digits.pop();
    }
    let scientific_exponent = decimal_point - 1;
    let scientific = if digits.len() == 1 {
        format!("{digits}e{scientific_exponent}")
    } else {
        format!("{}.{}e{scientific_exponent}", &digits[..1], &digits[1..])
    };
    let fixed = if decimal_point <= 0 {
        format!("0.{}{digits}", "0".repeat((-decimal_point) as usize))
    } else if decimal_point as usize >= digits.len() {
        format!(
            "{digits}{}",
            "0".repeat(decimal_point as usize - digits.len())
        )
    } else {
        let split = decimal_point as usize;
        format!("{}.{}", &digits[..split], &digits[split..])
    };
    let result = if fixed.len() <= scientific.len() {
        fixed
    } else {
        scientific
    };
    if number.is_sign_negative() {
        format!("-{result}")
    } else {
        result
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ErrorKey {
    pub code: String,
    pub line: Option<u64>,
    pub column: Option<u64>,
    pub record_path: Option<Vec<u64>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorDocument {
    code: String,
    #[serde(rename = "message")]
    _message: String,
    #[serde(deserialize_with = "required_nullable")]
    input_location: Option<Location>,
    #[serde(deserialize_with = "required_nullable")]
    spec_location: Option<Location>,
    #[serde(deserialize_with = "required_nullable")]
    #[serde(rename = "context")]
    _context: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    record_path: Option<Vec<u64>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Location {
    #[serde(deserialize_with = "required_nullable")]
    #[serde(rename = "origin")]
    _origin: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    line: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    column: Option<u64>,
}

// Explicit deserialize_with prevents Serde from defaulting a missing Option field.
fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::deserialize(deserializer)
}

fn decode_error(bytes: &[u8]) -> Result<ErrorKey, String> {
    let document: ErrorDocument = decode_json(bytes)?;
    let registry = include_str!("../../../spec/0.1/errors.md");
    let registered = registry.lines().any(|line| {
        line.strip_prefix("| `")
            .and_then(|tail| tail.split_once('`'))
            .is_some_and(|(code, _)| code == document.code)
    });
    if document.code == "E-UNMAPPED" {
        return Err("E-UNMAPPED is a hard adapter failure".into());
    }
    if !registered {
        return Err(format!("unregistered error code {:?}", document.code));
    }
    for location in [&document.input_location, &document.spec_location]
        .into_iter()
        .flatten()
    {
        if location.line == Some(0) || location.column == Some(0) {
            return Err("error coordinates must be positive or null".into());
        }
    }
    let primary = document.input_location.or(document.spec_location);
    Ok(ErrorKey {
        code: document.code,
        line: primary.as_ref().and_then(|location| location.line),
        column: primary.and_then(|location| location.column),
        record_path: document.record_path,
    })
}

pub(crate) fn decode_json<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, String> {
    serde_json::from_slice(bytes).map_err(|error| error.to_string())
}

pub(crate) fn validate_params(text: &str) -> Result<bool, String> {
    // A typed entry visitor preserves source order while checking duplicates.
    struct ParamsVisitor;
    impl<'de> serde::de::Visitor<'de> for ParamsVisitor {
        type Value = bool;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a string-to-string parameter object")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(self, mut map: M) -> Result<bool, M::Error> {
            let mut names = HashSet::new();
            while let Some((name, _value)) = map.next_entry::<String, String>()? {
                if !names.insert(name) {
                    return Err(serde::de::Error::custom("duplicate parameter name"));
                }
            }
            Ok(names.is_empty())
        }
    }
    let mut decoder = serde_json::Deserializer::from_str(text);
    let empty = decoder
        .deserialize_map(ParamsVisitor)
        .map_err(|error| error.to_string())?;
    decoder.end().map_err(|error| error.to_string())?;
    Ok(empty)
}
