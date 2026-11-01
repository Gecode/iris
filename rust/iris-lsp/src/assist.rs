use crate::{
    builtins::{self, Kind},
    document::{Document, PositionEncoding, Positions},
    structure::{analyzed_document, respond, response_error},
};
use iris_sema::{
    DefinitionKind, Possibilities, Resolution, ScopeId, SemanticModel, ValueFacts, ValueType,
};
use iris_syntax::{SyntaxKind as K, SyntaxNode, SyntaxToken, TextRange, TextSize};
use lsp_server::{ErrorCode, Request, Response, ResponseError};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit, Documentation,
    Hover, HoverContents, InlayHint, InlayHintKind, InlayHintLabel, InsertTextFormat,
    MarkupContent, MarkupKind, Position, Range, TextEdit, Uri,
    request::{Completion, HoverRequest, InlayHintRequest, Request as LspRequest},
};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct Options {
    pub(crate) hover_markdown: bool,
    pub(crate) completion_markdown: bool,
    pub(crate) snippets: bool,
}

#[allow(clippy::mutable_key_type)] // Uri's cache does not change its identity.
pub(crate) fn handle_request(
    request: Request,
    documents: &mut HashMap<Uri, Document>,
    encoding: PositionEncoding,
    options: &Options,
) -> Response {
    match request.method.as_str() {
        HoverRequest::METHOD => respond::<HoverRequest>(request, |params| {
            let params = params.text_document_position_params;
            let document = analyzed_document(documents, &params.text_document.uri)?;
            let offset = offset(document, params.position, encoding)?;
            let positions = Positions::new(&document.source, &document.lines, encoding);
            Ok(hover(document, offset, &positions, options.hover_markdown))
        }),
        Completion::METHOD => respond::<Completion>(request, |params| {
            let params = params.text_document_position;
            let document = analyzed_document(documents, &params.text_document.uri)?;
            let offset = offset(document, params.position, encoding)?;
            let positions = Positions::new(&document.source, &document.lines, encoding);
            Ok(Some(CompletionResponse::Array(completions(
                document, offset, &positions, options,
            ))))
        }),
        InlayHintRequest::METHOD => respond::<InlayHintRequest>(request, |params| {
            let document = analyzed_document(documents, &params.text_document.uri)?;
            if params.range.start > params.range.end {
                return Err(response_error(
                    ErrorCode::InvalidParams,
                    "reversed hint range",
                ));
            }
            let start = offset(document, params.range.start, encoding)?;
            let end = offset(document, params.range.end, encoding)?;
            let positions = Positions::new(&document.source, &document.lines, encoding);
            Ok(Some(inlay_hints(
                document,
                TextRange::new(start, end),
                &positions,
            )))
        }),
        _ => unreachable!("the server dispatches only assist requests here"),
    }
}

fn offset(
    document: &Document,
    position: Position,
    encoding: PositionEncoding,
) -> Result<TextSize, ResponseError> {
    let offset = document
        .lines
        .byte_offset(&document.source, position, encoding)
        .map_err(|error| response_error(ErrorCode::InvalidParams, error))?;
    TextSize::try_from(offset)
        .map_err(|error| response_error(ErrorCode::InvalidParams, error.to_string()))
}

