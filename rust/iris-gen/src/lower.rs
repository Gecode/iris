//! Lower a checked spec to a [`Program`].
//!
//! The walk is one loop over the syntax tree with an explicit stack of frames,
//! because the language's nesting limits are not a bound on this crate's stack.
use std::collections::{HashMap, HashSet};

use gecode_iris_sema::{Analysis, Denotation, Reference, RepeatId, Resolution};
use gecode_iris_syntax::{
    DecodedString, Parse, SyntaxElement, SyntaxKind as K, SyntaxNode, SyntaxToken, TextSize,
    WalkEvent,
};

use crate::program::{
    AlphabetOption, BaseType, BinaryOp, ComprehensionKind, Declared, Event, Expr, ExprOp, Function,
    Host, ListEnd, Name, NameId, NameKind, Piece, Program, RecordId, RecordNames, RepeatCount,
    RepeatNames, SectionClause, Target, Type, UnaryOp, VariableId,
};

/// The program of a spec, or the first error that `iris check` reports.
pub fn lower(parsed: &Parse, analysis: &Analysis) -> Result<Program, gecode_iris_sema::Error> {
    if let Some(error) = analysis.first_error() {
        return Err(error.clone());
    }
    Ok(Lowering::new(parsed, analysis).run(parsed.syntax()))
}

fn is_trivia(kind: K) -> bool {
    matches!(kind, K::Whitespace | K::Comment)
}

/// The tokens directly in `node`, without trivia.
fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> + use<> {
    node.children_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .filter(|token| !is_trivia(token.kind()))
}

/// The direct token after the one spelled `keyword`.
fn token_after(node: &SyntaxNode, keyword: &str) -> Option<SyntaxToken> {
    tokens(node)
        .skip_while(|token| token.text() != keyword)
        .nth(1)
}

fn first_token(node: &SyntaxNode) -> SyntaxToken {
    tokens(node).next().expect("the node has a token")
}

fn has_token(node: &SyntaxNode, text: &str) -> bool {
    tokens(node).any(|token| token.text() == text)
}

/// The defining identifier of a statement, whose keyword `skip` tokens precede it.
fn defined_name(node: &SyntaxNode, skip: usize) -> SyntaxToken {
    tokens(node)
        .filter(|token| token.kind() == K::Ident)
        .nth(skip)
        .expect("a checked definition has a name")
}

fn decode_quoted(quoted: &str) -> String {
    DecodedString::new(quoted).text
}

/// The text of a slice of a string literal: the pattern's escapes are still in it.
fn decode_raw(raw: &str) -> String {
    decode_quoted(&format!("\"{raw}\""))
}

/// What a pass over the whole tree finds before any statement is lowered.
struct Survey {
    /// Every value name that a statement defines, including `store _`.
    defined: HashSet<String>,
    /// Every identifier, so that invented names cannot collide.
    identifiers: HashSet<String>,
}

impl Survey {
    fn new(root: &SyntaxNode) -> Self {
        let mut survey = Self {
            defined: HashSet::new(),
            identifiers: HashSet::new(),
        };
        for element in root.preorder_with_tokens() {
            let WalkEvent::Enter(element) = element else {
                continue;
            };
            match element {
                SyntaxElement::Token(token) if token.kind() == K::Ident => {
                    survey.identifiers.insert(decode_raw(token.text()));
                }
                SyntaxElement::Node(node) => survey.definition(&node),
                SyntaxElement::Token(_) => {}
            }
        }
        survey
    }

    fn definition(&mut self, node: &SyntaxNode) {
        let token = match node.kind() {
            K::Declaration => Some(defined_name(node, 0)),
            K::List | K::Let | K::Param => Some(defined_name(node, 1)),
            K::Capture => tokens(node).find(|token| token.kind() == K::Ident),
            K::Terminators => token_after(node, "store"),
            _ => None,
        };
        let Some(token) = token else { return };
        let name = decode_raw(token.text());
        if name != "_" || node.kind() == K::Terminators {
            self.defined.insert(name);
        }
    }
}

/// A node being lowered, with what its children have produced so far.
struct Frame {
    node: SyntaxNode,
    kind: K,
    /// Where this node's postfix operations start in [`Program::exprs`].
    start: usize,
    exprs: Vec<Expr>,
    ty: Option<Type>,
    pieces: Vec<Piece>,
    dimensions: Vec<Expr>,
    layout: Option<Expr>,
    offset: Option<Expr>,
    /// How many of this node's blocks have started.
    blocks: usize,
}

