//! Lossless syntax analysis and whitespace-only formatting for Iris 0.1.
//!
//! Syntax acceptance does not include name resolution or type checking.
mod diagnostic;
mod format;
mod lexer;
mod parser;
mod syntax;

use std::{fmt, sync::Arc};

pub use diagnostic::{Diagnostic, DiagnosticCode, DiagnosticPhase, SourceLocation};
pub use format::{FormatEdit, FormatError, FormatOptions, RangeFormat, format, format_range};
pub use rowan::{TextRange, TextSize};
pub use syntax::{IrisLanguage, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageVersion {
    V0_1,
}

#[derive(Debug, Clone)]
pub struct Parse {
    green: rowan::GreenNode,
    diagnostics: Vec<Diagnostic>,
    version: LanguageVersion,
    origin: Arc<str>,
}

impl Parse {
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn version(&self) -> LanguageVersion {
        self.version
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
}

/// Failures that prevent construction of a CST. SourceTooLarge is a host
/// resource limit imposed by rowan's u32 lengths, not a language diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    InvalidUtf8(Diagnostic),
    SourceTooLarge { len: usize },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8(diagnostic) => diagnostic.fmt(f),
            Self::SourceTooLarge { len } => {
                write!(f, "source has {len} bytes, exceeding rowan's u32 capacity")
            }
        }
    }
}
impl std::error::Error for ParseError {}

/// Validate bytes, lex the complete source, then parse statements and patterns.
///
/// Every valid UTF-8 source of at most u32::MAX bytes yields a CST whose text
/// equals the source, even when syntax diagnostics are present. Longer sources
/// return a host resource failure before UTF-8 validation. Invalid UTF-8 yields
/// one source diagnostic and no tree; bytes are never replaced or normalized.
pub fn parse(source: &[u8], origin: &str, version: LanguageVersion) -> Result<Parse, ParseError> {
    check_source_len(source.len())?;
    let origin: Arc<str> = origin.into();
    let source = std::str::from_utf8(source).map_err(|error| {
        let offset = error.valid_up_to();
        let prefix = &source[..offset];
        let line = prefix.iter().filter(|&&byte| byte == b'\n').count() + 1;
        let column = prefix
            .iter()
            .rposition(|&byte| byte == b'\n')
            .map_or(offset + 1, |lf| offset - lf);
        ParseError::InvalidUtf8(Diagnostic {
            code: DiagnosticCode::Utf8,
            message: "reader spec is not valid UTF-8".into(),
            range: diagnostic::text_range(
                offset,
                offset + error.error_len().unwrap_or(source.len() - offset),
            ),
            location: SourceLocation {
                origin: origin.clone(),
                line,
                column: Some(column),
            },
            phase: DiagnosticPhase::Source,
        })
    })?;
    let lexed = lexer::lex(source, origin.clone());
    let (green, diagnostics) = parser::parse(source, origin.clone(), lexed);
    Ok(Parse {
        green,
        diagnostics,
        version,
        origin,
    })
}

fn check_source_len(len: usize) -> Result<(), ParseError> {
    if u32::try_from(len).is_err() {
        Err(ParseError::SourceTooLarge { len })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_size_is_a_host_limit() {
        assert!(super::check_source_len(u32::MAX as usize).is_ok());
        if let Some(len) = (u32::MAX as usize).checked_add(1) {
            assert_eq!(
                super::check_source_len(len),
                Err(super::ParseError::SourceTooLarge { len })
            );
        }
    }
}
