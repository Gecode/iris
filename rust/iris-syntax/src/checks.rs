//! Ordered boundaries between syntax parsing and semantic validation.
use crate::{SourceLocation, TextRange, TextSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseCheckKind {
    Diagnostic(usize),
    TypeDefinitionName,
    TypeDefinitionComplete,
    TypeUse,
    ReadForm,
    Layout,
    Offset,
    CaptureForm,
    BindingName,
    ComprehensionComplete,
}

/// `target` identifies the production's first raw source byte. `range` and
/// `location` identify the current token when this boundary was reached.
/// Capture locations can be virtual; raw ranges always refer to the source.
/// That token can lie past the production, so `owner` names the first raw byte
/// of the statement or section arm being parsed.
#[derive(Debug, Clone)]
pub struct ParseCheck {
    pub kind: ParseCheckKind,
    pub target: TextSize,
    pub owner: TextSize,
    pub range: TextRange,
    pub location: SourceLocation,
}