/// A visible `type` definition.
struct NamedType {
    name: String,
    event: usize,
}

struct Lowering<'a> {
    analysis: &'a Analysis,
    survey: Survey,
    references: HashMap<TextSize, &'a Reference>,
    /// The record of each repeat that something refers to, by `RepeatId`.
    records: Vec<Option<RecordId>>,
    repeats: HashMap<TextSize, RepeatId>,
    program: Program,
    name_ids: HashMap<String, NameId>,
    variable_ids: HashMap<String, VariableId>,
    frames: Vec<Frame>,
    types: Vec<NamedType>,
    /// The length of `types` at the start of each open block.
    type_scopes: Vec<usize>,
    closures: usize,
    /// How many `Capture` frames are open: their strings are escaped twice.
    captures: usize,
}

impl<'a> Lowering<'a> {
    fn new(parsed: &Parse, analysis: &'a Analysis) -> Self {
        let model = &analysis.model;
        let references = model
            .references()
            .iter()
            .map(|reference| (reference.name_range.start(), reference))
            .collect();
        let repeats = model
            .repeats()
            .iter()
            .enumerate()
            .map(|(id, repeat)| (repeat.node.text_range().start(), RepeatId(id)))
            .collect();
        let mut lowering = Self {
            analysis,
            survey: Survey::new(&parsed.syntax()),
            references,
            records: vec![None; model.repeats().len()],
            repeats,
            program: Program::default(),
            name_ids: HashMap::new(),
            variable_ids: HashMap::new(),
            frames: Vec::new(),
            types: Vec::new(),
            type_scopes: Vec::new(),
            closures: 0,
            captures: 0,
        };
        lowering.allocate_records();
        lowering
    }

    /// Give every repeat that a reference needs a record, in text order.
    fn allocate_records(&mut self) {
        let mut referenced = vec![false; self.records.len()];
        for reference in self.analysis.model.references() {
            if let Resolution::Structural {
                denotation: Denotation::Record(repeat) | Denotation::RecordList(repeat),
                ..
            } = &reference.resolution
            {
                referenced[repeat.0] = true;
            }
        }
        let mut order: Vec<usize> = (0..referenced.len()).filter(|&id| referenced[id]).collect();
        let repeats = self.analysis.model.repeats();
        order.sort_by_key(|&id| repeats[id].node.text_range().start());
        for id in order {
            let (each, list) = repeat_names(&repeats[id].node, Decoding::Source);
            let each = each.unwrap_or_else(|| self.invented_name());
            let list = list.unwrap_or_else(|| self.invented_name());
            self.records[id] = Some(RecordId(self.program.records.len()));
            self.program.records.push(RecordNames { each, list });
        }
    }

    /// A name no identifier of the spec spells.
    fn invented_name(&mut self) -> String {
        let mut number = 1;
        loop {
            let name = format!("record_{number}");
            if self.survey.identifiers.insert(name.clone()) {
                return name;
            }
            number += 1;
        }
    }

    fn run(mut self, root: SyntaxNode) -> Program {
        let mut walk = root.preorder_with_tokens();
        while let Some(event) = walk.next() {
            match event {
                WalkEvent::Enter(SyntaxElement::Node(node)) => {
                    // A type name is not a value name, even as an expression.
                    let skip = node.kind() == K::TypeName;
                    self.enter(node);
                    if skip {
                        walk.skip_subtree();
                    }
                }
                WalkEvent::Leave(SyntaxElement::Node(_)) => self.leave(),
                WalkEvent::Enter(SyntaxElement::Token(_))
                | WalkEvent::Leave(SyntaxElement::Token(_)) => {}
            }
        }
        self.program
    }

    fn decoding(&self) -> Decoding {
        if self.captures > 0 {
            Decoding::Capture
        } else {
            Decoding::Source
        }
    }

    fn token_text(&self, token: &SyntaxToken) -> String {
        self.decoding().plain(token)
    }

    fn string_text(&self, token: &SyntaxToken) -> String {
        self.decoding().string(token)
    }

    fn strings(&self, node: &SyntaxNode) -> Vec<String> {
        tokens(node)
            .filter(|token| token.kind() == K::String)
            .map(|token| self.string_text(&token))
            .collect()
    }

