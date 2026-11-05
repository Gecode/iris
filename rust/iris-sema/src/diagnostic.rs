use gecode_iris_syntax::{Diagnostic, DiagnosticCode, SourceLocation, TextRange};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    Syntax(DiagnosticCode),
    NameConflict,
    UnknownType,
    Type,
    RecordScope,
    Field,
    RecordValue,
    CheckScope,
}
impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Syntax(code) => code.as_str(),
            Self::NameConflict => "E-SPEC-NAME-CONFLICT",
            Self::UnknownType => "E-SPEC-UNKNOWN-TYPE",
            Self::Type => "E-SPEC-TYPE",
            Self::RecordScope => "E-SPEC-RECORD-SCOPE",
            Self::Field => "E-SPEC-FIELD",
            Self::RecordValue => "E-SPEC-RECORD-VALUE",
            Self::CheckScope => "E-SPEC-CHECK-SCOPE",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
    pub location: SourceLocation,
    pub range: TextRange,
}
impl From<&Diagnostic> for Error {
    fn from(diagnostic: &Diagnostic) -> Self {
        Self {
            code: ErrorCode::Syntax(diagnostic.code),
            message: diagnostic.message.clone(),
            location: diagnostic.location.clone(),
            range: diagnostic.range,
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:", self.location.origin, self.location.line)?;
        if let Some(column) = self.location.column {
            write!(f, "{column}:")?;
        }
        write!(f, " {}", self.message)
    }
}
impl std::error::Error for Error {}
