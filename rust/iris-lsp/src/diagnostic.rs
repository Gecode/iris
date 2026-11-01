use crate::document::{LineIndex, PositionEncoding};
use iris_sema::{Error, ErrorCode};
use iris_syntax::DiagnosticCode;
use lsp_types::{CodeDescription, Diagnostic, DiagnosticSeverity, NumberOrString, Range};

const DIAGNOSTIC_DOCS_BASE_URL: &str = "https://github.com/Gecode/iris/blob/main/spec/0.1/";

pub(crate) fn to_lsp(
    error: &Error,
    source: &str,
    lines: &LineIndex,
    encoding: PositionEncoding,
) -> Result<Diagnostic, &'static str> {
    let section = match error.code {
        ErrorCode::Syntax(DiagnosticCode::Utf8) => "116-iris-only-errors",
        _ => "114-parse-time-codes",
    };
    Ok(Diagnostic {
        range: Range::new(
            lines.position(source, u32::from(error.range.start()) as usize, encoding)?,
            lines.position(source, u32::from(error.range.end()) as usize, encoding)?,
        ),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(error.code.as_str().into())),
        code_description: Some(CodeDescription {
            href: format!("{DIAGNOSTIC_DOCS_BASE_URL}errors.md#{section}")
                .parse()
                .expect("the documentation URL constant and section form a valid URI"),
        }),
        source: Some("iris".into()),
        message: error.message.clone(),
        ..Diagnostic::default()
    })
}
