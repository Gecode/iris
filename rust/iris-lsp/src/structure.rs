use crate::{
    document::{Document, PositionEncoding, Positions},
    semantic_tokens,
};
use iris_syntax::{SyntaxKind as K, SyntaxNode, TextRange};
use lsp_server::{ErrorCode, Request, Response, ResponseError};
use lsp_types::{
    DocumentSymbol, DocumentSymbolResponse, FoldingRange, FoldingRangeKind, Location,
    SymbolInformation, SymbolKind, Uri,
    request::{
        DocumentSymbolRequest, FoldingRangeRequest, Request as LspRequest,
        SemanticTokensFullRequest,
    },
};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct Options {
    pub(crate) hierarchical_symbols: bool,
    pub(crate) line_folding_only: bool,
    /// Whether the client lists `Key` among its symbol kinds; LSP 3.17 lets a
    /// server assume only `File` through `Array` otherwise.
    pub(crate) key_symbols: bool,
}

type Result<T> = std::result::Result<T, ResponseError>;

#[allow(clippy::mutable_key_type)] // Uri's internal cache does not change its identity.
pub(crate) fn handle_request(
    request: Request,
    documents: &mut HashMap<Uri, Document>,
    encoding: PositionEncoding,
    options: &Options,
) -> Response {
    match request.method.as_str() {
        DocumentSymbolRequest::METHOD => respond::<DocumentSymbolRequest>(request, |params| {
            let uri = &params.text_document.uri;
            let document = analyzed_document(documents, uri)?;
            let root = document.cached_syntax().expect("analysis retains the tree");
            let positions = Positions::new(&document.source, &document.lines, encoding);
            let mut symbols = Vec::new();
            let arm_kind = if options.key_symbols {
                SymbolKind::KEY
            } else {
                SymbolKind::STRING
            };
            collect_symbols(&root, &document.source, &positions, arm_kind, &mut symbols);
            Ok(Some(if options.hierarchical_symbols {
                DocumentSymbolResponse::Nested(symbols)
            } else {
                let mut flat = Vec::new();
                flatten_symbols(symbols, uri, &mut flat);
                DocumentSymbolResponse::Flat(flat)
            }))
        }),
        FoldingRangeRequest::METHOD => respond::<FoldingRangeRequest>(request, |params| {
            let document = analyzed_document(documents, &params.text_document.uri)?;
            let root = document.cached_syntax().expect("analysis retains the tree");
            let positions = Positions::new(&document.source, &document.lines, encoding);
            Ok(Some(folding_ranges(
                &root,
                &positions,
                options.line_folding_only,
            )))
        }),
        SemanticTokensFullRequest::METHOD => {
            respond::<SemanticTokensFullRequest>(request, |params| {
                let document = analyzed_document(documents, &params.text_document.uri)?;
                let root = document.cached_syntax().expect("analysis retains the tree");
                let model = &document
                    .cached_analysis()
                    .expect("analysis was just cached")
                    .model;
                let positions = Positions::new(&document.source, &document.lines, encoding);
                Ok(Some(
                    semantic_tokens::tokens(&root, model, &positions).into(),
                ))
            })
        }
        _ => unreachable!("the server dispatches only structure requests here"),
    }
}

pub(crate) fn respond<R: LspRequest>(
    request: Request,
    handler: impl FnOnce(R::Params) -> Result<R::Result>,
) -> Response {
    let result = serde_json::from_value(request.params)
        .map_err(|error| response_error(ErrorCode::InvalidParams, error.to_string()))
        .and_then(handler)
        .and_then(|result| {
            serde_json::to_value(result)
                .map_err(|error| response_error(ErrorCode::RequestFailed, error.to_string()))
        });
    Response {
        id: request.id,
        response_result: result,
    }
}

#[allow(clippy::mutable_key_type)]
pub(crate) fn analyzed_document<'a>(
    documents: &'a mut HashMap<Uri, Document>,
    uri: &Uri,
) -> Result<&'a Document> {
    let document = documents
        .get_mut(uri)
        .ok_or_else(|| response_error(ErrorCode::InvalidParams, "document is not open"))?;
    document
        .analyze(uri.as_str())
        .map_err(|error| response_error(ErrorCode::RequestFailed, error.to_string()))?;
    Ok(document)
}

pub(crate) fn response_error(code: ErrorCode, message: impl Into<String>) -> ResponseError {
    ResponseError {
        code: code as i32,
        message: message.into(),
        data: None,
    }
}

