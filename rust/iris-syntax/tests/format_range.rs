mod support;
use iris_syntax::{
    FormatError, FormatOptions, LanguageVersion, RangeFormat, SyntaxKind, TextRange, TextSize,
    format, format_range, parse,
};

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::try_from(start).unwrap(),
        TextSize::try_from(end).unwrap(),
    )
}

fn apply(source: &str, result: &RangeFormat) -> String {
    let mut output = source.to_owned();
    for edit in result.edits.iter().rev() {
        assert!(result.range.contains_range(edit.range));
        output.replace_range(
            std::ops::Range::<usize>::from(edit.range),
            &edit.replacement,
        );
    }
    let start = usize::from(result.range.start());
    let end = usize::from(result.range.end());
    assert_eq!(&output[..start], &source[..start]);
    assert!(output.ends_with(&source[end..]));
    output
}

fn checked(source: &str, selected: &str) -> (RangeFormat, String) {
    let start = source.find(selected).unwrap();
    let request = range(start, start + selected.len());
    let result = format_range(
        source.as_bytes(),
        "range",
        LanguageVersion::V0_1,
        request,
        FormatOptions::default(),
    )
    .unwrap();
    assert!(result.range.contains_range(request));
    let output = apply(source, &result);
    let before = parse(source.as_bytes(), "before", LanguageVersion::V0_1).unwrap();
    let after = parse(output.as_bytes(), "after", LanguageVersion::V0_1).unwrap();
    if before.diagnostics().is_empty() {
        assert!(
            after.diagnostics().is_empty(),
            "{output}: {:?}",
            after.diagnostics()
        );
        assert_eq!(
            support::tree(&before.syntax()),
            support::tree(&after.syntax())
        );
        assert_eq!(
            support::protected(&before.syntax()),
            support::protected(&after.syntax())
        );
    }
    let again = format_range(
        output.as_bytes(),
        "again",
        LanguageVersion::V0_1,
        range(
            usize::from(result.range.start()),
            usize::from(result.range.start())
                + result
                    .edits
                    .first()
                    .map_or(usize::from(result.range.len()), |edit| {
                        edit.replacement.len()
                    }),
        ),
        FormatOptions::default(),
    )
    .unwrap();
    assert!(
        again.edits.is_empty(),
        "range idempotence:\n{output}\n{again:?}"
    );
    (result, output)
}

#[test]
fn range_inside_block() {
    let (result, output) = checked("repeat 2 {\n x :int\n y:word\n}\n", "x");
    assert_eq!(output, "repeat 2 {\n  x: int\n  y:word\n}\n");
    assert_eq!(result.range, range(10, 20));
}

#[test]
fn range_tokens_do_not_merge() {
    let (result, output) = checked("x:1. ..2", "x:1. ..2");
    assert_eq!(result.range, range(0, 8));
    assert_eq!(output, "x: 1. ..2\n");
}

#[test]
fn range_spanning_arms() {
    let source = "before:int\nsections {\"A\" {a:int} \"B\" {b:int}}\nafter:word\n";
    let (result, output) = checked(source, "a:int} \"B\"");
    assert_eq!(result.range.start(), TextSize::from(10));
    assert_eq!(
        output,
        "before:int\nsections {\n  \"A\" {\n    a: int\n  }\n  \"B\" {\n    b: int\n  }\n}\nafter:word\n"
    );
}

#[test]
fn range_clean_statement_next_to_malformed() {
    for source in [
        "good :int\nlet bad=+\n",
        "let bad=+ good :int\n",
        "repeat 1 {let bad=+ good :int}\n",
        "repeat 1 {good :int let bad=+}\n",
        "@\ngood :int\n",
        "expect\ngood :int\n",
    ] {
        let (_, output) = checked(source, "good");
        assert!(output.contains("good: int"), "{output}");
        assert!(output.contains(if source.contains("let bad=+") {
            "let bad=+"
        } else if source.contains('@') {
            "@"
        } else {
            "expect"
        }));
    }
}

