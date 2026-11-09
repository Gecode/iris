//! Green tree construction with a node cache that never rehashes subtrees.
//!
//! rowan's `GreenNodeBuilder` shares equal small nodes through a hash table,
//! but it recomputes the hash of every cached node by walking its whole
//! subtree recursively whenever the table grows. Left operands can nest tens
//! of thousands of nodes deep (Specification §5.12 releases the entries of a
//! parenthesized left operand), so that walk takes quadratic time and
//! overflows the stack. This builder has the same interface, shares the same
//! tokens and nodes, and builds the same tree, but keys a cached node by the
//! identity of its children, which are cached themselves, so a key is hashed
//! in constant time.

use rowan::{GreenNode, GreenNodeData, GreenToken, GreenTokenData, NodeOrToken, SyntaxKind};
use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet, hash_map::Entry},
    hash::{BuildHasherDefault, Hash, Hasher},
    ptr,
};

/// rowan's limit: nodes with more children are never shared.
const MAX_SHARED_CHILDREN: usize = 3;

#[derive(Clone, Copy, Debug)]
pub(super) struct Checkpoint(usize);

/// A built tree and the number of node levels in it.
pub(crate) struct BuiltTree {
    pub(crate) root: GreenNode,
    pub(crate) depth: usize,
}

/// A finished child of an open node.
struct Child {
    element: NodeOrToken<GreenNode, GreenToken>,
    /// Whether the child is a token or a node from the node cache.
    shared: bool,
    /// The number of node levels in the child: 0 for a token.
    depth: usize,
}

#[derive(Default)]
pub(super) struct TreeBuilder {
    /// The shared tokens, indexed by kind.
    tokens: Vec<HashSet<TokenText, FastHash>>,
    nodes: HashMap<NodeKey, GreenNode, FastHash>,
    parents: Vec<(SyntaxKind, usize)>,
    children: Vec<Child>,
}

impl TreeBuilder {
    pub(super) fn token(&mut self, kind: SyntaxKind, text: &str) {
        let index = usize::from(kind.0);
        if self.tokens.len() <= index {
            self.tokens.resize_with(index + 1, HashSet::default);
        }
        let tokens = &mut self.tokens[index];
        let token = match tokens.get(text) {
            Some(token) => token.0.clone(),
            None => {
                let token = GreenToken::new(kind, text);
                tokens.insert(TokenText(token.clone()));
                token
            }
        };
        self.children.push(Child {
            element: NodeOrToken::Token(token),
            shared: true,
            depth: 0,
        });
    }
    pub(super) fn start_node(&mut self, kind: SyntaxKind) {
        self.parents.push((kind, self.children.len()));
    }
    pub(super) fn finish_node(&mut self) {
        let (kind, first_child) = self.parents.pop().expect("an open node");
        let children = &self.children[first_child..];
        let depth = 1 + children.iter().map(|c| c.depth).max().unwrap_or(0);
        let shared = children.len() <= MAX_SHARED_CHILDREN && children.iter().all(|c| c.shared);
        let node = if shared {
            match self.nodes.entry(NodeKey::new(kind, children)) {
                Entry::Occupied(entry) => {
                    self.children.truncate(first_child);
                    entry.get().clone()
                }
                Entry::Vacant(entry) => {
                    let node = build(kind, self.children.drain(first_child..));
                    entry.insert(node.clone());
                    node
                }
            }
        } else {
            build(kind, self.children.drain(first_child..))
        };
        self.children.push(Child {
            element: NodeOrToken::Node(node),
            shared,
            depth,
        });
    }
    pub(super) fn checkpoint(&self) -> Checkpoint {
        Checkpoint(self.children.len())
    }
    pub(super) fn start_node_at(&mut self, checkpoint: Checkpoint, kind: SyntaxKind) {
        assert!(checkpoint.0 <= self.children.len(), "stale checkpoint");
        if let Some(&(_, first_child)) = self.parents.last() {
            assert!(
                checkpoint.0 >= first_child,
                "checkpoint before the open node"
            );
        }
        self.parents.push((kind, checkpoint.0));
    }
    pub(super) fn finish(mut self) -> BuiltTree {
        assert_eq!(self.children.len(), 1, "one root");
        match self.children.pop() {
            Some(Child {
                element: NodeOrToken::Node(root),
                depth,
                ..
            }) => BuiltTree { root, depth },
            _ => panic!("the root is a node"),
        }
    }
}

fn build(kind: SyntaxKind, children: impl ExactSizeIterator<Item = Child>) -> GreenNode {
    GreenNode::new(kind, children.map(|child| child.element))
}

