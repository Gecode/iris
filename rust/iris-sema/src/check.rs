use crate::{Error, ErrorCode as E, model::*, syntax::*};
use iris_syntax::{Parse, ParseCheck, ParseCheckKind as C, SyntaxKind as K, SyntaxNode, TextSize};
use std::collections::HashMap;

type Result<T = ()> = std::result::Result<T, Error>;

pub(crate) fn check(parsed: &Parse, model: &mut SemanticModel) -> Result {
    StatementChecks {
        parsed,
        root: parsed.syntax(),
        model,
        reading_types: HashMap::new(),
        completed_types: vec![],
        completed_bindings: vec![],
    }
    .run()
}
struct StatementChecks<'a> {
    parsed: &'a Parse,
    root: SyntaxNode,
    model: &'a mut SemanticModel,
    reading_types: HashMap<TextSize, ReadingKind>,
    completed_types: Vec<DefinitionId>,
    completed_bindings: Vec<DefinitionId>,
}
impl StatementChecks<'_> {
    fn run(mut self) -> Result {
        for event in self.parsed.checks() {
            self.check_event(event)?;
        }
        self.check_global_conflicts()?;
        for definition in &mut self.model.definitions {
            if definition.kind == DefinitionKind::Capture
                && definition.facts == ValueFacts::unknown()
            {
                definition.facts = ReadingKind::Word.facts(0);
            }
        }
        Ok(())
    }
    fn check_event(&mut self, event: &ParseCheck) -> Result {
        match event.kind {
            C::Diagnostic(index) => Err(Error::from(&self.parsed.diagnostics()[index])),
            C::TypeDefinitionName => self.check_type_name(event),
            C::TypeUse => self.check_type_use(event),
            C::TypeDefinitionComplete => self.complete_type(event),
            C::ReadForm | C::CaptureForm => self.check_read_form(event),
            C::Layout | C::Offset => self.check_option(event),
            C::BindingName => self.check_binding_name(event),
            C::ComprehensionComplete => self.complete_comprehension(event),
        }
    }
    fn check_type_name(&mut self, event: &ParseCheck) -> Result {
        let definition = self
            .model
            .definitions
            .iter()
            .find(|d| d.name_range.start() == event.target && d.kind == DefinitionKind::NamedType)
            .expect("type name checkpoint");
        if matches!(
            definition.name.as_str(),
            "int" | "float" | "num" | "word" | "rest" | "alphabet"
        ) {
            return Err(error(
                event,
                E::NameConflict,
                format!("'{}' is a base type", definition.name),
            ));
        }
        if visible_type(
            self.model,
            &definition.name,
            definition.scope,
            &self.completed_types,
        )
        .is_some()
        {
            return Err(error(
                event,
                E::NameConflict,
                format!("the type '{}' is already defined", definition.name),
            ));
        }

        Ok(())
    }
    fn check_type_use(&mut self, event: &ParseCheck) -> Result {
        let node = at_start(&self.root, K::Type, event.target).expect("type checkpoint");
        let child = node.children().next().expect("completed type");
        let kind = match child.kind() {
            K::BaseType => match child.text().to_string().as_str() {
                "int" => ReadingKind::Int,
                "float" => ReadingKind::Float,
                "num" => ReadingKind::Num,
                "word" => ReadingKind::Word,
                "rest" => ReadingKind::Rest,
                _ => unreachable!("base type spelling"),
            },
            K::Range => ReadingKind::Range,
            K::Alphabet => ReadingKind::Alphabet,
            K::TypeName => {
                let token = child
                    .descendants_with_tokens()
                    .filter_map(|e| e.into_token())
                    .find(|t| t.kind() == K::Ident)
                    .expect("name type");
                // Parenthesized names were parsed as bounds, not alias uses.
                let alias = child
                    .children()
                    .next()
                    .is_none()
                    .then(|| {
                        visible_type(
                            self.model,
                            token.text(),
                            scope_of(self.model, &node),
                            &self.completed_types,
                        )
                    })
                    .flatten();
                let Some(id) = alias else {
                    return Err(error(
                        event,
                        E::UnknownType,
                        format!("unknown type '{}'", token.text()),
                    ));
                };
                let kind = self
                    .model
                    .named_types
                    .iter()
                    .find(|t| t.definition == id)
                    .expect("completed named type")
                    .kind;
                self.model.add_reference(Reference {
                    name: token.text().into(),
                    name_range: token.text_range(),
                    resolution: Resolution::NamedType(id),
                    facts: kind.facts(0),
                });
                kind
            }
            _ => unreachable!("type production"),
        };
        self.reading_types.insert(event.target, kind);

        Ok(())
    }
    fn complete_type(&mut self, event: &ParseCheck) -> Result {
        let id = self
            .model
            .definitions
            .iter()
            .position(|d| {
                d.name_range.start() == event.target && d.kind == DefinitionKind::NamedType
            })
            .map(DefinitionId)
            .expect("completed type definition");
        let node = self
            .model
            .definition(id)
            .node
            .children()
            .find(|n| n.kind() == K::Type)
            .expect("type RHS");
        let kind = self.reading_types[&node.text_range().start()];
        self.model.named_types.push(NamedType {
            definition: id,
            kind,
            source: node,
        });
        self.completed_types.push(id);

        Ok(())
    }
    fn check_read_form(&mut self, event: &ParseCheck) -> Result {
        let node = at_start(&self.root, K::Type, event.target).expect("read form type");
        let owner = node.parent().expect("type owner");
        let kind = self.reading_types[&event.target];
        let dimensions = owner
            .children()
            .filter(|n| n.kind() == K::Dimension)
            .count();
        let message = if event.kind == C::CaptureForm {
            (kind == ReadingKind::Alphabet).then_some("a capture cannot be of an alphabet type")
        } else if owner.kind() == K::List {
            matches!(kind, ReadingKind::Rest | ReadingKind::Alphabet)
                .then_some("a list cannot be of 'rest' or of an alphabet type")
        } else if kind == ReadingKind::Rest && dimensions > 0 {
            Some("'rest' values cannot be arrays")
        } else if kind == ReadingKind::Alphabet && dimensions != 2 {
            Some("an alphabet type needs a two-dimensional array")
        } else {
            None
        };
        if let Some(message) = message {
            let code = if event.kind == C::CaptureForm {
                E::Syntax(iris_syntax::DiagnosticCode::Pattern)
            } else {
                E::Type
            };
            let mut diagnostic = error(event, code, message.into());
            if event.kind == C::CaptureForm
                && let Some(pattern) = owner.ancestors().find(|n| n.kind() == K::Pattern)
            {
                diagnostic.location.line = line_of(self.parsed, pattern.text_range().start());
            }
            return Err(diagnostic);
        }
        if let Some(id) = definition_for(self.model, &owner) {
            let depth = if owner.kind() == K::List {
                1
            } else {
                dimensions
            };
            self.model.definitions[id.0].facts = kind.facts(depth);
        }

        Ok(())
    }
    fn check_option(&mut self, event: &ParseCheck) -> Result {
        let option_kind = if event.kind == C::Layout {
            K::Layout
        } else {
            K::Offset
        };
        let node = at_start(&self.root, option_kind, event.target).expect("option checkpoint");
        let owner = node.parent().expect("option owner");
        let type_node = owner
            .children()
            .find(|n| n.kind() == K::Type)
            .expect("reading type");
        let kind = self.reading_types[&type_node.text_range().start()];
        let dimensions = owner
            .children()
            .filter(|n| n.kind() == K::Dimension)
            .count();
        if event.kind == C::Layout && !(kind.numeric() && dimensions == 2) {
            return Err(error(
                event,
                E::Type,
                "'layout' needs a two-dimensional array of numbers".into(),
            ));
        }
        if event.kind == C::Offset && !kind.numeric() {
            return Err(error(event, E::Type, "'offset' needs numbers".into()));
        }

        Ok(())
    }
    fn check_binding_name(&mut self, event: &ParseCheck) -> Result {
        let binding = self
            .model
            .definitions
            .iter()
            .find(|d| d.kind == DefinitionKind::Binding && d.name_range.start() == event.target)
            .expect("binding checkpoint");
        for ancestor in binding
            .node
            .ancestors()
            .skip(1)
            .filter(|n| n.kind() == K::Comprehension)
        {
            let parts = expressions(&ancestor);
            if parts
                .get(1)
                .is_some_and(|body| body.text_range().contains_range(binding.node.text_range()))
                && definition_token(&ancestor).is_some_and(|t| t.text() == binding.name)
            {
                return Err(error(
                    event,
                    E::NameConflict,
                    format!(
                        "'{}' is already bound by an enclosing comprehension",
                        binding.name
                    ),
                ));
            }
        }

        Ok(())
    }
    fn complete_comprehension(&mut self, event: &ParseCheck) -> Result {
        let node =
            at_start(&self.root, K::Comprehension, event.target).expect("comprehension checkpoint");
        self.completed_bindings
            .push(definition_for(self.model, &node).expect("binding definition"));

        Ok(())
    }
    fn check_global_conflicts(&self) -> Result {
        for &id in &self.completed_bindings {
            let definition = self.model.definition(id);
            if self
                .model
                .definitions
                .iter()
                .any(|d| d.is_ordinary() && d.name == definition.name)
            {
                let mut diagnostic = at_node(
                    self.parsed,
                    &definition.node,
                    E::NameConflict,
                    format!(
                        "'{}' is bound by a comprehension but is also a name in the spec",
                        definition.name
                    ),
                );
                diagnostic.location.line = line_of(self.parsed, definition.name_range.start());
                diagnostic.range = definition.name_range;
                return Err(diagnostic);
            }
        }
        for &id in &self.completed_types {
            let definition = self.model.definition(id);
            if self
                .model
                .definitions
                .iter()
                .any(|d| d.is_ordinary() && d.name == definition.name)
            {
                return Err(at_node(
                    self.parsed,
                    &definition.node,
                    E::NameConflict,
                    format!(
                        "'{}' names a type, so it cannot also name a value",
                        definition.name
                    ),
                ));
            }
        }

        Ok(())
    }
}

