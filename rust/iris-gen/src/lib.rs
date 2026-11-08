//! Generate a spec builder program from a reader spec.
//!
//! [`lower`] turns a checked spec into a language-independent [`Program`], and
//! a [`Backend`] renders that as source in one implementation language. A new
//! language adds a [`Language`] variant, a backend module and one match arm in
//! [`builder_code`].
//!
//! [`view`](view::view) turns a checked spec into a language-independent
//! [`View`](view::View) of typed handles, and [`view_code`] prints it for a
//! language whose [`Language::has_view`] is true: Rust, Python and C++. [`rust_view`] gives the
//! Rust declarations that `view_code` prints as data, for the `iris!` macro.
mod cpp;
mod java;
mod kotlin;
mod lower;
pub mod program;
mod python;
mod rust;
mod typescript;
pub mod view;

use std::fmt;
use std::str::FromStr;

use gecode_iris_sema::Analysis;
use gecode_iris_syntax::Parse;

pub use lower::lower;
use program::Program;
pub use rust::view::{RustHandle, RustView};

/// An implementation language with a builder backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Language {
    Rust,
    Python,
    Cpp,
    Java,
    Kotlin,
    TypeScript,
}

impl Language {
    /// Every language with a backend.
    pub const ALL: &'static [Language] = &[
        Language::Rust,
        Language::Python,
        Language::Cpp,
        Language::Java,
        Language::Kotlin,
        Language::TypeScript,
    ];

    /// The language's name on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::Cpp => "cpp",
            Self::Java => "java",
            Self::Kotlin => "kotlin",
            Self::TypeScript => "typescript",
        }
    }

    /// Whether [`view_code`] has a printer for the language.
    pub fn has_view(self) -> bool {
        match self {
            Self::Rust | Self::Python | Self::Cpp => true,
            Self::Java | Self::Kotlin | Self::TypeScript => false,
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
) -> Result<String, gecode_iris_sema::Error> {
    let program = lower(parsed, analysis)?;
    Ok(match language {
        Language::Rust => rust::RustBackend.render(&program),
        Language::Python => python::PythonBackend.render(&program),
        Language::Cpp => cpp::CppBackend.render(&program),
        Language::Java => java::JavaBackend.render(&program),
        Language::Kotlin => kotlin::KotlinBackend.render(&program),
        Language::TypeScript => typescript::TypeScriptBackend.render(&program),
    })
}

/// Why [`view_code`] printed nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ViewError {
    /// The first error that `iris check` reports.
    Spec(gecode_iris_sema::Error),
    /// The language has no view printer; see [`Language::has_view`].
    NoPrinter(Language),
}

impl fmt::Display for ViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spec(error) => write!(formatter, "{error}"),
            Self::NoPrinter(language) => {
                write!(formatter, "{} has no view generator yet", language.name())
            }
        }
    }
}

impl std::error::Error for ViewError {}

impl From<gecode_iris_sema::Error> for ViewError {
    fn from(error: gecode_iris_sema::Error) -> Self {
        Self::Spec(error)
    }
}

/// A module of the spec's typed handles. A spec that `iris check` rejects gives
/// its first error, and a language without a printer gives
/// [`ViewError::NoPrinter`].
///
/// A Rust or Python module takes its name from the file it is saved in. A C++
/// header puts its handles in a namespace named after the spec's file, the
/// origin of `parsed`: `formats/jobshop/jobshop.iris` gives `jobshop`.
pub fn view_code(
    parsed: &Parse,
    analysis: &Analysis,
    language: Language,
) -> Result<String, ViewError> {
    match language {
        Language::Rust => Ok(rust::view::render(&rust_view(analysis)?)),
        Language::Python => Ok(python::view::render(&view::view(analysis)?)),
        Language::Cpp => Ok(cpp::view::render(&view::view(analysis)?, parsed.origin())),
        Language::Java | Language::Kotlin | Language::TypeScript => {
            Err(ViewError::NoPrinter(language))
        }
    }
}

/// The handles that [`view_code`] prints for Rust, as data, or the first error
/// that `iris check` reports.
pub fn rust_view(analysis: &Analysis) -> Result<RustView, gecode_iris_sema::Error> {
    Ok(RustView::new(&view::view(analysis)?))
}
