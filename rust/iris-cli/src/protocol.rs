//! Driver JSON transport. No language decision is made by serialization.
use iris_sema::Error;
use serde::{
    Deserialize, Serialize,
    de::{self, MapAccess, Visitor},
};
use std::{collections::HashSet, fmt};

#[derive(Serialize)]
struct Location<'a> {
    origin: &'a str,
    line: usize,
    column: Option<usize>,
}
#[derive(Serialize)]
struct CanonicalError<'a> {
    code: &'static str,
    message: &'a str,
    input_location: Option<()>,
    spec_location: Location<'a>,
    context: Option<()>,
    record_path: Option<()>,
}
pub fn error_json(error: &Error) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&CanonicalError {
        code: error.code.as_str(),
        message: &error.message,
        input_location: None,
        spec_location: Location {
            origin: &error.location.origin,
            line: error.location.line,
            column: error.location.column,
        },
        context: None,
        record_path: None,
    })
}
pub const PARSE_SUCCESS: &[u8] = b"{\"iris\":\"0.1\",\"parsed\":true}\n";

struct Parameters;
impl<'de> Deserialize<'de> for Parameters {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ParameterVisitor;
        impl<'de> Visitor<'de> for ParameterVisitor {
            type Value = Parameters;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an empty string-to-string parameter object")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Parameters, M::Error> {
                let mut names = HashSet::new();
                while let Some((name, _value)) = map.next_entry::<String, String>()? {
                    if !names.insert(name) {
                        return Err(de::Error::custom("duplicate parameter"));
                    }
                }
                if !names.is_empty() {
                    return Err(de::Error::custom("parse does not accept caller parameters"));
                }
                Ok(Parameters)
            }
        }
        deserializer.deserialize_map(ParameterVisitor)
    }
}
pub fn validate_parse_params(text: &str) -> Result<(), serde_json::Error> {
    serde_json::from_str::<Parameters>(text).map(|_| ())
}
