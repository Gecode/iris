//! Lossless syntax analysis and whitespace-only formatting for Iris 0.1.
//!
//! Syntax acceptance does not include name resolution or type checking.
mod checks;
mod diagnostic;
mod format;
mod lexer;
mod parser;
mod syntax;

use std::{fmt, sync::Arc};

pub use checks::{ParseCheck, ParseCheckKind};
pub use diagnostic::{Diagnostic, DiagnosticCode, DiagnosticPhase, SourceLocation};
pub use format::{FormatEdit, FormatError, FormatOptions, RangeFormat, format, format_range};
pub use rowan::{TextRange, TextSize, WalkEvent};
pub use syntax::{IrisLanguage, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageVersion {
    V0_1,
}

/// A parsed reader spec: its concrete syntax tree, diagnostics and checks.
#[derive(Debug, Clone)]
pub struct Parse {
    tree: SyntaxTree,
    diagnostics: Vec<Diagnostic>,
    checks: Vec<ParseCheck>,
    version: LanguageVersion,
    origin: Arc<str>,
}

/// A concrete syntax tree, shared by its [`Parse`] and by whatever keeps
/// syntax nodes from it, such as a semantic model.
///
/// Left operands can nest tens of thousands of nodes deep (Specification
/// §5.12), and rowan frees a tree recursively once its last owner is gone.
/// The last clone of a deep `SyntaxTree` frees the tree without recursion,
/// so a value that keeps syntax nodes keeps a clone and drops it after them.
#[derive(Clone)]
pub struct SyntaxTree(Arc<GreenTree>);

struct GreenTree {
    root: rowan::GreenNode,
    depth: usize,
}

/// The deepest tree that rowan frees by recursion. A level takes about 430
/// bytes of stack in a debug build and 85 in a release build, so this needs
/// at most about 110 KiB. Ordinary specs are much shallower and cost nothing
/// more to free.
const RECURSIVE_FREE_DEPTH: usize = 256;

impl SyntaxTree {
    /// The root node. Keep the tree for as long as the node or any node
    /// reached from it.
    pub fn root(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.0.root.clone())
    }
}

impl fmt::Debug for SyntaxTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxTree")
            .field("root", &self.0.root)
            .finish()
    }
}

impl Drop for GreenTree {
    fn drop(&mut self) {
        if self.depth > RECURSIVE_FREE_DEPTH {
            let placeholder = rowan::GreenNode::new(SyntaxKind::Root.into(), []);
            free_without_recursion(std::mem::replace(&mut self.root, placeholder));
        }
    }
}

/// Frees `root` with constant stack depth if no other owner keeps it alive.
///
/// Every node is first held once more for each place it has in the tree,
/// parents before children. Releasing them in that order frees each node
/// while its children are still held, so no node frees another.
fn free_without_recursion(root: rowan::GreenNode) {
    let mut nodes = vec![root];
    let mut next = 0;
    while let Some(node) = nodes.get(next).cloned() {
        nodes.extend(
            node.children()
                .filter_map(|child| child.into_node().map(ToOwned::to_owned)),
        );
        next += 1;
    }
    for node in nodes {
        drop(node);
    }
}

impl Parse {
    /// The root node. Keep the `Parse`, or its [`tree`](Self::tree), for as
    /// long as the node or any node reached from it.
    pub fn syntax(&self) -> SyntaxNode {
        self.tree.root()
    }
    pub fn tree(&self) -> &SyntaxTree {
        &self.tree
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn checks(&self) -> &[ParseCheck] {
        &self.checks
    }
    pub fn version(&self) -> LanguageVersion {
        self.version
    }
    /// The last declared format name, decoded from the reader spec.
    pub fn format_name(&self) -> Option<String> {
        use SyntaxKind as K;
        self.syntax()
            .children()
            .filter(|n| n.kind() == K::Format)
            .filter_map(|node| {
                let tokens: Vec<_> = node
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .filter(|t| !matches!(t.kind(), K::Whitespace | K::Comment))
                    .skip(1)
                    .collect();
                let first = tokens.first()?;
                if first.kind() == K::String {
                    Some(DecodedString::new(first.text()).text)
                } else {
                    Some(tokens.iter().map(|t| t.text()).collect())
                }
            })
            .last()
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }
}

/// Failures that prevent construction of a CST. SourceTooLarge is a host
/// resource limit imposed by rowan's u32 lengths, not a language diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    InvalidUtf8(Diagnostic),
    SourceTooLarge { len: usize },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8(diagnostic) => diagnostic.fmt(f),
            Self::SourceTooLarge { len } => {
                write!(f, "source has {len} bytes, exceeding rowan's u32 capacity")
            }
        }
    }
}
impl std::error::Error for ParseError {}

/// Validate bytes, lex the complete source, then parse statements and patterns.
///
/// Every valid UTF-8 source of at most u32::MAX bytes yields a CST whose text
/// equals the source, even when syntax diagnostics are present. Longer sources
/// return a host resource failure before UTF-8 validation. Invalid UTF-8 yields
/// one source diagnostic and no tree; bytes are never replaced or normalized.
pub fn parse(source: &[u8], origin: &str, version: LanguageVersion) -> Result<Parse, ParseError> {
    check_source_len(source.len())?;
    let origin: Arc<str> = origin.into();
    let source = std::str::from_utf8(source).map_err(|error| {
        let offset = error.valid_up_to();
        let prefix = &source[..offset];
        let line = prefix.iter().filter(|&&byte| byte == b'\n').count() + 1;
        let column = prefix
            .iter()
            .rposition(|&byte| byte == b'\n')
            .map_or(offset + 1, |lf| offset - lf);
        ParseError::InvalidUtf8(Diagnostic {
            code: DiagnosticCode::Utf8,
            message: "reader spec is not valid UTF-8".into(),
            range: diagnostic::text_range(
                offset,
                offset + error.error_len().unwrap_or(source.len() - offset),
            ),
            location: SourceLocation {
                origin: origin.clone(),
                line,
                column: Some(column),
            },
            phase: DiagnosticPhase::Source,
        })
    })?;
    let lexed = lexer::lex(source, origin.clone());
    let (tree, diagnostics, checks) = parser::parse(source, origin.clone(), lexed);
    Ok(Parse {
        tree: SyntaxTree(Arc::new(GreenTree {
            root: tree.root,
            depth: tree.depth,
        })),
        diagnostics,
        checks,
        version,
        origin,
    })
}

fn check_source_len(len: usize) -> Result<(), ParseError> {
    if u32::try_from(len).is_err() {
        Err(ParseError::SourceTooLarge { len })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_size_is_a_host_limit() {
        assert!(super::check_source_len(u32::MAX as usize).is_ok());
        if let Some(len) = (u32::MAX as usize).checked_add(1) {
            assert_eq!(
                super::check_source_len(len),
                Err(super::ParseError::SourceTooLarge { len })
            );
        }
    }
}

/// A valid source string decoded once, with raw byte boundaries for tooling.
#[derive(Debug, Clone)]
pub struct DecodedString {
    pub text: String,
    pub raw_boundaries: Vec<usize>,
}
impl DecodedString {
    /// `text` must be a complete, lexically valid quoted Iris string token.
    pub fn new(text: &str) -> Self {
        let (text, raw_boundaries) = lexer::decode_string(text);
        Self {
            text,
            raw_boundaries,
        }
    }
}
