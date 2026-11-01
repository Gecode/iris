use std::{fmt, sync::Arc};

use rowan::{TextRange, TextSize};

/// Codes owned by source validation and syntax analysis (Specification §11.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCode {
    Utf8,
    String,
    Number,
    Character,
    Pattern,
    Syntax,
    Placement,
    Name,
    Nesting,
    Function,
    Constant,
}

impl DiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Utf8 => "E-SPEC-UTF8",
            Self::String => "E-SPEC-STRING",
            Self::Number => "E-SPEC-NUMBER",
            Self::Character => "E-SPEC-CHARACTER",
            Self::Pattern => "E-SPEC-PATTERN",
            Self::Syntax => "E-SPEC-SYNTAX",
            Self::Placement => "E-SPEC-PLACEMENT",
            Self::Name => "E-SPEC-NAME",
            Self::Nesting => "E-SPEC-NESTING",
            Self::Function => "E-SPEC-FUNCTION",
            Self::Constant => "E-SPEC-CONSTANT",
        }
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticPhase {
    Source,
    Lexing,
    Parsing,
}

/// Reported coordinates differ from raw spans: ordinary errors have no column,
/// and capture types can report a virtual line introduced by a decoded escape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub origin: Arc<str>,
    pub line: usize,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub message: String,
    pub range: TextRange,
    pub location: SourceLocation,
    pub phase: DiagnosticPhase,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:", self.location.origin, self.location.line)?;
        if let Some(column) = self.location.column {
            write!(f, "{column}:")?;
        }
        write!(f, " {}: {}", self.code, self.message)
    }
}

pub(crate) fn text_range(start: usize, end: usize) -> TextRange {
    // The public entry point checks the complete source length before parsing.
    TextRange::new(
        TextSize::new(u32::try_from(start).expect("validated source size")),
        TextSize::new(u32::try_from(end).expect("validated source size")),
    )
}
