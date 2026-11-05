//! Generate a spec builder program from a reader spec.
//!
//! [`lower`] turns a checked spec into a language-independent [`Program`], and
//! a [`Backend`] renders that as source in one implementation language. A new
//! language adds a [`Language`] variant, a backend module and one match arm in
//! [`builder_code`].
mod lower;
pub mod program;
mod rust;

use std::fmt;
use std::str::FromStr;

use iris_sema::Analysis;
use iris_syntax::Parse;

pub use lower::lower;
use program::Program;

/// An implementation language with a builder backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Language {
    Rust,
}

impl Language {
    /// Every language with a backend.
    pub const ALL: &'static [Language] = &[Language::Rust];

    /// The language's name on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
        }
    }
}

/// A language name that no backend has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownLanguage(pub String);

impl fmt::Display for UnknownLanguage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown language '{}'", self.0)
    }
}

impl std::error::Error for UnknownLanguage {}

impl FromStr for Language {
    type Err = UnknownLanguage;

    fn from_str(name: &str) -> Result<Self, UnknownLanguage> {
        Self::ALL
            .iter()
            .copied()
            .find(|language| language.name() == name)
            .ok_or_else(|| UnknownLanguage(name.into()))
    }
}

/// Renders a [`Program`] as source in one language.
pub trait Backend {
    fn render(&self, program: &Program) -> String;
}

/// The builder program of a spec, or the first error that `iris check` reports.
pub fn builder_code(
    parsed: &Parse,
    analysis: &Analysis,
    language: Language,
) -> Result<String, iris_sema::Error> {
    let program = lower(parsed, analysis)?;
    Ok(match language {
        Language::Rust => rust::RustBackend.render(&program),
    })
}
