//! Checks formatter idempotence and syntax tree preservation at several widths.
#![no_main]

use iris_syntax::{
    FormatError, FormatOptions, LanguageVersion, SyntaxElement, SyntaxKind as K, SyntaxNode,
    format, parse,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|source: &[u8]| {
    for width in [40, 80, 100] {
        let options = FormatOptions { line_width: width };
        let output = match format(source, "fuzz.iris", LanguageVersion::V0_1, options) {
            Ok(output) => output,
            Err(FormatError::Source(_) | FormatError::Syntax(_)) => return,
            Err(error) => panic!("unexpected formatter error: {error}"),
        };
        let before = parse(source, "fuzz.iris", LanguageVersion::V0_1).unwrap();
        let after = parse(output.as_bytes(), "fuzz.iris", LanguageVersion::V0_1).unwrap();
        assert!(after.diagnostics().is_empty(), "{output:?}");
        assert_eq!(tree(&before.syntax()), tree(&after.syntax()));
        assert_eq!(
            format(
                output.as_bytes(),
                "fuzz.iris",
                LanguageVersion::V0_1,
                options
            )
            .unwrap(),
            output,
            "formatter is not idempotent at width {width}"
        );
    }
});

// Include node boundaries: an equal token stream alone does not prove same-tree.
// Keep comments too, so only whitespace may differ.
fn tree(node: &SyntaxNode) -> String {
    let mut result = format!("({:?}", node.kind());
    for element in node.children_with_tokens() {
        match element {
            SyntaxElement::Node(child) => result.push_str(&tree(&child)),
            SyntaxElement::Token(token) if token.kind() != K::Whitespace => {
                result.push_str(&format!(" {:?}:{:?}", token.kind(), token.text()));
            }
            _ => {}
        }
    }
    result.push(')');
    result
}