fn visible_type(
    model: &SemanticModel,
    name: &str,
    mut scope: ScopeId,
    completed: &[DefinitionId],
) -> Option<DefinitionId> {
    loop {
        if let Some(id) = completed.iter().rev().find(|id| {
            let d = model.definition(**id);
            d.name == name && d.scope == scope
        }) {
            return Some(*id);
        }
        scope = model.scopes[scope.0].parent?;
    }
}
fn at_start(root: &SyntaxNode, kind: K, start: TextSize) -> Option<SyntaxNode> {
    root.descendants().find(|n| {
        n.kind() == kind
            && (n.text_range().start() == start
                || n.descendants_with_tokens()
                    .filter_map(|e| e.into_token())
                    .find(|t| !matches!(t.kind(), K::Whitespace | K::Comment))
                    .is_some_and(|t| t.text_range().start() == start))
    })
}
fn error(event: &ParseCheck, code: E, message: String) -> Error {
    Error {
        code,
        message,
        location: event.location.clone(),
        range: event.range,
    }
}
pub(crate) fn line_of(parsed: &Parse, offset: TextSize) -> usize {
    parsed.syntax().text().to_string().as_bytes()[..usize::from(offset)]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}
pub(crate) fn at_node(parsed: &Parse, node: &SyntaxNode, code: E, message: String) -> Error {
    Error {
        code,
        message,
        range: node.text_range(),
        location: iris_syntax::SourceLocation {
            origin: parsed.origin().into(),
            line: line_of(parsed, node.text_range().start()),
            column: None,
        },
    }
}
