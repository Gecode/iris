use crate::call_site::CallSite;
use crate::input::{Input, Position};
use gecode_iris_syntax::{TextRange, TextSize};
use serde::Serialize;
use std::{fmt, ops::Range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    Spec(gecode_iris_sema::ErrorCode),
    ParamUnknown,
    ParamMissing,
    ParamType,
    InstanceEmpty,
    InstanceNotFound,
    InstanceNumber,
    InstanceSelection,
    Utf8,
    ExpectedValue,
    RangeBounds,
    TypeUndefined,
    NameUndefined,
    CheckFailed,
    CheckRecords,
    FieldUndefined,
    IntegerType,
    ComparisonType,
    DivisionZero,
    Exponent,
    IntegerOverflow,
    FunctionType,
    FunctionEmpty,
    TrailingInput,
    Count,
    SizeLimit,
    Budget,
    Layout,
    ListTerminator,
    OffsetType,
    Symbol,
    ExpectedToken,
    IndexScalar,
    IndexType,
    IndexRange,
    ValueTypeMix,
    RepeatTerminator,
    NoProgress,
    RepeatMissing,
    RepeatDepth,
    IndexContext,
    IndexMismatch,
    LineMatch,
    FindMatch,
    SkipEof,
    PatternReference,
    SectionsRepeated,
    SectionsUnexpected,
}
impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spec(code) => code.as_str(),
            Self::CheckFailed => "E-CHECK-FAILED",
            Self::CheckRecords => "E-CHECK-RECORDS",
            Self::FieldUndefined => "E-FIELD-UNDEFINED",
            Self::LineMatch => "E-LINE-MATCH",
            Self::FindMatch => "E-FIND-MATCH",
            Self::SkipEof => "E-SKIP-EOF",
            Self::PatternReference => "E-PATTERN-REFERENCE",
            Self::SectionsRepeated => "E-SECTIONS-REPEATED",
            Self::SectionsUnexpected => "E-SECTIONS-UNEXPECTED",
            Self::ParamMissing => "E-PARAM-MISSING",
            Self::ParamType => "E-PARAM-TYPE",
            Self::InstanceEmpty => "E-INSTANCE-EMPTY",
            Self::InstanceNotFound => "E-INSTANCE-NOT-FOUND",
            Self::InstanceNumber => "E-INSTANCE-NUMBER",
            Self::InstanceSelection => "E-INSTANCE-SELECTION",
            Self::ParamUnknown => "E-PARAM-UNKNOWN",
            Self::Utf8 => "E-UTF8",
            Self::ExpectedValue => "E-EXPECTED-VALUE",
            Self::RangeBounds => "E-RANGE-BOUNDS",
            Self::TypeUndefined => "E-TYPE-UNDEFINED",
            Self::NameUndefined => "E-NAME-UNDEFINED",
            Self::IntegerType => "E-INTEGER-TYPE",
            Self::ComparisonType => "E-COMPARISON-TYPE",
            Self::DivisionZero => "E-DIVISION-ZERO",
            Self::Exponent => "E-EXPONENT",
            Self::IntegerOverflow => "E-INTEGER-OVERFLOW",
            Self::FunctionType => "E-FUNCTION-TYPE",
            Self::FunctionEmpty => "E-FUNCTION-EMPTY",
            Self::Count => "E-COUNT",
            Self::SizeLimit => "E-SIZE-LIMIT",
            Self::Budget => "E-BUDGET",
            Self::Layout => "E-LAYOUT",
            Self::ListTerminator => "E-LIST-TERMINATOR",
            Self::OffsetType => "E-OFFSET-TYPE",
            Self::Symbol => "E-SYMBOL",
            Self::ExpectedToken => "E-EXPECTED-TOKEN",
            Self::IndexScalar => "E-INDEX-SCALAR",
            Self::IndexType => "E-INDEX-TYPE",
            Self::IndexRange => "E-INDEX-RANGE",
            Self::RepeatTerminator => "E-REPEAT-TERMINATOR",
            Self::NoProgress => "E-NO-PROGRESS",
            Self::RepeatMissing => "E-REPEAT-MISSING",
            Self::RepeatDepth => "E-REPEAT-DEPTH",
            Self::IndexContext => "E-INDEX-CONTEXT",
            Self::IndexMismatch => "E-INDEX-MISMATCH",
            Self::ValueTypeMix => "E-VALUE-TYPE-MIX",
            Self::TrailingInput => "E-TRAILING-INPUT",
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Location {
    pub origin: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}