fn hover(
    document: &Document,
    offset: TextSize,
    positions: &Positions,
    markdown: bool,
) -> Option<Hover> {
    let model = &document.cached_analysis()?.model;
    // Each point lookup scans once; never perform these lookups per token or candidate.
    let named = if let Some(id) = model.definition_at(offset) {
        let definition = model.definition(id);
        Some((
            definition.name_range,
            name_description(
                &definition.name,
                definition.kind,
                &definition.facts,
                markdown,
            ),
        ))
    } else if let Some(id) = model.reference_at(offset) {
        let reference = model.reference(id);
        let kind = match reference.resolution {
            Resolution::NamedType(_) => DefinitionKind::NamedType,
            Resolution::Structural { ref denotation, .. } => match denotation {
                iris_sema::Denotation::Record(_) => DefinitionKind::EachRecord,
                iris_sema::Denotation::RecordList(_) => DefinitionKind::RecordList,
                iris_sema::Denotation::Value => DefinitionKind::Read,
            },
            _ => DefinitionKind::Read,
        };
        Some((
            reference.name_range,
            name_description(&reference.name, kind, &reference.facts, markdown),
        ))
    } else {
        None
    };
    if let Some((range, value)) = named {
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: if markdown {
                    MarkupKind::Markdown
                } else {
                    MarkupKind::PlainText
                },
                value,
            }),
            range: Some(positions.range(range)),
        });
    }
    let root = document.cached_syntax()?;
    let token = root.token_at_offset(offset).right_biased()?;
    if token.kind() != K::Ident || !token.text_range().contains(offset) {
        return None;
    }
    // The model wins over contextual keyword spellings, including partial names
    // in invalid source. Do not document literal format names as keywords either.
    if token.parent().is_some_and(|node| {
        matches!(
            node.kind(),
            K::Name | K::Field | K::TypeName | K::CurrentValue | K::Capture
        ) || (node.kind() == K::Format && token.text_range().start() != node.text_range().start())
    }) {
        return None;
    }
    let documentation = builtins::documentation(token.text())?;
    Some(Hover {
        contents: HoverContents::Markup(documentation.markup(markdown)),
        range: Some(positions.range(token.text_range())),
    })
}

fn name_description(
    name: &str,
    kind: DefinitionKind,
    facts: &ValueFacts,
    markdown: bool,
) -> String {
    let name = if markdown {
        format!("`{name}`")
    } else {
        name.to_owned()
    };
    match kind {
        DefinitionKind::NamedType => format!(
            "{name}: named type\n\n{}",
            builtins::documentation("type")
                .expect("type has built-in documentation")
                .markup(markdown)
                .value
        ),
        DefinitionKind::RecordList => format!("{name}: record list"),
        DefinitionKind::EachRecord => format!("{name}: record"),
        _ => format!("{name}: {}", shape(facts)),
    }
}

fn shape(facts: &ValueFacts) -> String {
    let value_type = possibilities(&facts.value_types, |value| match value {
        ValueType::Int => "int".into(),
        ValueType::Float => "float".into(),
        ValueType::String => "string".into(),
    });
    let depth = possibilities(&facts.depths, usize::to_string);
    format!("{value_type}, depth {depth}")
}

fn possibilities<T>(values: &Possibilities<T>, render: impl Fn(&T) -> String) -> String {
    match values {
        Possibilities::Known(values) if values.len() == 1 => render(&values[0]),
        Possibilities::Known(values) if !values.is_empty() => {
            format!(
                "{} (ambiguous)",
                values.iter().map(render).collect::<Vec<_>>().join(" or ")
            )
        }
        _ => "unknown".into(),
    }
}

