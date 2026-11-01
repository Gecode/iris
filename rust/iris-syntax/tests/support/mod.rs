use iris_syntax::{FormatOptions, LanguageVersion, SyntaxKind as K, SyntaxNode, format, parse};

/// Node boundaries matter: equal token streams can still have different trees.
pub fn tree(node: &SyntaxNode) -> String {
    let mut result = format!("({:?}", node.kind());
    for element in node.children_with_tokens() {
        match element {
            iris_syntax::SyntaxElement::Node(child) => result.push_str(&tree(&child)),
            iris_syntax::SyntaxElement::Token(token) if token.kind() != K::Whitespace => {
                result.push_str(&format!(" {:?}:{:?}", token.kind(), token.text()));
            }
            _ => {}
        }
    }
    result.push(')');
    result
}

pub fn protected(node: &SyntaxNode) -> Vec<String> {
    let mut result = Vec::new();
    for element in node.children_with_tokens() {
        match element {
            iris_syntax::SyntaxElement::Node(child) if child.kind() == K::Pattern => {
                result.push(child.text().to_string())
            }
            iris_syntax::SyntaxElement::Node(child) => result.extend(protected(&child)),
            iris_syntax::SyntaxElement::Token(token)
                if matches!(token.kind(), K::String | K::Comment) =>
            {
                result.push(token.text().to_owned())
            }
            _ => {}
        }
    }
    result
}

// Integration test binaries exercise different entry points of the shared checks.
#[allow(dead_code)]
pub fn checked(source: &str, width: usize) -> String {
    let options = FormatOptions { line_width: width };
    let before = parse(source.as_bytes(), "before", LanguageVersion::V0_1).unwrap();
    assert!(
        before.diagnostics().is_empty(),
        "{source:?}: {:?}",
        before.diagnostics()
    );
    let output = format(source.as_bytes(), "before", LanguageVersion::V0_1, options).unwrap();
    let after = parse(output.as_bytes(), "after", LanguageVersion::V0_1).unwrap();
    assert!(
        after.diagnostics().is_empty(),
        "{source:?} -> {output:?}: {:?}",
        after.diagnostics()
    );
    assert_eq!(
        tree(&before.syntax()),
        tree(&after.syntax()),
        "{source:?} -> {output:?}"
    );
    assert_eq!(protected(&before.syntax()), protected(&after.syntax()));
    assert_eq!(
        format(output.as_bytes(), "after", LanguageVersion::V0_1, options).unwrap(),
        output,
        "idempotence at {width}: {source:?}"
    );
    output
}
