use std::collections::BTreeSet;

use super::*;
use crate::grammar::embedded_rule_names;

fn difference(left: &str, right: &str) -> Option<Difference> {
    compare(left.as_bytes(), right.as_bytes()).expect("both specs are recognized")
}

fn assert_same(left: &str, right: &str) {
    assert_eq!(difference(left, right), None, "{left:?} and {right:?}");
}

fn assert_different(left: &str, right: &str) {
    assert!(
        difference(left, right).is_some(),
        "{left:?} and {right:?} compared as the same"
    );
    assert!(
        difference(right, left).is_some(),
        "{right:?} and {left:?} compared as the same"
    );
}

#[test]
fn every_grammar_rule_is_classified_once() {
    for rule in embedded_rule_names() {
        let classes = [
            LEAF_RULES.contains(&rule.as_str()),
            Frame::lowering(&rule).is_some(),
            PASS_THROUGH_RULES.contains(&rule.as_str()),
            SOURCE_RULES.contains(&rule.as_str()),
        ];
        assert_eq!(
            classes.iter().filter(|class| **class).count(),
            1,
            "{rule} is in {classes:?}"
        );
    }
    let names: BTreeSet<_> = embedded_rule_names().into_iter().collect();
    for rule in PASS_THROUGH_RULES.iter().chain(SOURCE_RULES) {
        assert!(names.contains(*rule), "{rule} is not a grammar rule");
    }
}

#[test]
fn ignores_whitespace_and_comments() {
    assert_same(
        "format a\nn:int   # the size\nline \"x\"\n",
        "format a n : int line\n\n# a comment\n\"x\"",
    );
}

#[test]
fn ignores_redundant_parentheses() {
    assert_same("let x=(a)+((b))", "let x=a+b");
    assert_same("let x=(a-b)-c", "let x=a-b-c");
    assert_same("let x=a*(b)[(1)]", "let x=a*b[1]");
    assert_same("x:(1)..(2)", "x:1..2");
    assert_same("let x=(f(1))", "let x=f(1)");
}

#[test]
fn keeps_associativity_and_precedence() {
    assert_different("let x=a-(b-c)", "let x=a-b-c");
    assert_different("let x=(a+b)*c", "let x=a+b*c");
    assert_different("let x=(-a)^b", "let x=-a^b");
    assert_different("let x=(a^b)^c", "let x=a^b^c");
}

#[test]
fn ignores_renamed_list_each_and_comprehension_names() {
    assert_same(
        "repeat 2 as rows { x:int } check all(r in rows: r.x>0)",
        "repeat 2 as lines { x:int } check all(l in lines: l.x>0)",
    );
    assert_same(
        "repeat 2 { each row x:int } check row.x>0",
        "repeat 2 { each line x:int } check line.x>0",
    );
    assert_same(
        "repeat 2 as rows { each row x:int } check row.x>0 check size(rows)==2",
        "repeat 2 as r1 { each r2 x:int } check r2.x>0 check size(r1)==2",
    );
    assert_same(
        "repeat 2 as a { each b repeat 2 as c { x:int } } check size(b.c)==2",
        "repeat 2 as p { each q repeat 2 as r { x:int } } check size(q.r)==2",
    );
    assert_same(
        "let y=sum(i in v: all(j in v: i<j))",
        "let y=sum(p in v: all(q in v: p<q))",
    );
}

#[test]
fn ignores_omitted_unused_each_and_as() {
    assert_same("repeat 2 as rows { x:int }", "repeat 2 { x:int }");
    assert_same("repeat 2 { each row x:int }", "repeat 2 { x:int }");
    assert_same("repeat 2 as rows { each row x:int }", "repeat 2 { x:int }");
    assert_same(
        "repeat until eof as rows { x:int }",
        "repeat until eof { x:int }",
    );
    assert_same(
        "repeat until \"e\" store s as rows { x:int }",
        "repeat until \"e\" store s { x:int }",
    );
}