    fn string(&self, node: &SyntaxNode) -> String {
        self.strings(node)
            .into_iter()
            .next()
            .expect("a checked statement has its string")
    }

    fn name(&mut self, text: String) -> NameId {
        if let Some(&id) = self.name_ids.get(&text) {
            return id;
        }
        let id = NameId(self.program.names.len());
        let kind = if self.survey.defined.contains(&text) {
            NameKind::Forward
        } else {
            NameKind::Undefined
        };
        self.name_ids.insert(text.clone(), id);
        self.program.names.push(Name { text, kind });
        id
    }

    fn variable(&mut self, text: String) -> VariableId {
        if let Some(&id) = self.variable_ids.get(&text) {
            return id;
        }
        let id = VariableId(self.program.variables.len());
        self.variable_ids.insert(text.clone(), id);
        self.program.variables.push(text);
        id
    }

    fn target(&mut self, token: &SyntaxToken) -> Target {
        let text = self.token_text(token);
        if text == "_" {
            Target::Discard
        } else {
            Target::Name(self.name(text))
        }
    }

    fn event(&mut self, event: Event) {
        self.program.events.push(event);
    }

    fn top(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("a frame is open")
    }

    fn parent_kind(&self) -> Option<K> {
        self.frames.last().map(|frame| frame.kind)
    }

    fn open_closures(&mut self, count: usize) {
        self.closures += count;
        self.program.closure_depth = self.program.closure_depth.max(self.closures);
    }

    fn enter(&mut self, node: SyntaxNode) {
        let kind = node.kind();
        if is_expression(kind) && self.parent_kind() == Some(K::Comprehension) {
            self.comprehension_part();
        }
        match kind {
            K::Format => self.format(&node),
            K::Setting => self.setting(&node),
            K::Expect => {
                let words = self.strings(&node);
                self.event(Event::Expect(words));
            }
            K::Sections => self.sections(&node),
            K::Block => self.open_block(&node),
            K::Capture => self.captures += 1,
            K::PatternText | K::PatternBlank => self.pattern_text(&node),
            K::Ellipsis => self.top().pieces.push(Piece::RestOfLine),
            K::CurrentValue => self.current_value(&node),
            K::TypeName => self.type_name(&node),
            _ => {}
        }
        let start = self.program.exprs.len();
        self.frames.push(Frame {
            node,
            kind,
            start,
            exprs: Vec::new(),
            ty: None,
            pieces: Vec::new(),
            dimensions: Vec::new(),
            layout: None,
            offset: None,
            blocks: 0,
        });
    }

    fn leave(&mut self) {
        let frame = self.frames.pop().expect("every leave has its enter");
        match frame.kind {
            K::Declaration => self.declaration(frame),
            K::List => self.list(frame),
            K::TypeDefinition => self.type_definition(frame),
            K::Let => {
                let target = self.named(&defined_name(&frame.node, 1));
                self.event(Event::Let {
                    target,
                    value: only_expr(&frame),
                });
            }
            K::Param => {
                let target = self.named(&defined_name(&frame.node, 1));
                self.event(Event::Param {
                    target,
                    default: frame.exprs.first().copied(),
                });
            }
            K::Check => {
                let message = tokens(&frame.node)
                    .find(|token| token.kind() == K::String)
                    .map(|token| self.string_text(&token));
                self.event(Event::Check {
                    condition: only_expr(&frame),
                    message,
                });
            }
            K::Skip => {
                let event = match frame.exprs.first() {
                    Some(&count) => Event::SkipLines(count),
                    None => Event::SkipLine,
                };
                self.event(event);
            }
            K::Index => self.event(Event::Index(only_expr(&frame))),
            K::Line => self.event(Event::Line(frame.pieces)),
            K::Find => self.event(Event::Find(frame.pieces)),
            K::Arm if frame.blocks == 0 => self.arm(&frame.node, frame.pieces, false),
            K::Sections => self.close_block(),
            K::Block => {
                self.type_scope_end();
                self.close_block();
            }
            K::Pattern => self.top().pieces = frame.pieces,
            K::Capture => self.capture(frame),
            K::Dimension => {
                let size = only_expr(&frame);
                self.top().dimensions.push(size);
            }
            K::Layout => self.top().layout = frame.exprs.first().copied(),
            K::Offset => self.top().offset = frame.exprs.first().copied(),
            K::BaseType => {
                let ty = Type::Base(base_type(&self.token_text(&first_token(&frame.node))));
                self.top().ty = Some(ty);
            }
            K::Type => self.top().ty = frame.ty,
            K::Alphabet => {
                let ty = self.alphabet(&frame.node);
                self.top().ty = Some(ty);
            }
            K::Range => {
                let ty = range(&frame);
                self.top().ty = Some(ty);
            }
            kind if is_expression(kind) => self.expression(&frame),
            _ => {}
        }
    }

