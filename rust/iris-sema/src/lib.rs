//! Parse-time validity and conservative semantic facts for Iris reader specs.
mod check;
mod diagnostic;
mod environment;
mod infer;
mod model;
mod render;
mod resolve;
pub use render::{render_error, render_format_error};
mod syntax;
mod targets;
mod work;

pub use diagnostic::{Error, ErrorCode};
use iris_syntax::{DiagnosticPhase, Parse};
pub use model::*;

#[derive(Debug)]
pub struct Analysis {
    pub model: SemanticModel,
    pub diagnostics: Vec<Error>,
}
impl Analysis {
    pub fn first_error(&self) -> Option<&Error> {
        self.diagnostics.first()
    }
    pub fn is_valid(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// Apply Specification §11.2 without executing expressions or reading input.
/// IDs and cloned CST handles belong to this analysis snapshot.
pub fn analyze(parsed: &Parse) -> Analysis {
    let mut model = syntax::collect(&parsed.syntax());
    let index = syntax::SyntaxIndex::new(&model);
    let error = parsed
        .diagnostics()
        .iter()
        .find(|d| d.phase == DiagnosticPhase::Lexing)
        .map(Error::from)
        .or_else(|| check::check(parsed, &mut model, &index).err())
        .or_else(|| resolve::resolve(parsed, &mut model, &index).err());
    if error.is_none() {
        infer::infer(&mut model, &index);
    }
    Analysis {
        model,
        diagnostics: error.into_iter().collect(),
    }
}
