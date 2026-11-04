//! The `iris` command line as a library, and the shared byte-oriented
//! parse/check boundary for the user CLI and test driver.
mod command;
pub mod protocol;
pub use command::run;
use iris_sema::Analysis;
pub use iris_sema::render_error;
use iris_syntax::{LanguageVersion, Parse, ParseError};

#[derive(Debug)]
pub struct CheckedSpec {
    pub parsed: Parse,
    pub analysis: Analysis,
}
#[derive(Debug)]
pub enum CheckFailure {
    Language(iris_sema::Error),
    Host(ParseError),
}
impl std::fmt::Display for CheckFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Language(error) => error.fmt(f),
            Self::Host(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for CheckFailure {}
pub fn check_spec(bytes: &[u8], origin: &str) -> Result<CheckedSpec, CheckFailure> {
    let parsed = iris_syntax::parse(bytes, origin, LanguageVersion::V0_1).map_err(|e| match e {
        ParseError::InvalidUtf8(d) => CheckFailure::Language(iris_sema::Error::from(&d)),
        other => CheckFailure::Host(other),
    })?;
    let analysis = iris_sema::analyze(&parsed);
    if let Some(error) = analysis.first_error() {
        return Err(CheckFailure::Language(error.clone()));
    }
    Ok(CheckedSpec { parsed, analysis })
}

pub fn format_name(parsed: &Parse) -> Option<String> {
    parsed.format_name()
}