    fn close_block(&mut self) {
        self.closures -= 1;
        self.event(Event::Close);
    }

    fn type_scope_end(&mut self) {
        let start = self.type_scopes.pop().expect("every block opened a scope");
        self.types.truncate(start);
    }

    fn format(&mut self, node: &SyntaxNode) {
        let name: String = tokens(node)
            .skip(1)
            .map(|token| match token.kind() {
                K::String => decode_quoted(token.text()),
                _ => token.text().to_owned(),
            })
            .collect();
        self.event(Event::Format(name));
    }

    fn setting(&mut self, node: &SyntaxNode) {
        let keyword = tokens(node).next().expect("a setting has a keyword");
        let event = match keyword.text() {
            "comment" => Event::Comment(self.strings(node)),
            "stop" => Event::Stop(self.strings(node)),
            "tokens" => Event::Tokens(self.string(node)),
            "separators" => Event::Separators(self.string(node)),
            _ => Event::IgnoreTrailing,
        };
        self.event(event);
    }

    /// A name that cannot be `_`, because the syntax does not allow it there.
    fn named(&mut self, token: &SyntaxToken) -> NameId {
        let text = self.token_text(token);
        self.name(text)
    }

    fn terminators(&mut self, node: &SyntaxNode) -> (Vec<String>, Option<NameId>) {
        let store = token_after(node, "store").map(|token| self.named(&token));
        (self.strings(node), store)
    }

    fn sections(&mut self, node: &SyntaxNode) {
        let mut clauses = Vec::new();
        for element in node.children_with_tokens() {
            match element {
                SyntaxElement::Node(child) if child.kind() == K::Terminators => {
                    let (terminators, store) = self.terminators(&child);
                    clauses.push(SectionClause::Until { terminators, store });
                }
                SyntaxElement::Token(token) if token.text() == "skip" => {
                    clauses.push(SectionClause::SkipOther);
                }
                SyntaxElement::Node(_) | SyntaxElement::Token(_) => {}
            }
        }
        self.event(Event::Sections(clauses));
        self.open_closures(1);
    }

    fn arm(&mut self, node: &SyntaxNode, pattern: Vec<Piece>, block: bool) {
        self.event(Event::Arm {
            pattern,
            optional: has_token(node, "optional"),
            last: has_token(node, "last"),
            block,
        });
    }

    /// Emit the event that a block's enclosing statement opens it with.
    fn open_block(&mut self, node: &SyntaxNode) {
        let frame_index = self.frames.len() - 1;
        let (parent, blocks) = {
            let parent = &mut self.frames[frame_index];
            parent.blocks += 1;
            (parent.kind, parent.blocks)
        };
        match parent {
            K::If => {
                let nested = self.frames[frame_index - 1].kind == K::If;
                let event = match (blocks, self.frames[frame_index].exprs.first().copied()) {
                    (1, Some(condition)) if nested => Event::ElseIf(condition),
                    (1, Some(condition)) => Event::If(condition),
                    _ => Event::Else,
                };
                self.event(event);
            }
            K::Repeat => self.repeat(node),
            K::Arm => {
                let frame = &mut self.frames[frame_index];
                let pattern = std::mem::take(&mut frame.pieces);
                let arm = frame.node.clone();
                self.arm(&arm, pattern, true);
            }
            _ => self.event(Event::Instances),
        }
        self.open_closures(1);
        self.type_scopes.push(self.types.len());
    }

    fn repeat(&mut self, block: &SyntaxNode) {
        let frame = self.frames.last().expect("a repeat's frame is open");
        let repeat = frame.node.clone();
        let times = frame.exprs.first().copied();
        let count = if let Some(terminators) = repeat
            .children()
            .find(|child| child.kind() == K::Terminators)
        {
            let (terminators, store) = self.terminators(&terminators);
            RepeatCount::Until { terminators, store }
        } else if let Some(times) = times {
            RepeatCount::Times(times)
        } else {
            RepeatCount::UntilEof
        };
        debug_assert_eq!(block.parent().as_ref(), Some(&repeat));
        let id = self.repeats[&repeat.text_range().start()];
        let names = match self.records[id.0] {
            Some(record) => RepeatNames::Forward(record),
            None => {
                let (each, list) = repeat_names(&repeat, Decoding::Source);
                RepeatNames::InPlace { each, list }
            }
        };
        self.event(Event::Repeat { count, names });
    }