fn inlay_hints(document: &Document, range: TextRange, positions: &Positions) -> Vec<InlayHint> {
    let model = &document
        .cached_analysis()
        .expect("document was analyzed")
        .model;
    model
        .definitions()
        .iter()
        .filter(|definition| {
            range.contains_inclusive(definition.name_range.end())
                && matches!(
                    definition.kind,
                    DefinitionKind::Read
                        | DefinitionKind::Capture
                        | DefinitionKind::Let
                        | DefinitionKind::Param
                        | DefinitionKind::Store
                )
        })
        .map(|definition| InlayHint {
            position: positions.at(definition.name_range.end()),
            label: InlayHintLabel::String(format!(": {}", shape(&definition.facts))),
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            tooltip: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Context {
    Statement,
    Type,
    Expression,
    Layout,
    Keywords,
    Suppressed,
}

struct CompletionSite {
    context: Context,
    keywords: &'static [&'static str],
    replacement: TextRange,
    prefix: String,
    block_start: Option<TextSize>,
    context_node: Option<SyntaxNode>,
}

fn completions(
    document: &Document,
    offset: TextSize,
    positions: &Positions,
    options: &Options,
) -> Vec<CompletionItem> {
    let root = document.cached_syntax().expect("document was analyzed");
    let model = &document
        .cached_analysis()
        .expect("document was analyzed")
        .model;
    let site = completion_site(&root, &document.source, offset);
    if site.context == Context::Suppressed {
        return vec![];
    }
    let range = positions.range(site.replacement);
    if site.context == Context::Layout {
        return builtins::LAYOUTS
            .iter()
            .filter(|name| name.starts_with(&site.prefix))
            .map(|name| CompletionItem {
                label: (*name).into(),
                kind: Some(CompletionItemKind::VALUE),
                documentation: builtins::documentation("layout").map(|doc| {
                    Documentation::MarkupContent(doc.markup(options.completion_markdown))
                }),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(
                    range,
                    format!("\"{name}\""),
                ))),
                ..CompletionItem::default()
            })
            .collect();
    }

    let mut items = Vec::new();
    let mut offered = HashSet::new();
    // Compute scope visibility once, then scan definitions once. No per-candidate
    // scope/model search, ancestor walk, or expression lookup is needed.
    let visibility = Visibility::new(model, offset, site.block_start, site.context_node.as_ref());
    for definition in model.definitions() {
        let is_type = definition.kind == DefinitionKind::NamedType;
        if is_type != (site.context == Context::Type)
            || !matches!(site.context, Context::Type | Context::Expression)
            || !definition.name.starts_with(&site.prefix)
            // A discarded value is not a name, but a structural `_` names its
            // records (§8.12).
            || (definition.name == "_"
                && !matches!(
                    definition.kind,
                    DefinitionKind::EachRecord | DefinitionKind::RecordList
                ))
            || !visibility.contains(definition, offset)
            || !offered.insert(definition.name.clone())
        {
            continue;
        }
        items.push(CompletionItem {
            label: definition.name.clone(),
            kind: Some(if is_type {
                CompletionItemKind::CLASS
            } else {
                CompletionItemKind::VARIABLE
            }),
            // Completion offers spellings, not a guessed shape at this new use.
            text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(
                range,
                definition.name.clone(),
            ))),
            ..CompletionItem::default()
        });
    }
    for doc in builtins::DOCUMENTATION {
        let relevant = site.keywords.contains(&doc.name)
            || match site.context {
                Context::Statement => match doc.name {
                    "instances" => visibility.top_level,
                    "each" => visibility.each_allowed,
                    _ => {
                        doc.kind == Kind::Statement
                            || (doc.kind == Kind::Setting && visibility.top_level)
                    }
                },
                Context::Type => doc.kind == Kind::Type,
                Context::Expression => doc.kind == Kind::Function,
                _ => false,
            };
        if !relevant || !doc.name.starts_with(&site.prefix) || !offered.insert(doc.name.to_owned())
        {
            continue;
        }
        items.push(builtin_completion(doc, range, options));
    }
    if site.context == Context::Expression {
        for name in ["gecode.int_max", "gecode.int_min"] {
            if name.starts_with(&site.prefix) {
                items.push(CompletionItem {
                    label: name.into(),
                    kind: Some(CompletionItemKind::VARIABLE),
                    text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(range, name.into()))),
                    ..CompletionItem::default()
                });
            }
        }
    }
    items
}

fn builtin_completion(
    doc: &builtins::Documentation,
    range: Range,
    options: &Options,
) -> CompletionItem {
    let snippet = options.snippets && doc.kind == Kind::Function;
    let new_text = if snippet {
        if matches!(doc.name, "all" | "any") {
            format!(
                "{}(${{1:x}} in ${{2:values}}: ${{3:condition}})$0",
                doc.name
            )
        } else {
            format!("{}(${{1}})$0", doc.name)
        }
    } else {
        doc.name.into()
    };
    CompletionItem {
        label: doc.name.into(),
        kind: Some(match doc.kind {
            Kind::Type => CompletionItemKind::CLASS,
            Kind::Function => CompletionItemKind::FUNCTION,
            _ => CompletionItemKind::KEYWORD,
        }),
        documentation: Some(Documentation::MarkupContent(
            doc.markup(options.completion_markdown),
        )),
        insert_text_format: Some(if snippet {
            InsertTextFormat::SNIPPET
        } else {
            InsertTextFormat::PLAIN_TEXT
        }),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(range, new_text))),
        ..CompletionItem::default()
    }
}

struct Visibility {
    ancestors: Vec<bool>,
    values: Vec<bool>,
    top_level: bool,
    each_allowed: bool,
    in_check: bool,
}

