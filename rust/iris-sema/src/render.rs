use crate::Error;
use iris_syntax::{FormatError, ParseError};

/// Keep reported coordinates in the header; locate the excerpt by raw span.
pub fn render_error(error: &Error, bytes: &[u8]) -> String {
    let offset = usize::from(error.range.start()).min(bytes.len());
    let start = bytes[..offset]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |p| p + 1);
    let end = bytes[start..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(bytes.len(), |p| start + p);
    let line = bytes[..start].iter().filter(|&&b| b == b'\n').count() + 1;
    let mut excerpt = String::new();
    let mut column = 0;
    let mut consumed = 0;
    // Lossy decoding is presentation only, including a source UTF-8 error.
    for ch in String::from_utf8_lossy(&bytes[start..end]).chars() {
        if consumed + ch.len_utf8() > 100 {
            break;
        }
        let rendered = match ch {
            '\t' => " ".repeat(8 - excerpt.chars().count() % 8),
            c if c.is_control() => c.escape_default().to_string(),
            c => c.to_string(),
        };
        if start + consumed < offset {
            column += rendered.chars().count();
        }
        excerpt.push_str(&rendered);
        consumed += ch.len_utf8();
    }
    if consumed < end - start {
        excerpt.push_str("...");
    }
    let width = line.to_string().len();
    let header: String = error
        .to_string()
        .chars()
        .map(|ch| {
            if ch.is_control() {
                ch.escape_default().to_string()
            } else {
                ch.to_string()
            }
        })
        .collect();
    format!(
        "{header}\n{line:>width$} | {excerpt}\n{} | {}^",
        " ".repeat(width),
        " ".repeat(column)
    )
}

/// Render syntax-only formatter failures with the same source excerpts as checking.
pub fn render_format_error(error: FormatError, bytes: &[u8], origin: &str) -> String {
    match error {
        FormatError::Source(ParseError::InvalidUtf8(diagnostic)) => {
            render_error(&crate::Error::from(&diagnostic), bytes)
        }
        FormatError::Syntax(diagnostics) => diagnostics
            .iter()
            .map(|diagnostic| render_error(&crate::Error::from(diagnostic), bytes))
            .collect::<Vec<_>>()
            .join("\n"),
        other => format!("{origin}: {other}"),
    }
}
