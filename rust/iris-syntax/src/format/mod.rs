//! Whitespace-only layout of the lossless syntax tree.
mod expressions;
mod layout;
mod range;
mod statements;
mod trivia;

use std::fmt;

use crate::{Diagnostic, LanguageVersion, ParseError, TextRange, parse};
use layout::Layout;

pub use range::format_range;

/// The formatter's only setting. Width counts Unicode scalar values and is a
/// soft target: protected text and indivisible groups may exceed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatOptions {
    pub line_width: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self { line_width: 100 }
    }
}

/// A replacement in half-open byte coordinates of the original source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatEdit {
    pub range: TextRange,
    pub replacement: String,
}

/// Range formatting widens to complete statements. Every edit is contained in
/// `range`; an already formatted selection has no edits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeFormat {
    pub range: TextRange,
    pub edits: Vec<FormatEdit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    Source(ParseError),
    Syntax(Vec<Diagnostic>),
    InvalidWidth,
    InvalidRange,
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(f),
            Self::Syntax(diagnostics) => write!(
                f,
                "cannot format reader spec: {} syntax diagnostic(s)",
                diagnostics.len()
            ),
            Self::InvalidWidth => f.write_str("formatter line width must be positive"),
            Self::InvalidRange => {
                f.write_str("format range is outside the source or splits a UTF-8 scalar")
            }
        }
    }
}

impl std::error::Error for FormatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            _ => None,
        }
    }
}

/// Format a complete reader spec, refusing source failures and all syntax
/// diagnostics without returning partial output. Only whitespace outside
/// strings, line patterns and comments changes. Nonempty output ends in LF.
///
/// Output style changes only in minor package releases, with a changelog entry.
pub fn format(
    source: &[u8],
    origin: &str,
    version: LanguageVersion,
    options: FormatOptions,
) -> Result<String, FormatError> {
    check_options(options)?;
    let parsed = parse(source, origin, version).map_err(FormatError::Source)?;
    if !parsed.diagnostics().is_empty() {
        return Err(FormatError::Syntax(parsed.diagnostics().to_vec()));
    }
    let source = std::str::from_utf8(source).expect("parse validated UTF-8");
    let root = parsed.syntax();
    let mut layout = Layout::new(source, &root, options.line_width);
    layout.format_document(&root);
    Ok(layout.render_document())
}

fn check_options(options: FormatOptions) -> Result<(), FormatError> {
    if options.line_width == 0 {
        Err(FormatError::InvalidWidth)
    } else {
        Ok(())
    }
}