#[test]
fn range_context_and_trivia() {
    for (source, selected, expected) in [
        (
            "sections {\"A\" {x :int y:word}}",
            "x",
            "sections {\"A\" {\n    x: int\n    y:word}}",
        ),
        (
            "if 0 {} else if [1] :int\nafter:int",
            "[1]",
            "if 0 {} else if[1]: int\nafter:int",
        ),
        (
            "first:int\nx :int # trailing  \n\nlast:word",
            "x",
            "first:int\nx: int # trailing  \n\nlast:word",
        ),
        (
            "first:int\n# standalone\nx :int\nlast:word",
            "# standalone\nx",
            "first:int\n# standalone\nx: int\nlast:word",
        ),
        (
            "a :int b :word\nc:int",
            "a :int b",
            "a: int\nb: word\nc:int",
        ),
    ] {
        let (_, output) = checked(source, selected);
        assert_eq!(output, expected);
    }
}

#[test]
fn range_errors_and_cursor_boundaries() {
    let options = FormatOptions::default();
    for (source, selected) in [
        ("let x=+ good:int", "x"),
        ("repeat 1 {", "repeat"),
        ("line \"{x\" good:int", "line"),
        ("expect\ngood:int", "expect"),
    ] {
        let start = source.find(selected).unwrap();
        assert!(
            matches!(
                format_range(
                    source.as_bytes(),
                    "bad",
                    LanguageVersion::V0_1,
                    range(start, start + selected.len()),
                    options
                ),
                Err(FormatError::Syntax(_))
            ),
            "{source}"
        );
    }
    let source = "let x=\"é\"";
    let inside = source.find('é').unwrap() + 1;
    assert_eq!(
        format_range(
            source.as_bytes(),
            "utf8",
            LanguageVersion::V0_1,
            range(inside, inside),
            options
        ),
        Err(FormatError::InvalidRange)
    );
    assert_eq!(
        format_range(
            b"x:int",
            "bounds",
            LanguageVersion::V0_1,
            range(0, 6),
            options
        ),
        Err(FormatError::InvalidRange)
    );
    assert!(
        format_range(b" \n", "empty", LanguageVersion::V0_1, range(1, 1), options)
            .unwrap()
            .edits
            .is_empty()
    );
    for (source, cursor) in [("x :int", 0), ("x :int", 6), ("a:int\nb :word", 6)] {
        let result = format_range(
            source.as_bytes(),
            "cursor",
            LanguageVersion::V0_1,
            range(cursor, cursor),
            options,
        )
        .unwrap();
        assert!(
            apply(source, &result).contains(if cursor == 6 && source.starts_with('a') {
                "b: word"
            } else {
                "x: int"
            })
        );
    }
}

#[test]
fn range_trailing_comment_ownership_and_braces() {
    for (source, selected, expected) in [
        (
            "a :int # attached\nb:word",
            "attached",
            "a: int # attached\nb:word",
        ),
        (
            "a:int   # attached\nb:word",
            "   ",
            "a: int # attached\nb:word",
        ),
        ("repeat 1 {x :int }", "x", "repeat 1 {\n  x: int\n}"),
        (
            "first:int\n\n\nx :int\nlast:int",
            "\n\nx",
            "first:int\n\nx: int\nlast:int",
        ),
        ("\n# only  \n\n", "only", "# only  \n"),
    ] {
        let (_, output) = checked(source, selected);
        assert_eq!(output, expected);
    }
    let (_, output) = checked("repeat 1 {\n# standalone\nx :int\n}", "# standalone");
    assert_eq!(output, "repeat 1 {\n  # standalone\n  x: int\n}");
    for token in ["{", "}"] {
        let source = "repeat 1 {x :int}\nafter:word";
        let (_, output) = checked(source, token);
        assert_eq!(output, "repeat 1 {\n  x: int\n}\nafter:word");
        let cursor = source.find(token).unwrap();
        let result = format_range(
            source.as_bytes(),
            "brace",
            LanguageVersion::V0_1,
            range(cursor, cursor),
            FormatOptions::default(),
        )
        .unwrap();
        assert_eq!(apply(source, &result), output);
    }
}

