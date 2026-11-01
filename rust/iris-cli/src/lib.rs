//! Shared byte-oriented parse/check boundary for the user CLI and test driver.
mod diagnostic;
pub mod protocol;
pub use diagnostic::render_error;
use iris_sema::Analysis;
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
    use iris_syntax::SyntaxKind as K;
    parsed
        .syntax()
        .children()
        .filter(|n| n.kind() == K::Format)
        .filter_map(|node| {
            let tokens: Vec<_> = node
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| !matches!(t.kind(), K::Whitespace | K::Comment))
                .skip(1)
                .collect();
            let first = tokens.first()?;
            if first.kind() == K::String {
                Some(iris_syntax::DecodedString::new(first.text()).text)
            } else {
                Some(tokens.iter().map(|t| t.text()).collect())
            }
        })
        .last()
}