    fn declared(frame: &mut Frame) -> Declared {
        Declared {
            ty: frame.ty.take().expect("a checked reading has a type"),
            dimensions: std::mem::take(&mut frame.dimensions),
            layout: frame.layout,
            offset: frame.offset,
        }
    }

    fn declaration(&mut self, mut frame: Frame) {
        let target = self.target(&defined_name(&frame.node, 0));
        let declared = Self::declared(&mut frame);
        let event = if self.parent_kind() == Some(K::If) {
            Event::ElseRead { target, declared }
        } else {
            Event::Read { target, declared }
        };
        self.event(event);
    }

    fn list(&mut self, mut frame: Frame) {
        let target = self.target(&defined_name(&frame.node, 1));
        let end = match frame.exprs.first() {
            Some(&end) => ListEnd::Expr(end),
            None => ListEnd::Eol,
        };
        let declared = Self::declared(&mut frame);
        self.event(Event::List {
            target,
            declared,
            end,
        });
    }

    fn type_definition(&mut self, mut frame: Frame) {
        let name = self.token_text(&defined_name(&frame.node, 1));
        let ty = frame.ty.take().expect("a type definition has a type");
        self.types.push(NamedType {
            name: name.clone(),
            event: self.program.events.len(),
        });
        self.event(Event::DefineType {
            name,
            ty,
            used: false,
        });
    }

    fn type_name(&mut self, node: &SyntaxNode) {
        let token = node
            .descendants_with_tokens()
            .filter_map(SyntaxElement::into_token)
            .find(|token| token.kind() == K::Ident)
            .expect("a type name has an identifier");
        let name = self.token_text(&token);
        if let Some(visible) = self.types.iter().rev().find(|ty| ty.name == name)
            && let Event::DefineType { used, .. } = &mut self.program.events[visible.event]
        {
            *used = true;
        }
        // The type name's own frame is not open yet, so this is its parent's.
        self.top().ty = Some(Type::Named(name));
    }

    fn alphabet(&self, node: &SyntaxNode) -> Type {
        let options = node
            .children()
            .filter(|child| child.kind() == K::AlphabetOption)
            .map(|option| self.alphabet_option(&option))
            .collect();
        Type::Alphabet {
            symbols: self.string(node),
            options,
        }
    }

    fn alphabet_option(&self, node: &SyntaxNode) -> AlphabetOption {
        let keyword = tokens(node).next().expect("an option has a keyword");
        match self.token_text(&keyword).as_str() {
            "blank" => AlphabetOption::Blank(self.string(node)),
            "strip" => AlphabetOption::Strip(self.string(node)),
            _ => {
                let integer = tokens(node)
                    .find(|token| token.kind() == K::Integer)
                    .expect("a checked base has an integer");
                let value: i64 = self
                    .token_text(&integer)
                    .parse()
                    .expect("the lexer accepts only integers that fit");
                AlphabetOption::Base(if has_token(node, "-") { -value } else { value })
            }
        }
    }

    fn pattern_text(&mut self, node: &SyntaxNode) {
        let raw = tokens(node)
            .next()
            .expect("a text item has its token")
            .text()
            .to_owned();
        let text = decode_raw(&raw);
        let pieces = &mut self.top().pieces;
        match pieces.last_mut() {
            Some(Piece::Text(last)) => last.push_str(&text),
            _ => pieces.push(Piece::Text(text)),
        }
    }

    fn current_value(&mut self, node: &SyntaxNode) {
        let token = tokens(node)
            .find(|token| token.kind() == K::Ident)
            .expect("a current value has a name");
        let name = self.name(decode_raw(token.text()));
        self.top().pieces.push(Piece::Reference(name));
    }

