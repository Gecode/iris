//! CST indexing shared by the checker, resolver, and fact inference.
use crate::model::*;
use gecode_iris_syntax::{SyntaxKind as K, SyntaxNode, SyntaxToken, TextSize};
use std::collections::HashMap;

pub(crate) fn tokens(node: &SyntaxNode) -> Vec<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !matches!(t.kind(), K::Whitespace | K::Comment))
        .collect()
}
pub(crate) fn expressions(node: &SyntaxNode) -> Vec<SyntaxNode> {
    node.children()
        .filter(|n| is_expression(n.kind()))
        .collect()
}
pub(crate) fn is_expression(kind: K) -> bool {
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
/// Whether an expression of `kind` has a left operand that may nest without
/// limit. Specification §5.12 releases the entries of a parenthesized left
/// operand, so `((1+1)+1)+1` can continue for tens of thousands of operators,
/// while every other operand holds an entry. Walks therefore loop over
/// [`left_spine`] and recurse only into the other operands, at most 256 deep.
pub(crate) fn has_left_operand(kind: K) -> bool {
    matches!(
        kind,
        K::Binary | K::Power | K::IndexExpression | K::Field | K::Parenthesized
    )
}
/// Returns `node` and its chain of left operands, outermost first. The last
/// node has no left operand.
pub(crate) fn left_spine(node: &SyntaxNode) -> Vec<SyntaxNode> {
    let mut spine = vec![node.clone()];
    while let Some(last) = spine.last()
        && has_left_operand(last.kind())
        && let Some(left) = last.children().find(|n| is_expression(n.kind()))
    {
        spine.push(left);
    }
    spine
}
pub(crate) fn unparenthesized(node: &SyntaxNode) -> SyntaxNode {
    let mut node = node.clone();
    while node.kind() == K::Parenthesized {
        let Some(inner) = node.children().next() else {
            break;
        };
        node = inner;
    }
    node
}
pub(crate) fn first_ident(node: &SyntaxNode) -> Option<SyntaxToken> {
    tokens(node).into_iter().find(|t| t.kind() == K::Ident)
}
pub(crate) fn definition_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    let tokens = tokens(node);
    match node.kind() {
        K::Declaration | K::Capture => tokens.into_iter().find(|t| t.kind() == K::Ident),
        _ => tokens.into_iter().filter(|t| t.kind() == K::Ident).nth(1),
    }
}
/// Immutable lookups for the definitions and scopes collected before checking.
/// Keep these outside the public model; analysis never changes their identities.
pub(crate) struct SyntaxIndex {
    scopes: HashMap<SyntaxNode, ScopeId>,
    definitions: HashMap<SyntaxNode, Vec<DefinitionId>>,
    names: HashMap<TextSize, DefinitionId>,
    repeats: HashMap<SyntaxNode, usize>,
    ordinary_by_start: Vec<DefinitionId>,
    ordinary: HashMap<String, Vec<DefinitionId>>,
    structural: HashMap<String, (RepeatId, DefinitionId, bool)>,
    checkpoints: HashMap<(K, TextSize), SyntaxNode>,
}
impl SyntaxIndex {
    pub(crate) fn new(model: &SemanticModel) -> Self {
        let mut index = Self {
            scopes: HashMap::new(),
            definitions: HashMap::new(),
            names: HashMap::new(),
            repeats: HashMap::new(),
            ordinary_by_start: vec![],
            ordinary: HashMap::new(),
            structural: HashMap::new(),
            checkpoints: HashMap::new(),
        };
        for (i, scope) in model.scopes.iter().enumerate() {
            crate::work::record();
            index.scopes.insert(scope.node.clone(), ScopeId(i));
        }
        for (i, definition) in model.definitions.iter().enumerate() {
            crate::work::record();
            let id = DefinitionId(i);
            if definition.is_ordinary() {
                index.ordinary_by_start.push(id);
                index
                    .ordinary
                    .entry(definition.name.clone())
                    .or_default()
                    .push(id);
            }
            index
                .definitions
                .entry(definition.node.clone())
                .or_default()
                .push(id);
            index
                .names
                .entry(definition.name_range.start())
                .or_insert(id);
        }
        index
            .ordinary_by_start
            .sort_by_key(|id| model.definition(*id).node.text_range().start());
        for (i, repeat) in model.repeats.iter().enumerate() {
            crate::work::record();
            index.repeats.insert(repeat.node.clone(), i);
            for (id, list) in [
                (repeat.list_definition, true),
                (repeat.each_definition, false),
            ] {
                if let Some(id) = id {
                    index
                        .structural
                        .entry(model.definition(id).name.clone())
                        .or_insert((RepeatId(i), id, list));
                }
            }
        }
        for node in model.scopes[0].node.descendants() {
            crate::work::record();
            if !matches!(
                node.kind(),
                K::Type | K::Layout | K::Offset | K::Comprehension
            ) {
                continue;
            }
            index
                .checkpoints
                .entry((node.kind(), node.text_range().start()))
                .or_insert_with(|| node.clone());
            if let Some(token) = node
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .find(|t| {
                    crate::work::record();
                    !matches!(t.kind(), K::Whitespace | K::Comment)
                })
            {
                index
                    .checkpoints
                    .entry((node.kind(), token.text_range().start()))
                    .or_insert(node);
            }
        }
        index
    }
    pub(crate) fn ordinary_definitions(&self, name: &str) -> &[DefinitionId] {
        crate::work::record();
        self.ordinary.get(name).map_or(&[], Vec::as_slice)
    }
    pub(crate) fn structural(&self, name: &str) -> Option<(RepeatId, DefinitionId, bool)> {
        crate::work::record();
        self.structural.get(name).copied()
    }
    pub(crate) fn repeat_for(&self, node: &SyntaxNode) -> Option<usize> {
        crate::work::record();
        self.repeats.get(node).copied()
    }
    pub(crate) fn ordinary_definitions_in(
        &self,
        container: &SyntaxNode,
        model: &SemanticModel,
    ) -> Vec<DefinitionId> {
        let range = container.text_range();
        let start = self.ordinary_by_start.partition_point(|id| {
            crate::work::record();
            model.definition(*id).node.text_range().start() < range.start()
        });
        let mut ids: Vec<_> = self.ordinary_by_start[start..]
            .iter()
            .copied()
            .take_while(|id| {
                crate::work::record();
                model.definition(*id).node.text_range().start() <= range.end()
            })
            .filter(|id| range.contains_range(model.definition(*id).node.text_range()))
            .collect();
        // Admission order is definition order, including overlapping owner nodes.
        ids.sort_unstable();
        ids
    }
    pub(crate) fn scope_of(&self, node: &SyntaxNode) -> ScopeId {
        node.ancestors()
            .find_map(|a| {
                crate::work::record();
                self.scopes.get(&a).copied()
            })
            .unwrap_or(ScopeId(0))
    }
    pub(crate) fn definitions_for(&self, node: &SyntaxNode) -> &[DefinitionId] {
        crate::work::record();
        self.definitions.get(node).map_or(&[], Vec::as_slice)
    }
    pub(crate) fn definition_for(&self, node: &SyntaxNode) -> Option<DefinitionId> {
        self.definitions_for(node).first().copied()
    }
    pub(crate) fn definition_at(&self, start: TextSize) -> Option<DefinitionId> {
        crate::work::record();
        self.names.get(&start).copied()
    }
    pub(crate) fn at_start(&self, kind: K, start: TextSize) -> Option<SyntaxNode> {
        crate::work::record();
        self.checkpoints.get(&(kind, start)).cloned()
    }
}
pub(crate) fn collect(root: &SyntaxNode) -> SemanticModel {
    let mut model = SemanticModel::default();
    model.scopes.push(Scope {
        node: root.clone(),
        parent: None,
        type_scope: ScopeId(0),
        value_frame: ScopeId(0),
        repeats: vec![],
    });
    collect_children(root, ScopeId(0), &mut model);
    let mut fields_by_name: Vec<HashMap<String, usize>> = vec![HashMap::new(); model.repeats.len()];
    for (id, definition) in model.definitions.iter().enumerate() {
        crate::work::record();
        if !definition.is_ordinary() {
            continue;
        }
        for repeat in &definition.repeats {
            crate::work::record();
            let fields = &mut model.repeats[repeat.0].fields;
            let index = *fields_by_name[repeat.0]
                .entry(definition.name.clone())
                .or_insert_with(|| {
                    let index = fields.len();
                    fields.push(FieldInfo {
                        name: definition.name.clone(),
                        definitions: vec![],
                        fallback: ValueFacts::unknown(),
                    });
                    index
                });
            fields[index].definitions.push(DefinitionId(id));
        }
    }
    model
}
fn add_definition(
    model: &mut SemanticModel,
    node: &SyntaxNode,
    token: SyntaxToken,
    kind: DefinitionKind,
    scope: ScopeId,
) -> DefinitionId {
    let id = DefinitionId(model.definitions.len());
    model.definitions.push(Definition {
        name: token.text().into(),
        name_range: token.text_range(),
        kind,
        node: node.clone(),
        scope,
        repeats: model.scopes[scope.0].repeats.clone(),
        facts: ValueFacts::unknown(),
        references: vec![],
    });
    id
}
/// One step of the definition walk: a node to visit in a scope, or the body
/// of a repeat, which opens the repeat's own scope when it is reached.
enum Collect {
    Node(SyntaxNode, ScopeId),
    RepeatBody {
        body: SyntaxNode,
        scope: ScopeId,
        repeat: RepeatId,
    },
}

