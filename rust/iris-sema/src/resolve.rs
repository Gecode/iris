//! Structural resolution in Specification §11.2 operand order.
use crate::{Error, ErrorCode as E, check::at_node, model::*, syntax::*};
use gecode_iris_syntax::{Parse, SyntaxKind as K, SyntaxNode};
type Result<T = ()> = std::result::Result<T, Error>;

pub(crate) fn resolve(parsed: &Parse, model: &mut SemanticModel, index: &SyntaxIndex) -> Result {
    precheck(parsed, model, index)?;
    let mut resolver = Resolver {
        parsed,
        model,
        index,
        statement: parsed.syntax(),
        repeats: vec![],
        bound: vec![],
        dependencies: vec![],
    };
    resolver.visit(&parsed.syntax())
}
fn precheck(parsed: &Parse, model: &SemanticModel, index: &SyntaxIndex) -> Result {
    let mut preceding = std::collections::HashSet::new();
    for repeat in &model.repeats {
        for (position, id) in [repeat.list_definition, repeat.each_definition]
            .into_iter()
            .enumerate()
        {
            let Some(id) = id else {
                continue;
            };
            let name = &model.definition(id).name;
            if position == 1
                && repeat
                    .list_definition
                    .is_some_and(|list| model.definition(list).name == *name)
            {
                return Err(at_node(
                    parsed,
                    &repeat.node,
                    E::NameConflict,
                    format!("'{name}' cannot name both the list and each record"),
                ));
            }
            if preceding.contains(name) {
                return Err(at_node(
                    parsed,
                    &repeat.node,
                    E::NameConflict,
                    format!("'{name}' already names records"),
                ));
            }
            if !index.ordinary_definitions(name).is_empty() {
                return Err(at_node(
                    parsed,
                    &repeat.node,
                    E::NameConflict,
                    format!("'{name}' names records, so it cannot also be defined"),
                ));
            }
        }
        for id in [repeat.list_definition, repeat.each_definition]
            .into_iter()
            .flatten()
        {
            preceding.insert(model.definition(id).name.clone());
        }
    }
    Ok(())
}
struct Resolver<'a> {
    parsed: &'a Parse,
    index: &'a SyntaxIndex,
    model: &'a mut SemanticModel,
    statement: SyntaxNode,
    repeats: Vec<RepeatId>,
    bound: Vec<(DefinitionId, Denotation)>,
    dependencies: Vec<LiftDependency>,
}
impl Resolver<'_> {
    fn fail<T>(&self, code: E, message: String) -> Result<T> {
        Err(at_node(self.parsed, &self.statement, code, message))
    }
    fn visit(&mut self, container: &SyntaxNode) -> Result {
        for statement in container.children() {
            self.visit_statement(&statement)?;
        }
        Ok(())
    }
    fn visit_statement(&mut self, statement: &SyntaxNode) -> Result {
        self.statement = statement.clone();
        self.dependencies.clear();
        self.repeats = self.model.scopes[self.index.scope_of(statement).0]
            .repeats
            .clone();
        self.resolve_components(statement)?;
        if statement.kind() == K::Check {
            self.check_chain(statement)?;
        }
        for body in statement
            .children()
            .filter(|n| n.kind() == K::Block || n.kind() == K::If || n.kind() == K::Declaration)
        {
            if body.kind() == K::Block {
                self.visit(&body)?;
            } else {
                self.visit_statement(&body)?;
            }
        }
        for arm in statement.children().filter(|n| n.kind() == K::Arm) {
            for body in arm.children().filter(|n| n.kind() == K::Block) {
                self.visit(&body)?;
            }
        }
        Ok(())
    }
    fn resolve_components(&mut self, statement: &SyntaxNode) -> Result {
        for node in statement.children().filter(|n| n.kind() == K::Type) {
            self.resolve_type_bounds(&node)?;
        }
        for pattern in statement.children().filter(|n| n.kind() == K::Pattern) {
            self.resolve_pattern_bounds(&pattern)?;
        }
        for arm in statement.children().filter(|n| n.kind() == K::Arm) {
            for pattern in arm.children().filter(|n| n.kind() == K::Pattern) {
                self.resolve_pattern_bounds(&pattern)?;
            }
        }
        for dimension in statement.children().filter(|n| n.kind() == K::Dimension) {
            for expression in expressions(&dimension) {
                self.resolve_value(&expression)?;
            }
        }
        // The list's direct expression is its sentinel. Other statements use
        // their direct expression as the main expression.
        if statement.kind() != K::List {
            for expression in expressions(statement) {
                self.resolve_value(&expression)?;
            }
        }
        for layout in statement.children().filter(|n| n.kind() == K::Layout) {
            for expression in expressions(&layout) {
                self.resolve_value(&expression)?;
            }
        }
        if statement.kind() == K::List {
            for expression in expressions(statement) {
                self.resolve_value(&expression)?;
            }
        }
        for offset in statement.children().filter(|n| n.kind() == K::Offset) {
            for expression in expressions(&offset) {
                self.resolve_value(&expression)?;
            }
        }
        Ok(())
    }
    fn resolve_type_bounds(&mut self, node: &SyntaxNode) -> Result {
        for range in node.children().filter(|n| n.kind() == K::Range) {
            for expression in expressions(&range) {
                self.resolve_value(&expression)?;
            }
        }
        Ok(())
    }
    fn resolve_pattern_bounds(&mut self, pattern: &SyntaxNode) -> Result {
        for capture in pattern.children().filter(|n| n.kind() == K::Capture) {
            for node in capture.children().filter(|n| n.kind() == K::Type) {
                self.resolve_type_bounds(&node)?;
            }
        }
        for current in pattern.children().filter(|n| n.kind() == K::CurrentValue) {
            if let Some(token) = first_ident(&current) {
                self.model.add_reference(Reference {
                    name: token.text().into(),
                    name_range: token.text_range(),
                    resolution: Resolution::Value {
                        definitions: vec![],
                        possibly_unavailable: true,
                        projection: vec![],
                    },
                    facts: ValueFacts::unknown(),
                });
            }
        }
        Ok(())
    }
    fn check_chain(&mut self, node: &SyntaxNode) -> Result {
        let mut deepest = 0;
        for i in 1..self.dependencies.len() {
            if self.dependencies[i].repeats.len() > self.dependencies[deepest].repeats.len() {
                deepest = i;
            }
        }
        for i in 0..self.dependencies.len() {
            if !self.dependencies[deepest]
                .repeats
                .starts_with(&self.dependencies[i].repeats)
            {
                return self.fail(E::CheckScope, format!("a check can only range over the records of one repeat, but '{}' and '{}' are defined in different repeats", self.dependencies[i.min(deepest)].label, self.dependencies[i.max(deepest)].label));
            }
        }
        self.model.checks.push(CheckInfo {
            node: node.clone(),
            deepest: self
                .dependencies
                .get(deepest)
                .map_or_else(Vec::new, |d| d.repeats.clone()),
            dependencies: self.dependencies.clone(),
        });
        Ok(())
    }
    fn beyond(&self, chain: &[RepeatId]) -> Vec<RepeatId> {
        let common = chain
            .iter()
            .zip(&self.repeats)
            .take_while(|(a, b)| a == b)
            .count();
        chain[common..].to_vec()
    }
    fn lift(&mut self, name: &str, repeats: Vec<RepeatId>) {
        if !repeats.is_empty()
            && !self
                .dependencies
                .iter()
                .any(|d| d.label == name && d.repeats == repeats)
        {
            self.dependencies.push(LiftDependency {
                label: name.into(),
                repeats,
            });
        }
    }
    fn resolve_value(&mut self, node: &SyntaxNode) -> Result<Denotation> {
        let denotation = self.resolve_expression(node, false)?;
        self.require_value(&denotation)?;
        Ok(denotation)
    }
    fn require_value(&self, denotation: &Denotation) -> Result {
        match denotation {
            Denotation::Value => Ok(()),
            Denotation::Record(r) => self.fail(
                E::RecordValue,
                format!(
                    "a record of '{}' is not a value; use one of its fields, as in '{}'",
                    self.records_name(*r),
                    self.example(*r, false)
                ),
            ),
            Denotation::RecordList(r) => {
                let list = self.list_name(*r);
                self.fail(E::RecordValue, format!("'{list}' is a list of records, not a value; use size({list}), an index, or a field of a record, as in '{}'", self.example(*r, true)))
            }
        }
    }
    fn resolve_expression(&mut self, node: &SyntaxNode, whole: bool) -> Result<Denotation> {
        let denotation = match node.kind() {
            K::Parenthesized => self.resolve_expression(&expressions(node)[0], whole)?,
            K::Name => self.resolve_name(node, whole)?,
            K::HostConstant => {
                let token = first_ident(node).expect("host name");
                let value = if token.text() == "gecode.int_max" {
                    2147483646
                } else {
                    -2147483646
                };
                self.model.add_reference(Reference {
                    name: token.text().into(),
                    name_range: token.text_range(),
                    resolution: Resolution::HostConstant(value),
                    facts: ValueFacts::known(ValueType::Int, 0),
                });
                Denotation::Value
            }
            K::Field => self.resolve_field(node)?,
            K::IndexExpression => {
                let args = expressions(node);
                let base = self.resolve_expression(&args[0], false)?;
                self.resolve_value(&args[1])?;
                match base {
                    Denotation::Record(r) => return self.fail(
                        E::RecordValue,
                        format!(
                            "a record of '{}' cannot be indexed; use one of its fields, as in '{}'",
                            self.records_name(r),
                            self.example(r, false)
                        ),
                    ),
                    Denotation::RecordList(r) => Denotation::Record(r),
                    Denotation::Value => Denotation::Value,
                }
            }
            K::Comprehension => self.resolve_comprehension(node)?,
            K::Call => {
                let callee = first_ident(node).expect("callee");
                for argument in expressions(node) {
                    let denotation =
                        self.resolve_expression(&argument, callee.text() == "defined")?;
                    if !matches!(callee.text(), "size" | "count" | "defined")
                        || !matches!(denotation, Denotation::RecordList(_))
                    {
                        self.require_value(&denotation)?;
                    }
                }
                Denotation::Value
            }
            _ => {
                for argument in expressions(node) {
                    self.resolve_value(&argument)?;
                }
                Denotation::Value
            }
        };
        self.model.expressions.push(ExpressionInfo {
            node: node.clone(),
            facts: ValueFacts::unknown(),
            denotation: denotation.clone(),
        });
        Ok(denotation)
    }
    fn resolve_name(&mut self, node: &SyntaxNode, whole: bool) -> Result<Denotation> {
        let token = first_ident(node).expect("name");
        let name = token.text();
        let mut projection = vec![];
        let (denotation, resolution) = if let Some((id, denotation)) = self
            .bound
            .iter()
            .rev()
            .find(|(id, _)| self.model.definition(*id).name == name)
        {
            (denotation.clone(), Resolution::Bound(*id))
        } else if let Some((r, id, list)) = self.index.structural(name) {
            let repeat = &self.model.repeats[r.0];
            let active = self.repeats.contains(&r);
            let denotation = if list {
                Denotation::RecordList(r)
            } else {
                Denotation::Record(r)
            };
            if list {
                if active {
                    return self.fail(
                        E::RecordScope,
                        format!("'{name}' cannot be used inside its own repeat"),
                    );
                }
                if !self
                    .beyond(&repeat.chain[..repeat.chain.len() - 1])
                    .is_empty()
                {
                    let parent = repeat.parent.expect("nested list");
                    let outer = self.each_name(parent);
                    let suggestion = if outer.is_empty() {
                        format!("name each of these records with 'each' and use its field '{name}'")
                    } else {
                        format!("use '{outer}.{name}'")
                    };
                    return self.fail(E::RecordScope, format!("'{name}' is a list in each record of the repeat around it; {suggestion}"));
                }
            } else if !active {
                if self.statement.kind() != K::Check {
                    return self.fail(E::RecordScope, format!("'{name}' is each record of a repeat, so only checks can use it outside that repeat"));
                }
                self.lift(name, self.beyond(&repeat.chain));
            }
            (
                denotation.clone(),
                Resolution::Structural {
                    definition: id,
                    denotation,
                },
            )
        } else {
            if self.statement.kind() == K::Check && !whole {
                let chains: Vec<_> = self
                    .index
                    .ordinary_definitions(name)
                    .iter()
                    .map(|id| self.beyond(&self.model.definition(*id).repeats))
                    .collect();
                if let Some(first) = chains.first() {
                    if chains.iter().any(|c| c != first) {
                        return self.fail(E::CheckScope, format!("'{name}' is defined in several repeats, or inside and outside a repeat, so a check here cannot range over its records"));
                    }
                    projection = first.clone();
                    self.lift(name, projection.clone());
                }
            }
            (
                Denotation::Value,
                Resolution::Value {
                    definitions: vec![],
                    possibly_unavailable: true,
                    projection,
                },
            )
        };
        self.model.add_reference(Reference {
            name: name.into(),
            name_range: token.text_range(),
            resolution,
            facts: ValueFacts::unknown(),
        });
        Ok(denotation)
    }
    fn list_name(&self, r: RepeatId) -> String {
        self.model.repeats[r.0]
            .list_definition
            .map_or_else(String::new, |id| self.model.definition(id).name.clone())
    }
    fn each_name(&self, r: RepeatId) -> String {
        self.model.repeats[r.0]
            .each_definition
            .map_or_else(String::new, |id| self.model.definition(id).name.clone())
    }
    fn records_name(&self, r: RepeatId) -> String {
        let list = self.list_name(r);
        if list.is_empty() {
            self.each_name(r)
        } else {
            list
        }
    }
    fn fields(&self, r: RepeatId) -> Vec<String> {
        let mut fields: Vec<_> = self.model.repeats[r.0]
            .fields
            .iter()
            .map(|f| f.name.clone())
            .collect();
        for nested in &self.model.repeats {
            if nested.parent == Some(r)
                && let Some(id) = nested.list_definition
            {
                let name = &self.model.definition(id).name;
                if !fields.contains(name) {
                    fields.push(name.clone());
                }
            }
        }
        fields
    }
    fn example(&self, r: RepeatId, list: bool) -> String {
        let each = self.each_name(r);
        let prefix = if list || each.is_empty() {
            format!("{}[0]", self.list_name(r))
        } else {
            each
        };
        self.fields(r)
            .first()
            .map_or(prefix.clone(), |f| format!("{prefix}.{f}"))
    }
    fn resolve_field(&mut self, node: &SyntaxNode) -> Result<Denotation> {
        let base = self.resolve_expression(&expressions(node)[0], false)?;
        let token = tokens(node)
            .into_iter()
            .find(|t| t.kind() == K::Ident)
            .expect("field token");
        let field = token.text();
        let r=match base {
            Denotation::Value => return self.fail(E::Field,format!("only records have fields such as '.{field}'; to name each record of a repeat, write 'repeat n as nodes {{ each node ... }}' and use 'node.{field}' or 'nodes[0].{field}'")),
            Denotation::RecordList(r) => {
                let list=self.list_name(r); let each=self.each_name(r);
                let suggestion=if each.is_empty() {"name each record with 'each' in the repeat and use its field".into()} else {format!("'{each}.{field}' for each record")};
                return self.fail(E::Field,format!("'{list}' is a list of records, so '.{field}' needs one of them: use '{list}[i].{field}', or {suggestion}"));
            }
            Denotation::Record(r)=>r,
        };
        if let Some((nested, id, true)) = self.index.structural(field) {
            let info = &self.model.repeats[nested.0];
            if info.chain[..info.chain.len() - 1].contains(&r) {
                if info.parent != Some(r) {
                    return self.fail(E::RecordScope,format!("'{field}' is inside a repeat without a list name in the records of '{}'; name that list with 'as' to reach it",self.records_name(r)));
                }
                if self.repeats.contains(&nested) {
                    return self.fail(
                        E::RecordScope,
                        format!("'{field}' cannot be used inside its own repeat"),
                    );
                }
                let denotation = Denotation::RecordList(nested);
                self.model.add_reference(Reference {
                    name: field.into(),
                    name_range: token.text_range(),
                    resolution: Resolution::Structural {
                        definition: id,
                        denotation: denotation.clone(),
                    },
                    facts: ValueFacts::unknown(),
                });
                return Ok(denotation);
            }
        }
        let definitions = self.index.ordinary_definitions(field).to_vec();
        if !definitions
            .iter()
            .any(|id| self.model.definition(*id).repeats.contains(&r))
        {
            let fields = self.fields(r);
            let known = if fields.is_empty() {
                "none".into()
            } else {
                fields.join(", ")
            };
            return self.fail(
                E::Field,
                format!(
                    "the records of '{}' have no field '{field}' (they have {known})",
                    self.records_name(r)
                ),
            );
        }
        if definitions
            .iter()
            .any(|id| !self.model.definition(*id).repeats.contains(&r))
        {
            return self.fail(E::Field,format!("'{field}' is defined inside and outside the repeat of '{}', so it cannot be a field of its records",self.records_name(r)));
        }
        for id in &definitions {
            let chain = &self.model.definition(*id).repeats;
            let common = chain
                .iter()
                .zip(&self.repeats)
                .take_while(|(a, b)| a == b)
                .count();
            if common
                > chain
                    .iter()
                    .position(|x| *x == r)
                    .expect("field membership")
                    + 1
            {
                return self.fail(
                    E::RecordScope,
                    format!("the field '{field}' cannot be used inside the repeat that reads it"),
                );
            }
        }
        self.model.add_reference(Reference {
            name: field.into(),
            name_range: token.text_range(),
            resolution: Resolution::Field {
                definitions,
                record: r,
            },
            facts: ValueFacts::unknown(),
        });
        Ok(Denotation::Value)
    }
    fn resolve_comprehension(&mut self, node: &SyntaxNode) -> Result<Denotation> {
        let id = self.index.definition_for(node).expect("binding");
        let name = self.model.definition(id).name.clone();
        if self.index.structural(&name).is_some() {
            return self.fail(
                E::NameConflict,
                format!("'{name}' is bound by a comprehension but names records"),
            );
        }
        let args = expressions(node);
        let domain = self.resolve_expression(&args[0], true)?;
        if let Denotation::Record(r) = domain {
            return self.fail(
                E::RecordValue,
                format!(
                    "a record of '{}' is not a list; use one of its fields",
                    self.records_name(r)
                ),
            );
        }
        let elements = match domain {
            Denotation::RecordList(r) => Some(r),
            _ => self.record_elements(&args[0]),
        };
        let binding = match domain {
            Denotation::RecordList(r) => Denotation::Record(r),
            _ => Denotation::Value,
        };
        self.bound.push((id, binding));
        self.resolve_value(&args[1])?;
        self.bound.pop();
        self.model.comprehensions.push(ComprehensionInfo {
            node: node.clone(),
            binding: id,
            domain: args[0].clone(),
            record_elements: elements,
        });
        Ok(Denotation::Value)
    }
    fn record_elements(&self, node: &SyntaxNode) -> Option<RepeatId> {
        let node = unparenthesized(node);
        let (name, parent) = match node.kind() {
            K::Name => {
                let name = first_ident(&node)?.text().to_owned();
                if self
                    .bound
                    .iter()
                    .any(|(id, _)| self.model.definition(*id).name == name)
                {
                    return None;
                }
                (name, None)
            }
            K::Field => {
                let base = self.model.expression(&expressions(&node)[0])?;
                let Denotation::Record(r) = base.denotation else {
                    return None;
                };
                (first_ident(&node)?.text().to_owned(), Some(r))
            }
            _ => return None,
        };
        let mut candidate = None;
        let mut found = false;
        for id in self.index.ordinary_definitions(&name) {
            let definition = self.model.definition(*id);
            let index = parent.map_or_else(
                || definition.repeats.len() - self.beyond(&definition.repeats).len(),
                |r| {
                    definition
                        .repeats
                        .iter()
                        .position(|x| *x == r)
                        .map_or(0, |p| p + 1)
                },
            );
            let current = definition.repeats.get(index).copied();
            if found && current != candidate {
                return None;
            }
            candidate = current;
            found = true;
        }
        candidate
    }
}