impl Visibility {
    fn new(
        model: &SemanticModel,
        offset: TextSize,
        block_start: Option<TextSize>,
        context_node: Option<&SyntaxNode>,
    ) -> Self {
        let scopes = model.scopes();
        let mut ancestors = vec![false; scopes.len()];
        let selected = block_start
            .and_then(|start| model.scope_at(start))
            .unwrap_or(ScopeId(0));
        let mut current = Some(selected);
        while let Some(scope) = current {
            ancestors[scope.0] = true;
            current = scopes[scope.0].parent;
        }
        let mut active_conditionals = HashSet::new();
        let mut in_check = model
            .checks()
            .iter()
            .any(|check| check.node.text_range().contains_inclusive(offset));
        // Semantic scopes identify branch membership. Inspect only the cursor's
        // ancestor chains to recognize active conditionals, including else-if.
        // The scope chain also covers whitespace outside a recovered body.
        let context_ancestors = context_node
            .map(SyntaxNode::ancestors)
            .into_iter()
            .flatten();
        for node in scopes[selected.0].node.ancestors().chain(context_ancestors) {
            match node.kind() {
                K::If => {
                    active_conditionals.insert(node.text_range());
                }
                K::Check => in_check = true,
                _ => {}
            }
        }
        // Scope IDs are allocated with parents before children. Completed repeat
        // frames publish columns; ordinary blocks share their parent's frame.
        let mut values = vec![false; scopes.len()];
        for (id, scope) in scopes.iter().enumerate() {
            let sibling_branch = !ancestors[id]
                && scope.node.parent().is_some_and(|parent| {
                    parent.kind() == K::If && active_conditionals.contains(&parent.text_range())
                });
            values[id] = ancestors[id]
                || (scope.node.text_range().end() <= offset
                    && scope.parent.is_some_and(|parent| values[parent.0])
                    && !sibling_branch);
        }
        Self {
            ancestors,
            values,
            top_level: block_start.is_none(),
            in_check,
            each_allowed: scopes[selected.0]
                .node
                .parent()
                .is_some_and(|parent| parent.kind() == K::Repeat)
                && scopes[selected.0]
                    .node
                    .children()
                    .find(|node| node.kind() != K::Error)
                    .is_none_or(|first| first.text_range().contains_inclusive(offset)),
        }
    }

    fn contains(&self, definition: &iris_sema::Definition, offset: TextSize) -> bool {
        match definition.kind {
            DefinitionKind::NamedType => {
                self.ancestors[definition.scope.0] && definition.node.text_range().end() < offset
            }
            DefinitionKind::Binding => {
                let colon = definition
                    .node
                    .children_with_tokens()
                    .filter_map(|element| element.into_token())
                    .find(|token| token.text() == ":");
                definition.node.children().last().is_some_and(|body| {
                    colon.is_some_and(|colon| colon.text_range().end() <= body.text_range().start())
                        && body.text_range().contains_inclusive(offset)
                })
            }
            DefinitionKind::EachRecord => {
                (self.ancestors[definition.scope.0]
                    || (self.in_check && self.values[definition.scope.0]))
                    && definition.name_range.end() <= offset
            }
            DefinitionKind::RecordList => {
                self.ancestors[definition.scope.0] && definition.node.text_range().end() < offset
            }
            _ => self.values[definition.scope.0] && definition.node.text_range().end() < offset,
        }
    }
}

