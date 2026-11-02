use crate::document::{LineIndex, Positions};
use iris_sema::Error;
use lsp_types::{CodeDescription, Diagnostic, DiagnosticSeverity, NumberOrString};

const DIAGNOSTIC_DOCS_BASE_URL: &str = "https://iris.gecode.dev/specification/0.1/errors/";

fn code_description(code: &str) -> CodeDescription {
    CodeDescription {
        href: format!("{DIAGNOSTIC_DOCS_BASE_URL}#{}", code.to_ascii_lowercase())
            .parse()
            .expect("the documentation URL and error code form a valid URI"),
    }
}

pub(crate) fn to_lsp(error: &Error, positions: &Positions) -> Diagnostic {
    Diagnostic {
        range: positions.range(error.range),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(error.code.as_str().into())),
        code_description: Some(code_description(error.code.as_str())),
        source: Some("iris".into()),
        message: error.message.clone(),
        ..Diagnostic::default()
    }
}

pub(crate) fn sample_file(
    message: &str,
    directive: &crate::sample::Directive,
    positions: &crate::document::Positions,
) -> Diagnostic {
    Diagnostic {
        range: positions.range(directive.range),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String("sample-file".into())),
        source: Some("iris".into()),
        message: format!(
            "{}: {}",
            directive.label,
            crate::sample::bounded(message, 3500)
        ),
        ..Diagnostic::default()
    }
}

pub(crate) fn sample_failure(
    error: &iris_core::Failure,
    directive: &crate::sample::Directive,
    source: &str,
    lines: &LineIndex,
    positions: &crate::document::Positions,
) -> Diagnostic {
    use iris_core::Failure;
    let Failure::Language(error) = error else {
        return sample_file(
            &crate::sample::bounded(&error.to_string(), 3500),
            directive,
            positions,
        );
    };
    let span = error.spec_span().or_else(|| {
        error
            .spec_location()
            .and_then(|location| location.line)
            .and_then(|line| lines.physical_line(line))
    });
    let range = match span {
        Some(span)
            if usize::from(span.end()) <= source.len()
                && source.is_char_boundary(usize::from(span.start()))
                && source.is_char_boundary(usize::from(span.end())) =>
        {
            positions.range(span)
        }
        Some(_) => {
            eprintln!("iris lsp: invalid sample spec span");
            positions.range(directive.range)
        }
        None => positions.range(directive.range),
    };
    let mut message = format!(
        "{}: {}",
        directive.label,
        crate::sample::bounded(error.message(), 1400)
    );
    if let Some(location) = error.input_location() {
        message.push_str(&format!(
            "\ninput line {}, column {}",
            location.line.unwrap_or(1),
            location.column.unwrap_or(1)
        ));
        if let Some(excerpt) = error.excerpt() {
            message.push_str(&format!(
                "\n{}\n{}^",
                crate::sample::bounded(excerpt, 500),
                " ".repeat(location.column.unwrap_or(1).saturating_sub(1).min(80))
            ));
        }
    }
    if let Some(context) = error.context() {
        message.push_str("\nwhile reading: ");
        message.push_str(&crate::sample::bounded(context, 800));
    }
    if let Some(path) = error.record_path() {
        message.push_str("\nrecord path:");
        for index in path.iter().take(32) {
            message.push_str(&format!(" [{index}]"));
        }
        if path.len() > 32 {
            message.push_str(" …");
        }
    }
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(error.code.as_str().into())),
        code_description: Some(code_description(error.code.as_str())),
        source: Some("iris".into()),
        message: crate::sample::bounded(&message, 4096),
        ..Diagnostic::default()
    }
}

#[cfg(test)]
mod tests {
    use super::code_description;

    #[test]
    fn documentation_links_use_stable_error_code_anchors() {
        for (code, anchor) in [
            ("E-SPEC-UTF8", "e-spec-utf8"),
            ("E-SPEC-SYNTAX", "e-spec-syntax"),
            ("E-CHECK-FAILED", "e-check-failed"),
            ("E-UTF8", "e-utf8"),
        ] {
            assert_eq!(
                code_description(code).href.as_str(),
                format!("https://iris.gecode.dev/specification/0.1/errors/#{anchor}")
            );
        }
    }
}
