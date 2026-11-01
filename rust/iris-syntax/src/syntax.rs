//! Concrete syntax kinds. Keywords remain contextual identifier tokens.

macro_rules! syntax_kinds {
    ($($kind:ident),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u16)]
        pub enum SyntaxKind { $($kind),+ }

        impl From<SyntaxKind> for rowan::SyntaxKind {
            fn from(kind: SyntaxKind) -> Self { Self(kind as u16) }
        }

        impl IrisLanguage {
            fn decode(raw: rowan::SyntaxKind) -> SyntaxKind {
                match raw.0 {
                    $(value if value == SyntaxKind::$kind as u16 => SyntaxKind::$kind,)+
                    _ => panic!("syntax kind was not created by iris-syntax"),
                }
            }
        }
    };
}

syntax_kinds! {
    Whitespace, Comment, Ident, Integer, Float, String, Punct, Invalid,
    Root, Error, Format, Setting, Declaration, Dimension, Type, BaseType,
    TypeName, Range, Alphabet, AlphabetOption, Layout, Offset, List, TypeDefinition,
    Expect, Param, Line, Find, Skip, Index, Repeat, Each, Instances, Sections, Arm,
    If, Let, Check, Block, Terminators, Pattern, PatternText, PatternBlank,
    Capture, CurrentValue, Ellipsis, Quote, PatternToken,
    Literal, Name, HostConstant, Parenthesized, Unary, Binary, Power,
    Call, Comprehension, IndexExpression, Field,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IrisLanguage {}

impl rowan::Language for IrisLanguage {
    type Kind = SyntaxKind;
    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        Self::decode(raw)
    }
    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<IrisLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<IrisLanguage>;
pub type SyntaxElement = rowan::SyntaxElement<IrisLanguage>;
