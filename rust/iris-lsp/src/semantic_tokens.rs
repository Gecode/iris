use crate::document::Positions;
use iris_sema::{DefinitionKind, Resolution, SemanticModel};
use iris_syntax::{SyntaxKind as K, SyntaxNode, SyntaxToken, TextRange, TextSize};
use lsp_types::{
    Position, SemanticToken, SemanticTokenModifier as Modifier, SemanticTokenType as Type,
    SemanticTokens, SemanticTokensLegend,
};
use std::collections::HashMap;

// Indices and modifier bits follow legend() below.
const VARIABLE: u32 = 0;
const PARAMETER: u32 = 1;
const TYPE: u32 = 2;
const FUNCTION: u32 = 3;
const KEYWORD: u32 = 4;
const STRING: u32 = 5;
const NUMBER: u32 = 6;
const COMMENT: u32 = 7;
const OPERATOR: u32 = 8;
const PROPERTY: u32 = 9;
const DECLARATION: u32 = 1;
const READONLY: u32 = 2;
const DEFAULT_LIBRARY: u32 = 4;

pub(crate) fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            Type::VARIABLE,
            Type::PARAMETER,
            Type::TYPE,
            Type::FUNCTION,
            Type::KEYWORD,
            Type::STRING,
            Type::NUMBER,
            Type::COMMENT,
            Type::OPERATOR,
            Type::PROPERTY,
        ],
        token_modifiers: vec![
            Modifier::DECLARATION,
            Modifier::READONLY,
            Modifier::DEFAULT_LIBRARY,
        ],
    }
}

pub(crate) fn tokens(
    root: &SyntaxNode,
    model: &SemanticModel,
    positions: &Positions,
) -> SemanticTokens {
    // Never call definition_at/reference_at per leaf: those queries scan the model.
    let mut names = HashMap::new();
    for reference in model.references() {
        let classification = match reference.resolution {
            Resolution::NamedType(_) => Classification::new(TYPE, 0),
            Resolution::Field { .. } => Classification::new(PROPERTY, 0),
            Resolution::HostConstant(_) => {
                Classification::new(VARIABLE, READONLY | DEFAULT_LIBRARY)
            }
            Resolution::Value {
                ref definitions, ..
            } if definitions.len() == 1
                && model.definition(definitions[0]).kind == DefinitionKind::Param =>
            {
                Classification::new(PARAMETER, 0)
            }
            _ => Classification::new(VARIABLE, 0),
        };
        names.insert(reference.name_range, classification);
    }
    for definition in model.definitions() {
        let kind = match definition.kind {
            DefinitionKind::NamedType => TYPE,
            DefinitionKind::Param => PARAMETER,
            _ => VARIABLE,
        };
        names.insert(
            definition.name_range,
            Classification::new(kind, DECLARATION),
        );
    }

    let mut data = Vec::new();
    let mut previous = Position::default();
    for token in root
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
    {
        if let Some(classification) = names
            .get(&token.text_range())
            .copied()
            .or_else(|| classify(&token))
        {
            // Always split at physical line endings, even for clients supporting
            // multiline tokens. Raw escapes remain on their original source line.
            let mut start = usize::from(token.text_range().start());
            for part in token.text().split_inclusive(['\r', '\n']) {
                let content = part.trim_end_matches(['\r', '\n']);
                if !content.is_empty() {
                    let range = TextRange::new(
                        TextSize::new(start as u32),
                        TextSize::new((start + content.len()) as u32),
                    );
                    let range = positions.range(range);
                    data.push(SemanticToken {
                        delta_line: range.start.line - previous.line,
                        delta_start: if range.start.line == previous.line {
                            range.start.character - previous.character
                        } else {
                            range.start.character
                        },
                        length: range.end.character - range.start.character,
                        token_type: classification.kind,
                        token_modifiers_bitset: classification.modifiers,
                    });
                    previous = range.start;
                }
                start += part.len();
            }
        }
    }
    SemanticTokens {
        result_id: None,
        data,
    }
}

#[derive(Clone, Copy)]
struct Classification {
    kind: u32,
    modifiers: u32,
}

impl Classification {
    fn new(kind: u32, modifiers: u32) -> Self {
        Self { kind, modifiers }
    }
}

fn classify(token: &SyntaxToken) -> Option<Classification> {
    let parent = token.parent()?.kind();
    let kind = match token.kind() {
        K::Comment => COMMENT,
        K::String | K::Quote | K::PatternToken => STRING,
        K::Integer | K::Float => NUMBER,
        K::Ident => match parent {
            K::BaseType => return Some(Classification::new(TYPE, DEFAULT_LIBRARY)),
            K::Name | K::CurrentValue | K::Capture => VARIABLE,
            K::TypeName => TYPE,
            K::Field => PROPERTY,
            K::HostConstant => {
                return Some(Classification::new(VARIABLE, READONLY | DEFAULT_LIBRARY));
            }
            K::Call | K::Comprehension if token.text() != "in" => {
                return Some(Classification::new(FUNCTION, DEFAULT_LIBRARY));
            }
            K::Format if token.text() != "format" => STRING,
            _ => KEYWORD,
        },
        K::Punct
            if matches!(
                token.text(),
                "=" | "+"
                    | "-"
                    | "*"
                    | "/"
                    | "%"
                    | "^"
                    | "!"
                    | "&&"
                    | "||"
                    | "=="
                    | "!="
                    | "<"
                    | ">"
                    | "<="
                    | ">="
                    | ".."
                    | "..<"
            ) =>
        {
            OPERATOR
        }
        _ => return None,
    };
    Some(Classification::new(kind, 0))
}
