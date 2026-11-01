//! CST indexing shared by the checker, resolver, and fact inference.
use crate::model::*;
use iris_syntax::{SyntaxKind as K, SyntaxNode, SyntaxToken};

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
pub(crate) fn scope_of(model: &SemanticModel, node: &SyntaxNode) -> ScopeId {
    node.ancestors()
        .find_map(|a| model.scopes.iter().position(|s| s.node == a).map(ScopeId))
        .unwrap_or(ScopeId(0))
}
pub(crate) fn definition_for(model: &SemanticModel, node: &SyntaxNode) -> Option<DefinitionId> {
    model
        .definitions
        .iter()
        .position(|d| d.node == *node)
        .map(DefinitionId)
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
    for index in 0..model.repeats.len() {
        let repeat = RepeatId(index);
        let mut fields: Vec<FieldInfo> = vec![];
        for (id, definition) in model.definitions.iter().enumerate() {
            if !definition.is_ordinary() || !definition.repeats.contains(&repeat) {
                continue;
            }
            if let Some(field) = fields.iter_mut().find(|f| f.name == definition.name) {
                field.definitions.push(DefinitionId(id));
            } else {
                fields.push(FieldInfo {
                    name: definition.name.clone(),
                    definitions: vec![DefinitionId(id)],
                    fallback: ValueFacts::unknown(),
                });
            }
        }
        model.repeats[index].fields = fields;
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