#[test]
fn a_used_record_name_counts_at_its_references() {
    assert_same(
        "repeat 2 { each row x:int } check row.x>0",
        "repeat 2 as record_1 { each row x:int } check row.x>0",
    );
    assert_different(
        "repeat 2 { each row x:int } check row.x>0",
        "repeat 2 { x:int } check row.x>0",
    );
}

#[test]
fn ignores_number_and_string_spelling() {
    assert_same("let x=007", "let x=7");
    assert_same("x:alphabet \"ab\" base -0", "x:alphabet \"ab\" base 0");
    assert_same("let x=1.50", "let x=1.5");
    assert_same("let x=1e2", "let x=100.0");
    assert_same("let x=\"a\\tb\"", "let x=\"a\tb\"");
    assert_same("line \"a\\tb\"", "line \"a\tb\"");
}

#[test]
fn ignores_format_name_spelling() {
    assert_same("format a-b.c", "format \"a-b.c\"");
    assert_same("format \"a\"-b", "format a-b");
}

#[test]
fn reports_a_reference_to_a_different_repeat() {
    assert_different(
        "repeat 2 { each a x:int } repeat 2 { each b x:int } check a.x>0",
        "repeat 2 { each a x:int } repeat 2 { each b x:int } check b.x>0",
    );
    assert_different(
        "repeat 2 as p { x:int } repeat 2 as q { y:int } check size(p)==2",
        "repeat 2 as p { x:int } repeat 2 as q { y:int } check size(q)==2",
    );
    assert_different(
        "repeat 2 as a { each b repeat 2 as c { x:int } repeat 2 as d { x:int } } check size(b.c)==2",
        "repeat 2 as a { each b repeat 2 as c { x:int } repeat 2 as d { x:int } } check size(b.d)==2",
    );
}

#[test]
fn reports_each_against_list() {
    assert_different(
        "repeat 2 as rows { each row x:int } check defined(rows)",
        "repeat 2 as rows { each row x:int } check defined(row)",
    );
}

#[test]
fn reports_a_different_bound_variable() {
    assert_different(
        "let y=sum(i in v: all(j in v: i<j))",
        "let y=sum(i in v: all(j in v: j<i))",
    );
    assert_different("let y=sum(i in v: i)", "let y=sum(i in v: j)");
}

#[test]
fn a_comprehension_variable_starts_after_its_domain() {
    assert_same("let y=sum(x in x: x)", "let y=sum(v in x: v)");
    assert_different("let y=sum(x in x: x)", "let y=sum(v in x: x)");
}

#[test]
fn reports_value_and_type_names() {
    assert_different("let x=a", "let x=b");
    assert_different("let x=1", "let y=1");
    assert_different("let x=r.a", "let x=r.b");
    assert_different("type t=int a:t", "type u=int a:u");
    assert_different("line \"{a}\"", "line \"{b}\"");
    assert_different(
        "repeat until \"e\" store s { x:int }",
        "repeat until \"e\" store t { x:int }",
    );
}

#[test]
fn reports_integer_against_float_and_range_kinds() {
    assert_different("let x=1", "let x=1.0");
    assert_different("x:1..2", "x:1..<2");
    assert_different("x:alphabet \"ab\" base 1", "x:alphabet \"ab\" base -1");
    assert_different("let x=\"a\"", "let x=\"b\"");
    assert_different("let x=-1", "let x=1");
    assert_different("let x=!a", "let x=-a");
}

#[test]
fn reports_pattern_differences() {
    assert_different("line \"{x}\"", "line \"{x:word}\"");
    assert_different("line \"{}\"", "line \"{_}\"");
    assert_different("line \"{x}\"", "line \"{$x}\"");
    assert_different("line \"a b\"", "line \"a  b\"");
    assert_different("line \"a b\"", "line \"a\\tb\"");
    assert_different("line \"a...\"", "line \"a... \"");
    assert_different("line \"a\"", "find \"a\"");
    assert_different("line \"{x:0..2}\"", "line \"{x:0..<2}\"");
    assert_same("line \"{x: 0 .. (2) }\"", "line \"{x:0..2}\"");
}