fn completion_site(root: &SyntaxNode, source: &str, offset: TextSize) -> CompletionSite {
    let mut previous = None;
    let mut touching: Option<SyntaxToken> = None;
    let mut blocks = Vec::new();
    let mut brackets = 0usize;
    // One leaf pass also handles an empty/partial tree and whitespace at EOF.
    for token in root
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        if token.text_range().start() > offset {
            break;
        }
        if token.text_range().start() < offset && token.kind() == K::Punct {
            match token.text() {
                "[" => brackets += 1,
                "]" => brackets = brackets.saturating_sub(1),
                _ => {}
            }
        }
        // Track active braces even when recovery leaves trailing whitespace
        // outside an unfinished block's range. Pattern braces are not blocks.
        if token.text_range().start() < offset
            && token.kind() == K::Punct
            && let Some(parent) = token.parent()
            && matches!(parent.kind(), K::Block | K::Sections)
        {
            match token.text() {
                "{" => blocks.push((parent.kind(), parent.text_range().start())),
                "}" => {
                    blocks.pop();
                }
                _ => {}
            }
        }
        // At a boundary such as `row|.x`, the identifier ending at the cursor
        // is the one being completed, not the punctuation starting there.
        if (token.text_range().contains(offset) || token.text_range().end() == offset)
            && !touching.as_ref().is_some_and(|t| t.kind() == K::Ident)
        {
            touching = Some(token.clone());
        }
        if !matches!(token.kind(), K::Whitespace | K::Comment)
            && token.text_range().start() < offset
        {
            previous = Some(token);
        }
    }
    let mut site = CompletionSite {
        context: Context::Statement,
        keywords: &[],
        replacement: TextRange::empty(offset),
        prefix: String::new(),
        block_start: blocks.last().map(|(_, start)| *start),
        context_node: None,
    };
    if let Some(token) = touching.as_ref() {
        if token.kind() == K::Comment && token.text_range().contains(offset) {
            site.context = Context::Suppressed;
            return site;
        }
        if token.kind() == K::Ident {
            site.replacement = token.text_range();
            site.prefix =
                source[usize::from(token.text_range().start())..usize::from(offset)].into();
        }
    }
    let token = if touching
        .as_ref()
        .is_some_and(|t| !matches!(t.kind(), K::Whitespace | K::Comment))
    {
        touching
    } else {
        previous
    };
    let Some(token) = token else {
        return site;
    };
    site.context_node = token.parent();
    site.context = context(&token, offset);
    if let Some(continuation) = continuation(&token, site.replacement.start()) {
        site.context = continuation.context;
        site.keywords = continuation.keywords;
    }
    // Brackets delimit dimensions and indices even if recovery placed their
    // whitespace or closing bracket outside the expression's node.
    if brackets > 0 {
        site.context = Context::Expression;
        site.keywords = &[];
    }
    if site.context == Context::Statement
        && blocks.last().is_some_and(|(kind, _)| *kind == K::Sections)
    {
        site.context = Context::Suppressed;
    }
    if matches!(token.kind(), K::PatternToken | K::Quote) && token.text_range().contains(offset) {
        site.context = Context::Suppressed;
    }
    if token.kind() == K::String {
        if site.context == Context::Layout {
            site.replacement = token.text_range();
            site.prefix = source[usize::from(token.text_range().start())..usize::from(offset)]
                .trim_start_matches('"')
                .into();
        } else if token.text_range().contains(offset) {
            site.context = Context::Suppressed;
        }
    }
    site
}

struct Continuation {
    context: Context,
    keywords: &'static [&'static str],
}

