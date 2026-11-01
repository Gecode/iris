//! Driver JSON transport. No language decision is made by serialization.
use iris_sema::Error;
use serde::{
    Deserialize,
    de::{self, MapAccess, Visitor},
};
use std::{collections::HashSet, fmt};

pub fn error_json(error: &Error) -> Result<Vec<u8>, serde_json::Error> {
    iris_core::Error::from_spec(error).to_json()
}
pub const PARSE_SUCCESS: &[u8] = b"{\"iris\":\"0.1\",\"parsed\":true}\n";

struct Parameters(iris_core::Params);
impl<'de> Deserialize<'de> for Parameters {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ParameterVisitor;
        impl<'de> Visitor<'de> for ParameterVisitor {
            type Value = Parameters;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a string-to-string parameter object")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Parameters, M::Error> {
                let mut names = HashSet::new();
                let mut params = iris_core::Params::new();
                while let Some((name, value)) = map.next_entry::<String, String>()? {
                    if !names.insert(name.clone()) {
                        return Err(de::Error::custom("duplicate parameter"));
                    }
                    params.set(name, value);
                }
                Ok(Parameters(params))
            }
        }
        deserializer.deserialize_map(ParameterVisitor)
    }
}
pub fn parse_params(text: &str) -> Result<iris_core::Params, serde_json::Error> {
    serde_json::from_str::<Parameters>(text).map(|params| params.0)
}
pub fn validate_parse_params(text: &str) -> Result<(), serde_json::Error> {
    let params = parse_params(text)?;
    if params.pairs().is_empty() {
        Ok(())
    } else {
        Err(<serde_json::Error as de::Error>::custom(
            "parse does not accept caller parameters",
        ))
    }
}