/// A shared token, found by its text among the tokens of one kind.
struct TokenText(GreenToken);

impl Borrow<str> for TokenText {
    fn borrow(&self) -> &str {
        self.0.text()
    }
}
impl Hash for TokenText {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.text().hash(state);
    }
}
impl PartialEq for TokenText {
    fn eq(&self, other: &Self) -> bool {
        self.0.text() == other.0.text()
    }
}
impl Eq for TokenText {}

/// A shared node's kind and the addresses of its children. Children are
/// shared too, and the cache keeps them alive, so equal addresses mean equal
/// subtrees for as long as the builder lives.
#[derive(PartialEq, Eq, Hash)]
struct NodeKey {
    kind: SyntaxKind,
    children: [usize; MAX_SHARED_CHILDREN],
    len: usize,
}

impl NodeKey {
    fn new(kind: SyntaxKind, children: &[Child]) -> Self {
        let mut addresses = [0; MAX_SHARED_CHILDREN];
        for (address, child) in addresses.iter_mut().zip(children) {
            *address = match &child.element {
                NodeOrToken::Node(node) => ptr::from_ref::<GreenNodeData>(node).addr(),
                NodeOrToken::Token(token) => ptr::from_ref::<GreenTokenData>(token).addr(),
            };
        }
        Self {
            kind,
            children: addresses,
            len: children.len(),
        }
    }
}

type FastHash = BuildHasherDefault<WordHasher>;

/// The multiply-rotate hash rowan's cache uses (FxHash). The keys are token
/// texts of the reader spec and node addresses, the same keys rowan hashes.
#[derive(Default)]
struct WordHasher(u64);

impl WordHasher {
    const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(Self::SEED);
    }
}

impl Hasher for WordHasher {
    fn write(&mut self, bytes: &[u8]) {
        let (words, rest) = bytes.as_chunks::<8>();
        for word in words {
            self.add(u64::from_le_bytes(*word));
        }
        let mut last = [0; 8];
        last[..rest.len()].copy_from_slice(rest);
        self.add(u64::from_le_bytes(last));
    }
    fn write_u8(&mut self, value: u8) {
        self.add(value.into());
    }
    fn write_u16(&mut self, value: u16) {
        self.add(value.into());
    }
    fn write_u32(&mut self, value: u32) {
        self.add(value.into());
    }
    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }
    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::TreeBuilder;
    use rowan::{GreenNodeBuilder, NodeOrToken, SyntaxKind};

    const ROOT: SyntaxKind = SyntaxKind(1);
    const BINARY: SyntaxKind = SyntaxKind(2);
    const PAREN: SyntaxKind = SyntaxKind(3);
    const NUMBER: SyntaxKind = SyntaxKind(4);
    const PLUS: SyntaxKind = SyntaxKind(5);

    /// `(1+1+…+1)` with `operands` operands, left-nested through checkpoints
    /// as the parser builds it, then `1+1` once more.
    macro_rules! build {
        ($builder:expr, $operands:expr) => {{
            let mut builder = $builder;
            builder.start_node(ROOT);
            builder.start_node(PAREN);
            let checkpoint = builder.checkpoint();
            builder.token(NUMBER, "1");
            for _ in 1..$operands {
                builder.start_node_at(checkpoint, BINARY);
                builder.token(PLUS, "+");
                builder.token(NUMBER, "1");
                builder.finish_node();
            }
            builder.finish_node();
            builder.start_node(BINARY);
            builder.token(NUMBER, "1");
            builder.token(PLUS, "+");
            builder.token(NUMBER, "1");
            builder.finish_node();
            builder.finish_node();
            builder.finish()
        }};
    }

    #[test]
    fn builds_the_tree_rowan_builds_and_counts_left_nested_levels() {
        for operands in [1, 2, 3, 50] {
            let built = build!(TreeBuilder::default(), operands);
            let expected = build!(GreenNodeBuilder::new(), operands);
            assert_eq!(built.root, expected);
            // The root, the parenthesis, and one level per operator.
            assert_eq!(built.depth, 1 + operands);
        }
    }

    #[test]
    fn shares_equal_small_nodes() {
        let built = build!(TreeBuilder::default(), 2);
        let nodes: Vec<_> = built
            .root
            .children()
            .filter_map(NodeOrToken::into_node)
            .collect();
        let inner = nodes[0].children().next().and_then(NodeOrToken::into_node);
        assert!(std::ptr::eq(inner.unwrap(), nodes[1]));
    }
}