    fn capture(&mut self, mut frame: Frame) {
        self.captures -= 1;
        let name = tokens(&frame.node)
            .find(|token| token.kind() == K::Ident)
            .map(|token| decode_raw(token.text()));
        let ty = frame.ty.take();
        let piece = match (name.as_deref(), ty) {
            (None, None) => Piece::Blank,
            (None, Some(ty)) => Piece::Validate(ty),
            (Some("_"), None) => Piece::Discard,
            (Some("_"), Some(ty)) => Piece::DiscardAs(ty),
            (Some(_), ty) => Piece::Bind {
                name: self.name(name.expect("the match found a name")),
                ty,
            },
        };
        self.top().pieces.push(piece);
    }

    /// Start a comprehension's body: its domain has just ended.
    fn comprehension_part(&mut self) {
        let frame = self.frames.last().expect("the comprehension's frame");
        if frame.exprs.len() != 1 {
            return;
        }
        let binding = defined_name(&frame.node, 1);
        let name = self.token_text(&binding);
        let variable = self.variable(name);
        self.program.exprs.push(ExprOp::ComprehensionBody(variable));
        self.open_closures(1);
    }

    fn resolved_name(&mut self, token: &SyntaxToken) -> ExprOp {
        let text = self.token_text(token);
        match self.resolution(token) {
            Some(Resolution::Bound(_)) => ExprOp::Var(self.variable(text)),
            Some(Resolution::Structural {
                denotation: Denotation::Record(repeat),
                ..
            }) => ExprOp::Each(self.record(*repeat)),
            Some(Resolution::Structural {
                denotation: Denotation::RecordList(repeat),
                ..
            }) => ExprOp::List(self.record(*repeat)),
            _ => ExprOp::Name(self.name(text)),
        }
    }

    fn resolution(&self, token: &SyntaxToken) -> Option<&'a Resolution> {
        self.references
            .get(&token.text_range().start())
            .map(|reference| &reference.resolution)
    }

    fn record(&self, repeat: RepeatId) -> RecordId {
        self.records[repeat.0].expect("a referenced repeat has a record")
    }

    fn expression(&mut self, frame: &Frame) {
        let op = match frame.kind {
            K::Literal => Some(self.literal(&frame.node)),
            K::HostConstant => Some(ExprOp::Host(host_constant(&frame.node))),
            K::Name => {
                let token = tokens(&frame.node).next().expect("a name has its token");
                Some(self.resolved_name(&token))
            }
            K::Unary => Some(ExprOp::Unary(unary_op(&frame.node))),
            K::Binary => Some(ExprOp::Binary(binary_op(&frame.node))),
            K::Power => Some(ExprOp::Power),
            K::IndexExpression => Some(ExprOp::Index),
            K::Field => Some(self.member(&frame.node)),
            K::Call => Some(ExprOp::Call {
                function: function(&frame.node),
                arguments: frame.exprs.len(),
            }),
            K::Comprehension => {
                self.closures -= 1;
                Some(ExprOp::Comprehension(comprehension_kind(&frame.node)))
            }
            _ => None,
        };
        if let Some(op) = op {
            self.program.exprs.push(op);
        }
        let expr = Expr {
            start: frame.start,
            end: self.program.exprs.len(),
        };
        self.top().exprs.push(expr);
    }

    fn literal(&self, node: &SyntaxNode) -> ExprOp {
        let token = tokens(node).next().expect("a literal has its token");
        match token.kind() {
            K::Integer => ExprOp::Int(
                self.token_text(&token)
                    .parse()
                    .expect("the lexer accepts only integers that fit"),
            ),
            K::Float => ExprOp::Float(
                self.token_text(&token)
                    .parse()
                    .expect("the lexer accepts only valid floats"),
            ),
            _ => ExprOp::Str(self.string_text(&token)),
        }
    }

    fn member(&mut self, node: &SyntaxNode) -> ExprOp {
        let token = tokens(node)
            .find(|token| token.kind() == K::Ident)
            .expect("a field has a name");
        if let Some(Resolution::Structural {
            denotation: Denotation::RecordList(repeat),
            ..
        }) = self.resolution(&token)
        {
            return ExprOp::ListMember(self.record(*repeat));
        }
        ExprOp::Member(self.named(&token))
    }
}

/// How the text of a token relates to the text it means.
#[derive(Clone, Copy)]
enum Decoding {
    /// A token of the spec.
    Source,
    /// A token inside a capture type, itself inside a pattern string.
    Capture,
}

impl Decoding {
    /// An identifier or number.
    fn plain(self, token: &SyntaxToken) -> String {
        match self {
            Self::Source => token.text().to_owned(),
            Self::Capture => decode_raw(token.text()),
        }
    }