fn continuation(token: &SyntaxToken, start: TextSize) -> Option<Continuation> {
    for node in token.parent_ancestors() {
        // A cursor within a type or expression is not a header continuation.
        // After its final token, whitespace can instead begin an option.
        if matches!(
            node.kind(),
            K::Type | K::Range | K::Dimension | K::Layout | K::Offset
        ) && start <= node.text_range().end()
        {
            return None;
        }
        let (context, keywords): (Context, &'static [&'static str]) = match node.kind() {
            K::Block => return None,
            K::Setting => {
                let first = node.first_token()?;
                if first.text() != "ignore" || start <= first.text_range().end() {
                    return None;
                }
                if node
                    .children_with_tokens()
                    .filter_map(|element| element.into_token())
                    .any(|token| token.text() == "trailing" && token.text_range().end() < start)
                {
                    return None;
                }
                (Context::Keywords, &["trailing"])
            }
            K::List => {
                let reading_type = node.children().find(|child| child.kind() == K::Type)?;
                if start <= reading_type.text_range().end() {
                    return None;
                }
                let until = node
                    .children_with_tokens()
                    .filter_map(|element| element.into_token())
                    .find(|token| token.text() == "until" && token.text_range().end() <= start);
                if until.is_some() {
                    let sentinel = node
                        .children()
                        .find(|child| !matches!(child.kind(), K::Type | K::Error | K::Offset));
                    let eol = node
                        .children_with_tokens()
                        .filter_map(|element| element.into_token())
                        .any(|token| token.text() == "eol" && token.text_range().end() < start);
                    if eol || sentinel.is_some_and(|sentinel| sentinel.text_range().end() < start) {
                        (Context::Statement, &["offset"])
                    } else {
                        (Context::Expression, &["eol"])
                    }
                } else {
                    (Context::Keywords, &["until"])
                }
            }
            K::Repeat => {
                if start <= node.first_token()?.text_range().end() {
                    return None;
                }
                let own: Vec<_> = node
                    .children_with_tokens()
                    .filter_map(|element| element.into_token())
                    .filter(|token| token.kind() == K::Ident && token.text_range().end() <= start)
                    .collect();
                if own.iter().any(|token| token.text() == "as") {
                    return None;
                }
                if own.iter().any(|token| token.text() == "until") {
                    let has_terminator = node
                        .children()
                        .filter(|child| child.kind() == K::Terminators)
                        .flat_map(|child| child.children_with_tokens())
                        .any(|element| {
                            element.kind() == K::String && element.text_range().end() <= start
                        });
                    if has_terminator {
                        (Context::Keywords, &["store", "as"])
                    } else if own.iter().any(|token| token.text() == "eof") {
                        (Context::Keywords, &["as"])
                    } else {
                        (Context::Keywords, &["eof"])
                    }
                } else {
                    let count = node
                        .children()
                        .find(|child| !matches!(child.kind(), K::Error | K::Block));
                    if count.is_some_and(|count| count.text_range().end() < start) {
                        (Context::Keywords, &["as"])
                    } else {
                        (Context::Expression, &["until"])
                    }
                }
            }
            K::Skip => {
                if start <= node.first_token()?.text_range().end()
                    || node
                        .children_with_tokens()
                        .filter_map(|element| element.into_token())
                        .any(|token| {
                            matches!(token.text(), "line" | "lines")
                                && token.text_range().end() < start
                        })
                {
                    return None;
                }
                let count = node.children().find(|child| child.kind() != K::Error);
                if count.is_some_and(|count| count.text_range().end() < start) {
                    (Context::Keywords, &["lines"])
                } else {
                    (Context::Expression, &["line"])
                }
            }
            // Other statements do not constrain the following keyword here.
            K::Let
            | K::Param
            | K::Check
            | K::If
            | K::Declaration
            | K::TypeDefinition
            | K::Line
            | K::Find
            | K::Expect
            | K::Each
            | K::Instances
            | K::Sections => return None,
            _ => continue,
        };
        return Some(Continuation { context, keywords });
    }
    None
}

fn context(token: &SyntaxToken, offset: TextSize) -> Context {
    let mut context = Context::Statement;
    if token.parent().is_some_and(|node| node.kind() == K::Field) {
        return Context::Suppressed;
    }
    for node in token.parent_ancestors() {
        // A keyword being edited at statement start is still a statement site.
        if token.kind() == K::Ident
            && offset <= token.text_range().end()
            && token.text_range().start() == node.text_range().start()
            && builtins::documentation(token.text())
                .is_some_and(|doc| matches!(doc.kind, Kind::Statement | Kind::Setting))
            && matches!(
                node.kind(),
                K::Let
                    | K::Param
                    | K::Check
                    | K::If
                    | K::Repeat
                    | K::Index
                    | K::Setting
                    | K::Format
                    | K::List
                    | K::TypeDefinition
                    | K::Line
                    | K::Find
                    | K::Skip
                    | K::Expect
                    | K::Each
                    | K::Instances
                    | K::Sections
            )
        {
            return Context::Statement;
        }
        match node.kind() {
            K::Block => return context,
            K::Layout if offset <= node.text_range().end() || token.text() == "layout" => {
                return Context::Layout;
            }
            K::Type | K::BaseType | K::TypeName if offset <= node.text_range().end() => {
                return Context::Type;
            }
            K::Range => return Context::Expression,
            K::Declaration | K::List | K::Capture if token.text() == ":" => return Context::Type,
            K::TypeDefinition if token.text() == "=" => return Context::Type,
            K::Name
            | K::HostConstant
            | K::Literal
            | K::Binary
            | K::Unary
            | K::Call
            | K::Comprehension
            | K::Dimension
            | K::Offset
                if offset <= node.text_range().end() =>
            {
                context = Context::Expression
            }
            K::Let | K::Param | K::Check | K::If | K::Repeat | K::Index | K::Skip
                if offset <= node.text_range().end()
                    || matches!(
                        token.text(),
                        "=" | "check" | "if" | "repeat" | "index" | "skip"
                    ) =>
            {
                context = Context::Expression
            }
            _ => {}
        }
    }
    context
}
