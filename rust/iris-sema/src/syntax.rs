//! CST indexing shared by the checker, resolver, and fact inference.
use crate::model::*;
use iris_syntax::{SyntaxKind as K, SyntaxNode, SyntaxToken, TextSize};
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
fn collect_children(node: &SyntaxNode, scope: ScopeId, model: &mut SemanticModel) {
    for child in node.children() {
        collect_node(&child, scope, model);
    }
}
fn collect_node(node: &SyntaxNode, scope: ScopeId, model: &mut SemanticModel) {
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
            chain: chain.clone(),
            body_scope: None,
            list_definition: list,
            each_definition: None,
            fields: vec![],
        });
        for child in node.children() {
            if child.kind() == K::Block {
                let body_scope = add_scope(&child, scope, true, chain.clone(), model);
                model.repeats[id.0].body_scope = Some(body_scope);
                if let Some(each) = child.children().find(|n| n.kind() == K::Each)
                    && let Some(token) = definition_token(&each)
                {
                    let each_id =
                        add_definition(model, &each, token, DefinitionKind::EachRecord, body_scope);
                    model.repeats[id.0].each_definition = Some(each_id);
                }
                collect_children(&child, body_scope, model);
            } else {
                collect_node(&child, scope, model);
            }
        }
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
        collect_children(node, child_scope, model);
    } else {
        collect_children(node, scope, model);
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