fn collect_symbols(
    node: &SyntaxNode,
    source: &str,
    positions: &Positions,
    arm_kind: SymbolKind,
    symbols: &mut Vec<DocumentSymbol>,
) {
    let kind = match node.kind() {
        K::Repeat => Some(SymbolKind::ARRAY),
        K::Sections => Some(SymbolKind::NAMESPACE),
        K::Arm => Some(arm_kind),
        K::Instances => Some(SymbolKind::MODULE),
        _ => None,
    };
    if let Some(kind) = kind {
        // Inspect only direct children. Header text never includes a nested body.
        let mut header_end = node.text_range().end();
        let mut selection = None;
        for element in node.children_with_tokens() {
            if matches!(element.kind(), K::Block | K::Arm)
                || (element.kind() == K::Punct
                    && element.as_token().is_some_and(|t| t.text() == "{"))
            {
                header_end = element.text_range().start();
                break;
            }
            if selection.is_none() && !matches!(element.kind(), K::Whitespace | K::Comment) {
                selection = Some(element.text_range());
            }
        }
        let selection = selection.unwrap_or_else(|| TextRange::empty(node.text_range().start()));
        let header = &source[usize::from(node.text_range().start())..usize::from(header_end)];
        let mut children = Vec::new();
        for child in node.children() {
            collect_symbols(&child, source, positions, arm_kind, &mut children);
        }
        #[allow(deprecated)]
        symbols.push(DocumentSymbol {
            name: header.trim().to_owned(),
            detail: None,
            kind,
            tags: None,
            deprecated: None,
            range: positions.range(node.text_range()),
            selection_range: positions.range(selection),
            children: (!children.is_empty()).then_some(children),
        });
    } else {
        for child in node.children() {
            collect_symbols(&child, source, positions, arm_kind, symbols);
        }
    }
}

fn flatten_symbols(symbols: Vec<DocumentSymbol>, uri: &Uri, flat: &mut Vec<SymbolInformation>) {
    for symbol in symbols {
        #[allow(deprecated)]
        flat.push(SymbolInformation {
            name: symbol.name,
            kind: symbol.kind,
            tags: None,
            deprecated: None,
            location: Location::new(uri.clone(), symbol.range),
            container_name: None,
        });
        if let Some(children) = symbol.children {
            flatten_symbols(children, uri, flat);
        }
    }
}

fn folding_ranges(root: &SyntaxNode, positions: &Positions, line_only: bool) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    for node in root
        .descendants()
        .filter(|n| matches!(n.kind(), K::Block | K::Sections))
    {
        let mut open = None;
        let mut close = node.text_range().end();
        for token in node.children_with_tokens().filter_map(|e| e.into_token()) {
            if token.kind() == K::Punct {
                match token.text() {
                    "{" => open = Some(token.text_range().end()),
                    "}" => close = token.text_range().start(),
                    _ => {}
                }
            }
        }
        if let Some(open) = open {
            let start = positions.at(open);
            let end = positions.at(close);
            let end_line = if line_only {
                end.line.saturating_sub(1)
            } else {
                end.line
            };
            if start.line < end_line {
                ranges.push(FoldingRange {
                    start_line: start.line,
                    start_character: (!line_only).then_some(start.character),
                    end_line,
                    end_character: (!line_only).then_some(end.character),
                    ..FoldingRange::default()
                });
            }
        }
    }
    let mut comments: Option<TextRange> = None;
    for token in root
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        if token.kind() == K::Whitespace {
            continue;
        }
        if token.kind() == K::Comment
            && let Some(run) = comments
            && positions.at(token.text_range().start()).line == positions.at(run.end()).line + 1
        {
            comments = Some(TextRange::new(run.start(), token.text_range().end()));
            continue;
        }
        if let Some(run) = comments.take() {
            push_comment_fold(run, positions, line_only, &mut ranges);
        }
        if token.kind() == K::Comment {
            comments = Some(token.text_range());
        }
    }
    if let Some(run) = comments {
        push_comment_fold(run, positions, line_only, &mut ranges);
    }
    ranges
}

fn push_comment_fold(
    run: TextRange,
    positions: &Positions,
    line_only: bool,
    ranges: &mut Vec<FoldingRange>,
) {
    let range = positions.range(run);
    if range.start.line < range.end.line {
        ranges.push(FoldingRange {
            start_line: range.start.line,
            start_character: (!line_only).then_some(range.start.character),
            end_line: range.end.line,
            end_character: (!line_only).then_some(range.end.character),
            kind: Some(FoldingRangeKind::Comment),
            ..FoldingRange::default()
        });
    }
}
