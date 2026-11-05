mod support;

use gecode_iris_syntax::{FormatError, FormatOptions, LanguageVersion, format};
use support::checked;

#[test]
fn guide_examples() {
    let guide = include_str!("../../../docs/formatting.md");
    let mut blocks = Vec::new();
    let mut width = 100;
    let mut lines = guide.lines();
    while let Some(line) = lines.next() {
        if line.starts_with("##") {
            width = if line.contains("width 40") { 40 } else { 100 };
        }
        if line == "```iris" {
            let mut block = String::new();
            for line in lines.by_ref() {
                if line == "```" {
                    break;
                }
                block.push_str(line);
                block.push('\n');
            }
            blocks.push((width, block));
        }
    }
    assert_eq!(blocks.len(), 44);
    let mut failures = Vec::new();
    for (index, pair) in blocks.as_chunks::<2>().0.iter().enumerate() {
        let (width, before) = &pair[0];
        let after = &pair[1].1;
        let output = checked(before, *width);
        if &output != after {
            failures.push(format!(
                "pair {} width {width}\nEXPECTED:\n{after}ACTUAL:\n{output}",
                index + 1
            ));
        }
        let formatted_after = checked(after, *width);
        if formatted_after != *after {
            failures.push(format!("after {}: {formatted_after}", index + 1));
        }
    }
    println!("guide pairs checked: {}", blocks.len() / 2);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn source_failures_return_no_output() {
    let options = FormatOptions::default();
    assert!(matches!(
        format(b"\xff", "bad", LanguageVersion::V0_1, options),
        Err(FormatError::Source(_))
    ));
    for source in [
        "@",
        "let x=+ after:int",
        "line \"{x\"",
        "repeat 1 {",
        "x:",
        "let x=\"bad\\q\"",
    ] {
        assert!(
            matches!(
                format(source.as_bytes(), "bad", LanguageVersion::V0_1, options),
                Err(FormatError::Syntax(_))
            ),
            "{source}"
        );
    }
    assert_eq!(
        format(
            b"",
            "empty",
            LanguageVersion::V0_1,
            FormatOptions { line_width: 0 }
        ),
        Err(FormatError::InvalidWidth)
    );
}

#[test]
fn source_trivia_and_token_boundaries() {
    for width in [40, 80, 100] {
        for (before, after) in [
            ("", ""),
            (" \t\r\n\n", ""),
            ("\n# comment  \r\n\n\n", "# comment  \r\n"),
            ("repeat 0 {\n\n}\n", "repeat 0 {}\n"),
            (
                "repeat 1 {\n\n x:int\n\n y:int\n\n}\n",
                "repeat 1 {\n  x: int\n\n  y: int\n}\n",
            ),
            (
                "sections {\n\n\"A\" {\n\n x:int\n\n} \n\n\"B\"\n\n}",
                "sections {\n  \"A\" {\n    x: int\n  }\n\n  \"B\"\n}\n",
            ),
            ("let x=1\n\n+2", "let x = 1\n\n  + 2\n"),
            (
                "let x=1 . value let y=gecode . value let z=1. . value",
                "let x = 1 . value\nlet y = gecode . value\nlet z = 1. . value\n",
            ),
            ("line \"{x: 0 .. 9 }\"", "line \"{x: 0 .. 9 }\"\n"),
            (
                "let x=1 # same token  \r\n+2",
                "let x = 1 # same token  \r\n  + 2\n",
            ),
        ] {
            assert_eq!(checked(before, width), after, "width {width}");
        }
    }
}

#[test]
fn width_boundaries_and_atomic_overflow() {
    // Each scalar counts once, including combining marks, rather than counting
    // UTF-8 bytes or terminal cells. The default line fits at precisely 40.
    let source = format!("let label=\"{}\"", "é".repeat(26));
    assert_eq!(
        checked(&source, 40),
        format!("let label = \"{}\"\n", "é".repeat(26))
    );
    assert_eq!(
        checked(&source, 39),
        format!("let label =\n  \"{}\"\n", "é".repeat(26))
    );
    assert_eq!(checked("let x=\"éé\"", 15), "let x = \"éé\"\n");
    assert_eq!(checked("let x=\"éé\"", 13), "let x =\n  \"éé\"\n");
    for width in [1, 2, 5, 40, 80, 100] {
        assert_eq!(
            checked("let long_unbreakable_name=short", width),
            if width >= 33 {
                "let long_unbreakable_name = short\n"
            } else if width >= 7 {
                "let long_unbreakable_name =\n  short\n"
            } else {
                "let long_unbreakable_name = short\n"
            }
        );
        assert_eq!(
            checked("let x=values[0].field", width),
            if (18..=22).contains(&width) {
                "let x =\n  values[0].field\n"
            } else {
                "let x = values[0].field\n"
            }
        );
    }
    assert_eq!(
        checked("repeat first_value {}", 21),
        "repeat first_value {}\n"
    );
    assert_eq!(
        checked("repeat first_value {}", 19),
        "repeat\n  first_value {}\n"
    );
    assert_eq!(
        checked(
            "check first_value>0 \"message\" # comment is allowed to overflow the line",
            40
        ),
        "check first_value > 0 \"message\" # comment is allowed to overflow the line\n"
    );
}

#[test]
fn range_tokens_do_not_merge() {
    assert_eq!(checked("x:1. ..2", 100), "x: 1. ..2\n");
    assert_eq!(checked("x:1. ..<2", 100), "x: 1. ..<2\n");
}

#[test]
fn empty_block_width_includes_closing_brace() {
    for (source, flat, wrapped) in [
        (
            "repeat first_value {}",
            "repeat first_value {}\n",
            "repeat\n  first_value {}\n",
        ),
        (
            "if first_value {}",
            "if first_value {}\n",
            "if\n  first_value {}\n",
        ),
    ] {
        let exact = flat.trim_end().chars().count();
        for width in [exact - 1, exact, exact + 1] {
            assert_eq!(
                checked(source, width),
                if width < exact { wrapped } else { flat }
            );
        }
    }
}

#[test]
fn capture_options_do_not_wrap_outer_headers() {
    for capture in [
        r#"{:alphabet \"a\" base 0}"#,
        r#"{x:alphabet \"ab\" blank \".\" strip \"|\" base -2}"#,
    ] {
        let arm = format!("\"{capture}\" optional");
        assert_eq!(
            checked(&format!("sections {{{arm}}}"), 20),
            format!("sections {{\n  {arm}\n}}\n")
        );
        for keyword in ["line", "find"] {
            let statement = format!("{keyword} \"{capture}\"");
            assert_eq!(checked(&statement, 10), format!("{statement}\n"));
        }
    }
}

#[test]
fn trailing_comments_stay_before_wrapped_options() {
    assert_eq!(
        checked(
            "x:alphabet \"ab\" # keep\nbase 0 blank \".\" strip \"|\"",
            20
        ),
        "x: alphabet \"ab\" # keep\n  base 0\n  blank \".\"\n  strip \"|\"\n"
    );
    assert_eq!(
        checked("x:int # keep\noffset 123456", 20),
        "x: int # keep\n  offset 123456\n"
    );
}

#[test]
fn hard_trivia_at_punctuation_and_joins() {
    for width in [40, 80, 100] {
        for (source, expected) in [
            (
                "repeat 1 # header\n{x:int}",
                "repeat 1 # header\n  {\n  x: int\n}\n",
            ),
            (
                "if 0 {} else # next header\nif 1 {x:int}",
                "if 0 {} else # next header\n  if 1 {\n  x: int\n}\n",
            ),
            (
                "let x=sum( # opener\n1, # comma\n2 # closer\n)",
                "let x = sum( # opener\n  1, # comma\n  2 # closer\n  )\n",
            ),
            ("if 0 {} # join\nelse {}", "if 0 {} # join\n  else {}\n"),
            ("if 0 {}\n\nelse {}", "if 0 {}\n\n  else {}\n"),
            (
                "let x=gecode # left\n. # right\nvalue",
                "let x = gecode # left\n  . # right\n  value\n",
            ),
            (
                "repeat 1 {\n\n# body\n\nx:int\n\n}",
                "repeat 1 {\n  # body\n\n  x: int\n}\n",
            ),
        ] {
            assert_eq!(checked(source, width), expected);
        }
    }
}

#[test]
fn rule_snapshots_at_three_widths() {
    let source = include_str!("fixtures/format/rules.iris");
    for (width, expected) in [
        (40, include_str!("fixtures/format/rules.40.txt")),
        (80, include_str!("fixtures/format/rules.80.txt")),
        (100, include_str!("fixtures/format/rules.100.txt")),
    ] {
        assert_eq!(checked(source, width), expected, "rule snapshot at {width}");
    }
}

#[test]
fn moved_options_and_delimiter_suffixes() {
    assert_eq!(
        checked(
            "list values:int until first_value+second_value offset -1",
            30
        ),
        "list values: int\n  until first_value +\n    second_value\n  offset -1\n"
    );
    assert_eq!(
        checked(
            "grid[2][2]:alphabet \"abcdefghijklmnopqrstuvwxy\" blank \"long_blank_alphabet\"",
            30
        ),
        "grid[2][2]: alphabet\n  \"abcdefghijklmnopqrstuvwxy\"\n  blank \"long_blank_alphabet\"\n"
    );
    assert_eq!(checked("ignore trailing", 10), "ignore trailing\n");
    assert_eq!(
        checked("repeat until eof {}", 12),
        "repeat\n  until eof {}\n"
    );
    assert_eq!(checked("long_name:int", 10), "long_name:\n  int\n");
    assert_eq!(
        checked("let total=sum(first_value,second_value,third_value)", 55),
        "let total = sum(first_value, second_value, third_value)\n"
    );
}

#[test]
fn joined_header_suffixes_are_measured() {
    for (source, width, expected) in [
        (
            "if first_value {} else {}",
            24,
            "if\n  first_value {} else {}\n",
        ),
        (
            "if first_value {} else if[1]:int",
            32,
            "if\n  first_value {} else if[1]: int\n",
        ),
    ] {
        assert_eq!(checked(source, width), expected);
    }
}

#[test]
fn statement_widths_around_flat_size() {
    use gecode_iris_syntax::{SyntaxKind as K, parse};
    let source = include_str!("fixtures/format/rules.iris");
    let flat = checked(source, usize::MAX);
    let parsed = parse(flat.as_bytes(), "flat", LanguageVersion::V0_1).unwrap();
    let mut checks = 0;
    for statement in parsed.syntax().descendants().filter(|node| {
        matches!(
            node.kind(),
            K::Param
                | K::Let
                | K::Format
                | K::Declaration
                | K::List
                | K::Setting
                | K::Expect
                | K::Line
                | K::Repeat
                | K::Each
                | K::Index
                | K::Check
                | K::Sections
                | K::If
        )
    }) {
        let text = statement.text().to_string();
        if statement.kind() == K::Each {
            // R8 keeps the record-name group together; it has no permitted break.
            let exact = 2 + text.trim().chars().count();
            for width in exact - 2..=exact + 2 {
                let output = checked(&format!("repeat 1 {{{text}}}"), width);
                assert!(output.lines().any(|line| line.trim() == text.trim()));
                checks += 1;
            }
            continue;
        }
        let exact = text.lines().map(|line| line.chars().count()).max().unwrap();
        for width in exact.saturating_sub(2).max(1)..=exact + 2 {
            let output = checked(&text, width);
            let after = parse(output.as_bytes(), "width", LanguageVersion::V0_1).unwrap();
            for (line_index, line) in output.lines().enumerate() {
                if line.chars().count() <= width {
                    continue;
                }
                // R11 allows an attached comment or protected token to overflow.
                // Test code before comments and the other physical lines too.
                let start = output
                    .split_inclusive('\n')
                    .take(line_index)
                    .map(str::len)
                    .sum::<usize>();
                let comment = after
                    .syntax()
                    .descendants_with_tokens()
                    .filter_map(|e| e.into_token())
                    .find(|t| {
                        t.kind() == K::Comment
                            && usize::from(t.text_range().start()) >= start
                            && usize::from(t.text_range().start()) < start + line.len()
                    });
                let code = comment.map_or(line, |t| {
                    &output[start..usize::from(t.text_range().start())]
                });
                let code = code.trim_end();
                // An atomic condition followed only by blocks has just
                // the R14 break after `if`. Overflow is allowed when its entire
                // continuation still cannot fit (braces/else may not split).
                let atomic_condition = statement.kind() == K::If
                    && statement
                        .children()
                        .all(|child| matches!(child.kind(), K::Name | K::Literal | K::Block));
                if atomic_condition
                    && code.starts_with("if ")
                    && 2 + code[3..].chars().count() > width
                {
                    continue;
                }
                if statement.kind() == K::Declaration
                    && statement
                        .children()
                        .filter(|child| child.kind() == K::Type)
                        .all(|ty| {
                            ty.descendants_with_tokens()
                                .filter_map(|e| e.into_token())
                                .filter(|t| t.kind() != K::Whitespace)
                                .count()
                                == 1
                        })
                    && code
                        .rsplit_once(": ")
                        .is_some_and(|(_, ty)| 2 + ty.chars().count() > width)
                {
                    continue;
                }
                assert!(
                    code.chars().count() <= width,
                    "avoidable overflow at width {width}: {line:?}\nstatement:\n{text}\noutput:\n{output}"
                );
            }
            checks += 1;
        }
    }
    println!("statement boundary width checks={checks}");
}