    /// A string literal, with its quotes.
    fn string(self, token: &SyntaxToken) -> String {
        match self {
            Self::Source => decode_quoted(token.text()),
            Self::Capture => decode_quoted(&decode_raw(token.text())),
        }
    }
}

/// The `each` and `as` names written on a repeat.
fn repeat_names(repeat: &SyntaxNode, decoding: Decoding) -> (Option<String>, Option<String>) {
    let list = token_after(repeat, "as").map(|token| decoding.plain(&token));
    let each = repeat
        .children()
        .find(|child| child.kind() == K::Block)
        .and_then(|block| block.children().find(|child| child.kind() == K::Each))
        .map(|each| decoding.plain(&defined_name(&each, 1)));
    (each, list)
}

fn only_expr(frame: &Frame) -> Expr {
    *frame
        .exprs
        .first()
        .expect("a checked statement has its expression")
}

fn is_expression(kind: K) -> bool {
    matches!(
        kind,
        K::Literal
            | K::Name
            | K::HostConstant
            | K::Parenthesized
            | K::Unary
            | K::Binary
            | K::Power
            | K::Call
            | K::Comprehension
            | K::IndexExpression
            | K::Field
    )
}

fn range(frame: &Frame) -> Type {
    let first = frame
        .node
        .children_with_tokens()
        .find(|element| !is_trivia(element.kind()))
        .expect("a range has its operator");
    let starts_with_operator = first.kind() == K::Punct;
    let exclusive = has_token(&frame.node, "..<");
    let (lower, upper) = match (frame.exprs.as_slice(), starts_with_operator) {
        (&[lower, upper], _) => (Some(lower), Some(upper)),
        (&[upper], true) => (None, Some(upper)),
        (&[lower], false) => (Some(lower), None),
        _ => (None, None),
    };
    Type::Range {
        lower,
        upper,
        exclusive,
    }
}

fn base_type(name: &str) -> BaseType {
    match name {
        "int" => BaseType::Int,
        "float" => BaseType::Float,
        "num" => BaseType::Num,
        "word" => BaseType::Word,
        "rest" => BaseType::Rest,
        other => unreachable!("the parser has no base type '{other}'"),
    }
}

fn host_constant(node: &SyntaxNode) -> Host {
    if has_token(node, "gecode.int_max") {
        Host::IntMax
    } else {
        Host::IntMin
    }
}

fn unary_op(node: &SyntaxNode) -> UnaryOp {
    if has_token(node, "-") {
        UnaryOp::Minus
    } else {
        UnaryOp::Not
    }
}

fn binary_op(node: &SyntaxNode) -> BinaryOp {
    let operator = tokens(node).next().expect("a binary node has its operator");
    match operator.text() {
        "||" => BinaryOp::Or,
        "&&" => BinaryOp::And,
        "==" => BinaryOp::Equal,
        "!=" => BinaryOp::NotEqual,
        "<" => BinaryOp::Less,
        "<=" => BinaryOp::LessEqual,
        ">" => BinaryOp::Greater,
        ">=" => BinaryOp::GreaterEqual,
        "+" => BinaryOp::Add,
        "-" => BinaryOp::Subtract,
        "*" => BinaryOp::Multiply,
        "/" => BinaryOp::Divide,
        "%" => BinaryOp::Remainder,
        other => unreachable!("the parser has no binary operator '{other}'"),
    }
}

fn callee(node: &SyntaxNode) -> String {
    tokens(node)
        .next()
        .expect("a call has its callee")
        .text()
        .to_owned()
}

fn function(node: &SyntaxNode) -> Function {
    match callee(node).as_str() {
        "defined" => Function::Defined,
        "size" => Function::Size,
        "count" => Function::Count,
        "min" => Function::Min,
        "max" => Function::Max,
        "sum" => Function::Sum,
        "distinct" => Function::Distinct,
        other => unreachable!("a checked spec has no function '{other}'"),
    }
}

fn comprehension_kind(node: &SyntaxNode) -> ComprehensionKind {
    match callee(node).as_str() {
        "all" => ComprehensionKind::All,
        "any" => ComprehensionKind::Any,
        "sum" => ComprehensionKind::Sum,
        "count" => ComprehensionKind::Count,
        other => unreachable!("the parser has no comprehension '{other}'"),
    }
}