#[test]
fn reports_block_structure() {
    assert_different(
        "if a { x:int } else if b { y:int }",
        "if a { x:int } else { if b { y:int } }",
    );
    assert_different("sections { \"a\" {} }", "sections { \"a\" }");
    assert_different("if a {} else {}", "if a {}");
}

#[test]
fn reports_statement_and_clause_order() {
    assert_different("x:int y:int", "y:int x:int");
    assert_different(
        "sections until \"a\" skip other { \"b\" }",
        "sections skip other until \"a\" { \"b\" }",
    );
    assert_different(
        "sections { \"a\" optional last }",
        "sections { \"a\" optional }",
    );
}

#[test]
fn reports_grouped_settings() {
    assert_different("comment \"a\" \"b\"", "comment \"a\" comment \"b\"");
}

#[test]
fn reports_a_format_joins_raw_integer() {
    assert_different("format a-007", "format a-7");
}

#[test]
fn reports_the_end_of_the_shorter_spec() {
    let found = difference("x:int\ny:int", "x:int").expect("specs differ");
    assert_eq!(found.left_line, 2);
    assert_eq!(found.right_line, 1);
    assert!(found.message.contains("the end of the spec"), "{found:?}");
}

#[test]
fn reports_lines_of_each_side() {
    let found = difference("\n\nlet x=1", "let x=2").expect("specs differ");
    assert_eq!((found.left_line, found.right_line), (3, 1));
    assert_eq!(found.message, "integer 1 differs from integer 2");
}

#[test]
fn unrecognized_input_names_its_side() {
    let failure = compare(b"let", b"x:int").expect_err("left is rejected");
    assert_eq!(failure.side, Side::Left);
    let failure = compare(b"x:int", b"let").expect_err("right is rejected");
    assert_eq!(failure.side, Side::Right);
}

#[test]
fn a_duplicate_record_name_cannot_be_compared() {
    let failure = compare(b"repeat 1 as a {} repeat 1 as a {}", b"x:int")
        .expect_err("the record name is declared twice");
    assert_eq!(failure.side, Side::Left);
    assert!(failure.message.contains("'a'"), "{failure}");
}

#[test]
fn long_chains_and_deep_nesting_do_not_recurse() {
    let chain = format!("let x=1{}", "+1".repeat(255));
    assert_same(&chain, &chain);
    let blocks = format!("{}{}", "if a {".repeat(256), "}".repeat(256));
    assert_same(&blocks, &blocks);
}

#[test]
fn manifests_count_comparisons_skips_and_differences() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let write = |name: &str, text: &str| {
        fs::write(directory.path().join(name), text).expect("write a test file");
    };
    write("a.iris", "x:int");
    write("same.iris", "x : int # same");
    write("other.iris", "y:int");
    write("rejected.iris", "let");
    write(
        "manifest",
        "a.iris\tsame.iris\n\
         a.iris\tother.iris\n\
         a.iris\tmissing.iris\n\
         a.iris\trejected.iris\n\
         rejected.iris\n",
    );
    let batch = compare_manifest(&directory.path().join("manifest")).expect("usable manifest");
    assert_eq!((batch.compared, batch.skipped), (4, 1));
    assert_eq!(batch.differences.len(), 3);
    assert!(batch.differences[0].message.contains("differs from"));
    assert!(batch.differences[1].message.contains("cannot read"));
    assert!(batch.differences[2].message.contains("rejected"));
    assert_eq!(
        batch.summary(),
        "Compare summary: compared=4 skipped=1 differences=3"
    );
}

#[test]
fn a_manifest_needs_usable_originals_and_lines() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let manifest = directory.path().join("manifest");
    fs::write(directory.path().join("rejected.iris"), "let").expect("write a test file");
    for text in [
        "missing.iris\tmissing.iris\n",
        "rejected.iris\trejected.iris\n",
        "a\tb\tc\n",
        "\tb\n",
        "a\t\n",
    ] {
        fs::write(&manifest, text).expect("write the manifest");
        assert!(compare_manifest(&manifest).is_err(), "{text:?}");
    }
    assert!(compare_manifest(&directory.path().join("absent")).is_err());
}