#[test]
fn range_normalizes_complete_boundary_whitespace() {
    for (source, selected, expected) in [
        (
            "first:int\n\n\nx :int\nlast:int",
            "\n\nx",
            "first:int\n\nx: int\nlast:int",
        ),
        (
            "repeat 1 {\n\n  x :int\n\n}",
            "x",
            "repeat 1 {\n  x: int\n}",
        ),
        (
            "first:int  \r\n \t x :int \r\n \t last:int",
            "x",
            "first:int\nx: int\nlast:int",
        ),
    ] {
        let (_, output) = checked(source, selected);
        assert_eq!(output, expected);
    }
}

#[test]
fn whole_statement_ranges_match_document_formatting() {
    let mut checks = 0;
    for source in [
        include_str!("fixtures/format/rules.iris"),
        "\n\nfirst:int \r\n\n\n x :int \t\n\nlast:int \n\n",
        "repeat 1 {\n\n x :int \n\n\n y :word\n\n}\n",
        "# leading\n\nfirst:int # trailing  \r\n\n\n# standalone\nx :int\n\n",
    ] {
        let parsed = parse(source.as_bytes(), "ranges", LanguageVersion::V0_1).unwrap();
        let tokens: Vec<_> = parsed
            .syntax()
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() != SyntaxKind::Whitespace)
            .collect();
        for width in [40, 80, 100] {
            let options = FormatOptions { line_width: width };
            let whole = format(source.as_bytes(), "whole", LanguageVersion::V0_1, options).unwrap();
            let formatted = parse(whole.as_bytes(), "whole", LanguageVersion::V0_1).unwrap();
            let formatted_tokens: Vec<_> = formatted
                .syntax()
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .filter(|t| t.kind() != SyntaxKind::Whitespace)
                .collect();
            let statements: Vec<_> = parsed.syntax().children().collect();
            let mut requests = vec![range(0, source.len())];
            for (i, first) in statements.iter().enumerate() {
                for last in &statements[i..] {
                    requests.push(TextRange::new(
                        first.text_range().start(),
                        last.text_range().end(),
                    ));
                }
            }
            // Body statements exercise both brace edges without selecting braces.
            for block in parsed
                .syntax()
                .descendants()
                .filter(|n| n.kind() == SyntaxKind::Block)
            {
                requests.extend(block.children().map(|n| n.text_range()));
            }
            for request in requests {
                let result = format_range(
                    source.as_bytes(),
                    "range",
                    LanguageVersion::V0_1,
                    request,
                    options,
                )
                .unwrap();
                let output = apply(source, &result);
                let start = tokens
                    .iter()
                    .rposition(|t| t.text_range().end() <= result.range.start())
                    .map_or(0, |i| usize::from(formatted_tokens[i].text_range().end()));
                let end = tokens
                    .iter()
                    .position(|t| t.text_range().start() >= result.range.end())
                    .map_or(whole.len(), |i| {
                        usize::from(formatted_tokens[i].text_range().start())
                    });
                let replacement = result.edits.first().map_or(
                    &source[std::ops::Range::<usize>::from(result.range)],
                    |edit| edit.replacement.as_str(),
                );
                assert_eq!(
                    replacement,
                    &whole[start..end],
                    "width {width}, request {request:?}, extent {:?}\n{source}\n{output}",
                    result.range
                );
                checks += 1;
            }
        }
    }
    println!("whole-statement range exactness checks={checks}");
}

#[test]
fn partial_boundary_whitespace_widens_to_the_complete_run() {
    let source = "first:int\n\n\nx :int\nlast:int";
    let result = format_range(
        source.as_bytes(),
        "boundary",
        LanguageVersion::V0_1,
        range(10, 13),
        FormatOptions::default(),
    )
    .unwrap();
    assert_eq!(result.range, range(9, 19));
    assert_eq!(apply(source, &result), "first:int\n\nx: int\nlast:int");
}