/// Visits the nodes below `root` in preorder. The walk keeps its own stack, as
/// left operands nest far deeper than any thread's stack (Specification §5.12
/// releases the entries of a parenthesized left operand).
fn collect_children(root: &SyntaxNode, scope: ScopeId, model: &mut SemanticModel) {
    let mut work = vec![];
    push_children(&mut work, root, scope);
    while let Some(step) = work.pop() {
        match step {
            Collect::Node(node, scope) => collect_node(&node, scope, model, &mut work),
            Collect::RepeatBody {
                body,
                scope,
                repeat,
            } => {
                let chain = model.repeats[repeat.0].chain.clone();
                let body_scope = add_scope(&body, scope, true, chain, model);
                model.repeats[repeat.0].body_scope = Some(body_scope);
                if let Some(each) = body.children().find(|n| n.kind() == K::Each)
                    && let Some(token) = definition_token(&each)
                {
                    let each_id =
                        add_definition(model, &each, token, DefinitionKind::EachRecord, body_scope);
                    model.repeats[repeat.0].each_definition = Some(each_id);
                }
                push_children(&mut work, &body, body_scope);
            }
        }
    }
}
/// Schedules the children of `node` so that they are visited in order.
fn push_children(work: &mut Vec<Collect>, node: &SyntaxNode, scope: ScopeId) {
    let start = work.len();
    work.extend(node.children().map(|child| Collect::Node(child, scope)));
    work[start..].reverse();
}
fn collect_node(
    node: &SyntaxNode,
    scope: ScopeId,
    model: &mut SemanticModel,
    work: &mut Vec<Collect>,
) {
    let kind = match node.kind() {
        K::Declaration | K::List => Some(DefinitionKind::Read),
        K::Let => Some(DefinitionKind::Let),
        K::Param => Some(DefinitionKind::Param),
        K::TypeDefinition => Some(DefinitionKind::NamedType),
        K::Capture => Some(DefinitionKind::Capture),
        K::Comprehension => Some(DefinitionKind::Binding),
        _ => None,
    };
    if let Some(kind) = kind
        && let Some(token) = definition_token(node)
        && (token.text() != "_" || !matches!(kind, DefinitionKind::Read | DefinitionKind::Capture))
    {
        add_definition(model, node, token, kind, scope);
    }
    if matches!(node.kind(), K::Repeat | K::Sections) {
        // Only the last store option survives parsing repeated sections options.
        if let Some(token) = node
            .children()
            .filter(|n| n.kind() == K::Terminators)
            .filter_map(|n| definition_token(&n))
            .last()
        {
            add_definition(model, node, token, DefinitionKind::Store, scope);
        }
    }
    if node.kind() == K::Repeat {
        let id = RepeatId(model.repeats.len());
        let mut chain = model.scopes[scope.0].repeats.clone();
        let parent = chain.last().copied();
        chain.push(id);
        let own = tokens(node);
        let list = own
            .iter()
            .position(|t| t.text() == "as")
            .and_then(|i| own.get(i + 1))
            .cloned()
            .map(|t| add_definition(model, node, t, DefinitionKind::RecordList, scope));
        model.repeats.push(RepeatInfo {
            node: node.clone(),
            parent,
            chain,
            body_scope: None,
            list_definition: list,
            each_definition: None,
            fields: vec![],
        });
        let start = work.len();
        work.extend(node.children().map(|child| {
            if child.kind() == K::Block {
                Collect::RepeatBody {
                    body: child,
                    scope,
                    repeat: id,
                }
            } else {
                Collect::Node(child, scope)
            }
        }));
        work[start..].reverse();
        return;
    }
    if node.kind() == K::Block {
        let fresh = node.parent().is_some_and(|n| n.kind() == K::Instances);
        let child_scope = add_scope(
            node,
            scope,
            fresh,
            model.scopes[scope.0].repeats.clone(),
            model,
        );
        push_children(work, node, child_scope);
    } else {
        push_children(work, node, scope);
    }
}
fn add_scope(
    node: &SyntaxNode,
    parent: ScopeId,
    fresh_frame: bool,
    repeats: Vec<RepeatId>,
    model: &mut SemanticModel,
) -> ScopeId {
    let id = ScopeId(model.scopes.len());
    let value_frame = if fresh_frame {
        id
    } else {
        model.scopes[parent.0].value_frame
    };
    model.scopes.push(Scope {
        node: node.clone(),
        parent: Some(parent),
        type_scope: id,
        value_frame,
        repeats,
    });
    id
}
