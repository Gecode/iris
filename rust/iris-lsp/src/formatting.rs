use crate::{
    document::{Document, PositionEncoding, Positions},
    structure::{respond, response_error},
};
use iris_syntax::{FormatEdit, FormatError, FormatOptions, LanguageVersion, TextRange, TextSize};
use lsp_server::{ErrorCode, Request, Response, ResponseError};
use lsp_types::{
    Range, TextEdit, Uri,
    request::{Formatting, RangeFormatting, Request as LspRequest},
};
use std::{collections::HashMap, path::PathBuf};

#[allow(clippy::mutable_key_type)] // Uri's cache does not change its identity.
pub(crate) fn handle_request(
    request: Request,
    documents: &HashMap<Uri, Document>,
    encoding: PositionEncoding,
) -> Response {
    match request.method.as_str() {
        Formatting::METHOD => respond::<Formatting>(request, |params| {
            let uri = &params.text_document.uri;
            let document = documents
                .get(uri)
                .ok_or_else(|| response_error(ErrorCode::InvalidParams, "document is not open"))?;
            document_edits(document, uri, encoding).map(Some)
        }),
        RangeFormatting::METHOD => respond::<RangeFormatting>(request, |params| {
            let uri = &params.text_document.uri;
            let document = documents
                .get(uri)
                .ok_or_else(|| response_error(ErrorCode::InvalidParams, "document is not open"))?;
            range_edits(document, uri, params.range, encoding).map(Some)
        }),
        _ => unreachable!("the server dispatches only formatting requests here"),
    }
}

fn document_edits(
    document: &Document,
    uri: &Uri,
    encoding: PositionEncoding,
) -> Result<Vec<TextEdit>, ResponseError> {
    let formatted = match iris_syntax::format(
        document.source.as_bytes(),
        uri.as_str(),
        LanguageVersion::V0_1,
        options(uri)?,
    ) {
        Ok(formatted) => formatted,
        Err(FormatError::Syntax(_)) => return Ok(Vec::new()),
        Err(error) => return Err(format_error(error)),
    };
    if formatted == document.source {
        return Ok(Vec::new());
    }
    let end = TextSize::try_from(document.source.len())
        .expect("the formatter validated the source length");
    let positions = Positions::new(&document.source, &document.lines, encoding);
    // One replacement preserves the formatter's exact output, including
    // normalization of physical line endings. No diff heuristic is needed.
    Ok(vec![TextEdit::new(
        positions.range(TextRange::new(TextSize::new(0), end)),
        formatted,
    )])
}

fn range_edits(
    document: &Document,
    uri: &Uri,
    range: Range,
    encoding: PositionEncoding,
) -> Result<Vec<TextEdit>, ResponseError> {
    let bytes = document
        .lines
        .byte_range(&document.source, range, encoding)
        .map_err(|error| response_error(ErrorCode::InvalidParams, error))?;
    let start = TextSize::try_from(bytes.start)
        .map_err(|error| response_error(ErrorCode::InvalidParams, error.to_string()))?;
    let end = TextSize::try_from(bytes.end)
        .map_err(|error| response_error(ErrorCode::InvalidParams, error.to_string()))?;
    let formatted = match iris_syntax::format_range(
        document.source.as_bytes(),
        uri.as_str(),
        LanguageVersion::V0_1,
        TextRange::new(start, end),
        options(uri)?,
    ) {
        Ok(formatted) => formatted,
        Err(FormatError::Syntax(_)) => return Ok(Vec::new()),
        Err(error) => return Err(format_error(error)),
    };
    if formatted.edits.is_empty() {
        return Ok(Vec::new());
    }
    let positions = Positions::new(&document.source, &document.lines, encoding);
    Ok(formatted
        .edits
        .into_iter()
        .map(|edit| {
            let (range, replacement) = whole_line_endings(&document.source, edit);
            TextEdit::new(positions.range(range), replacement)
        })
        .collect())
}

/// Widens an endpoint that falls between the CR and LF of a CRLF pair, which
/// no protocol position can address, and restores the byte it now covers.
fn whole_line_endings(source: &str, edit: FormatEdit) -> (TextRange, String) {
    let bytes = source.as_bytes();
    let splits = |offset: TextSize| {
        let offset = usize::from(offset);
        offset > 0 && bytes[offset - 1] == b'\r' && bytes.get(offset) == Some(&b'\n')
    };
    let mut range = edit.range;
    let mut replacement = edit.replacement;
    if splits(range.start()) {
        range = TextRange::new(range.start() - TextSize::new(1), range.end());
        replacement.insert(0, '\r');
    }
    if splits(range.end()) {
        range = TextRange::new(range.start(), range.end() + TextSize::new(1));
        replacement.push('\n');
    }
    (range, replacement)
}

fn options(uri: &Uri) -> Result<FormatOptions, ResponseError> {
    if !uri
        .scheme()
        .is_some_and(|scheme| scheme.as_str().eq_ignore_ascii_case("file"))
    {
        return Ok(FormatOptions::default());
    }
    let path = file_path(uri)?;
    FormatOptions::for_path(&path).map_err(|error| response_error(ErrorCode::RequestFailed, error))
}

fn file_path(uri: &Uri) -> Result<PathBuf, ResponseError> {
    if uri.authority().is_some_and(|authority| {
        !authority.as_str().is_empty() && !authority.as_str().eq_ignore_ascii_case("localhost")
    }) {
        return Err(response_error(
            ErrorCode::InvalidParams,
            "file URI authority must be empty or localhost",
        ));
    }
    let bytes = uri.path().as_estr().decode().into_bytes();
    // Decode separators too, as url::Url::to_file_path did on Unix. NUL cannot
    // name a file; other non-UTF-8 bytes are valid in Unix paths.
    if !uri.path().is_absolute() || bytes.contains(&0) {
        return Err(response_error(
            ErrorCode::InvalidParams,
            "invalid file URI path",
        ));
    }
    path_from_bytes(&bytes)
        .ok_or_else(|| response_error(ErrorCode::InvalidParams, "unsupported file URI path"))
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> Option<PathBuf> {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    Some(PathBuf::from(OsStr::from_bytes(bytes)))
}

#[cfg(windows)]
fn path_from_bytes(bytes: &[u8]) -> Option<PathBuf> {
    let path = std::str::from_utf8(bytes).ok()?;
    let drive = bytes.get(..4)?;
    if drive[0] != b'/' || !drive[1].is_ascii_alphabetic() || drive[2..] != *b":/" {
        return None;
    }
    // Support local drive paths, refusing invalid characters and alternate
    // streams rather than interpreting unsupported Windows path forms.
    if bytes[4..]
        .iter()
        .any(|byte| byte.is_ascii_control() || b"<>:\"|?*".contains(byte))
    {
        return None;
    }
    Some(PathBuf::from(path[1..].replace('/', "\\")))
}

#[cfg(not(any(unix, windows)))]
fn path_from_bytes(_bytes: &[u8]) -> Option<PathBuf> {
    None
}

fn format_error(error: FormatError) -> ResponseError {
    let code = match error {
        FormatError::InvalidRange => ErrorCode::InvalidParams,
        _ => ErrorCode::RequestFailed,
    };
    response_error(code, error.to_string())
}
