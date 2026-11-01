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
#[derive(Debug, Clone)]
pub struct ParseCheck {
    pub kind: ParseCheckKind,
    pub target: TextSize,
    pub range: TextRange,
    pub location: SourceLocation,
}