#[derive(Debug, Clone)]
pub struct InputSpan {
    pub bytes: Range<usize>,
}
#[derive(Debug, Clone)]
pub struct Error {
    pub code: ErrorCode,
    pub(crate) details: Box<Details>,
}
#[derive(Debug, Clone)]
pub(crate) struct Details {
    pub message: String,
    pub earlier_rejection: Option<String>,
    pub input_location: Option<Location>,
    pub spec_location: Option<Location>,
    pub context: Option<String>,
    pub record_path: Option<Vec<usize>>,
    pub input_span: Option<InputSpan>,
    pub spec_span: Option<TextRange>,
    /// Where the statement this error belongs to starts, when `spec_span` does
    /// not show it: the span can lie past that statement.
    pub owner: Option<TextSize>,
    pub excerpt: Option<String>,
    pub call_site: Option<CallSite>,
}
impl Error {
    fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            details: Box::new(Details {
                message: message.into(),
                earlier_rejection: None,
                input_location: None,
                spec_location: None,
                context: None,
                record_path: None,
                input_span: None,
                spec_span: None,
                owner: None,
                excerpt: None,
                call_site: None,
            }),
        }
    }
    pub fn from_spec(error: &gecode_iris_sema::Error) -> Self {
        let mut result = Self::new(ErrorCode::Spec(error.code), error.message.clone());
        result.details.spec_location = Some(Location {
            origin: Some(error.location.origin.to_string()),
            line: Some(error.location.line),
            column: error.location.column,
        });
        result.details.spec_span = Some(error.range);
        result.details.owner = error.owner;
        result
    }
    pub(crate) fn unknown_param(name: &str, origin: &str) -> Self {
        let mut result = Self::new(
            ErrorCode::ParamUnknown,
            format!("the spec has no parameter '{name}'"),
        );
        result.details.spec_location = Some(Location {
            origin: Some(origin.into()),
            line: None,
            column: None,
        });
        result
    }
    pub(crate) fn input(
        code: ErrorCode,
        message: impl Into<String>,
        input: &Input<'_>,
        position: Position,
        statement: Option<(&str, usize, TextRange)>,
        context: Option<&str>,
    ) -> Self {
        let mut result = Self::new(code, message);
        result.details.input_location = Some(Location {
            origin: Some(input.origin.into()),
            line: Some(position.line),
            column: Some(position.column),
        });
        result.details.input_span = Some(InputSpan {
            bytes: position.offset..position.offset,
        });
        result.details.excerpt = Some(excerpt(input.bytes, position));
        if let Some((origin, line, range)) = statement {
            result.details.spec_location = Some(Location {
                origin: Some(origin.into()),
                line: Some(line),
                column: None,
            });
            result.details.spec_span = Some(range);
        }
        result.details.context = context.map(str::to_owned);
        result
    }
    pub fn message(&self) -> &str {
        &self.details.message
    }
    pub fn input_location(&self) -> Option<&Location> {
        self.details.input_location.as_ref()
    }
    pub fn spec_location(&self) -> Option<&Location> {
        self.details.spec_location.as_ref()
    }
    pub fn input_span(&self) -> Option<&InputSpan> {
        self.details.input_span.as_ref()
    }
    pub fn excerpt(&self) -> Option<&str> {
        self.details.excerpt.as_deref()
    }
    pub fn context(&self) -> Option<&str> {
        self.details.context.as_deref()
    }
    pub fn record_path(&self) -> Option<&[usize]> {
        self.details.record_path.as_deref()
    }
    pub fn spec_span(&self) -> Option<TextRange> {
        self.details.spec_span
    }
    /// The host code that wrote the statement, for a spec from a builder.
    pub fn call_site(&self) -> Option<CallSite> {
        self.details.call_site
    }
    pub(crate) fn set_input_end(&mut self, end: usize) {
        if let Some(span) = &mut self.details.input_span {
            span.bytes.end = end;
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let location = self
            .details
            .input_location
            .as_ref()
            .or(self.details.spec_location.as_ref());
        if let Some(location) = location {
            if let Some(origin) = &location.origin {
                write!(f, "{}:", safe_text(origin))?;
            }
            if let Some(line) = location.line {
                write!(f, "{line}:")?;
            }
            if let Some(column) = location.column {
                write!(f, "{column}:")?;
            }
            write!(f, " ")?;
        }
        write!(f, "{}", safe_text(&self.details.message))?;
        if let Some(location) = &self.details.input_location {
            if let Some(excerpt) = &self.details.excerpt {
                write!(f, "\n{:>6} | {excerpt}", location.line.unwrap_or(1))?;
            }
            write!(
                f,
                "\n       | {}^",
                " ".repeat(location.column.unwrap_or(1) - 1)
            )?;
        }
        if let Some(context) = &self.details.context {
            write!(f, "\nwhile reading: {}", safe_text(context))?;
        }
        if self.details.input_location.is_some()
            && let Some(location) = &self.details.spec_location
        {
            write!(
                f,
                "\nspec: {}",
                safe_text(location.origin.as_deref().unwrap_or(""))
            )?;
            if let Some(line) = location.line {
                write!(f, ":{line}")?;
            }
        }
        if let Some(site) = self.details.call_site {
            write!(f, "\ncall site: {}", safe_text(&site.to_string()))?;
        }
        if let Some(summary) = &self.details.earlier_rejection {
            write!(
                f,
                "\n  after an earlier error in the instance: {}",
                safe_text(summary)
            )?;
        }
        Ok(())
    }
}
impl std::error::Error for Error {}
fn excerpt(bytes: &[u8], position: Position) -> String {
    let start = position.offset - (position.column - 1);
    let remaining = &bytes[start..bytes.len().min(start + 101)];
    let length = remaining
        .iter()
        .position(|&b| b == b'\n')
        .unwrap_or(remaining.len());
    let mut text = safe_text(&String::from_utf8_lossy(&remaining[..length.min(100)]));
    if length > 100 {
        text.push_str("...");
    }
    text
}
fn safe_text(text: &str) -> String {
    let mut escaped = String::new();
    for character in text.chars() {
        if character.is_control() {
            escaped.extend(character.escape_default());
        } else {
            escaped.push(character);
        }
    }
    escaped
}
