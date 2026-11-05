use gecode_iris_sema::{Error, ErrorCode};
use gecode_iris_syntax::{DiagnosticCode, LanguageVersion};
fn error(bytes: &[u8]) -> Error {
    match gecode_iris_syntax::parse(bytes, "test", LanguageVersion::V0_1) {
        Ok(parsed) => gecode_iris_sema::analyze(&parsed)
            .first_error()
            .unwrap()
            .clone(),
        Err(gecode_iris_syntax::ParseError::InvalidUtf8(d)) => Error::from(&d),
        Err(error) => panic!("{error}"),
    }
}
#[test]
fn registry_gaps_and_unicode_wording() {
    for (source, code, message) in [
        (
            "let x=9223372036854775808",
            DiagnosticCode::Number,
            "bad number 9223372036854775808",
        ),
        (
            "let x=@",
            DiagnosticCode::Character,
            "unexpected character '@'",
        ),
        (
            "let x=é",
            DiagnosticCode::Character,
            "unexpected character 'é'",
        ),
        (
            "let x=\"\\r\"",
            DiagnosticCode::String,
            "unknown escape '\\r' in string (use \\n, \\t, \\\\, or \\\")",
        ),
    ] {
        let result = error(source.as_bytes());
        assert_eq!(result.code, ErrorCode::Syntax(code));
        assert_eq!(result.message, message);
        assert_eq!(result.location.column, None);
    }
    let result = error(b"unknown\n# \xff");
    assert_eq!(result.code, ErrorCode::Syntax(DiagnosticCode::Utf8));
    assert_eq!((result.location.line, result.location.column), (2, Some(3)));
}
#[test]
fn local_checks_interleave_with_later_syntax() {
    for (source, code, line, message) in [
        (
            "a[1]:rest bogus",
            ErrorCode::Type,
            1,
            "'rest' values cannot be arrays",
        ),
        (
            "type int\n ?",
            ErrorCode::Syntax(DiagnosticCode::Character),
            2,
            "unexpected character '?'",
        ),
        (
            "type int\n )",
            ErrorCode::NameConflict,
            2,
            "'int' is a base type",
        ),
        (
            "a[1]:int layout )",
            ErrorCode::Type,
            1,
            "'layout' needs a two-dimensional array of numbers",
        ),
        (
            "a:word offset )",
            ErrorCode::Type,
            1,
            "'offset' needs numbers",
        ),
        (
            "line \"{x:alphabet \\\"a\\\"} {y:@}\"",
            ErrorCode::Syntax(DiagnosticCode::Pattern),
            1,
            "a capture cannot be of an alphabet type",
        ),
        (
            "line \"{x:alphabet \\\"a\\\"} {bad name}\"",
            ErrorCode::Syntax(DiagnosticCode::Pattern),
            1,
            "capture name 'bad name' in pattern \"{x:alphabet \"a\"} {bad name}\" is not a name",
        ),
        (
            "line \"{x:\\nunknown}\"",
            ErrorCode::UnknownType,
            2,
            "unknown type 'unknown'",
        ),
        (
            "gecode.foo\n:int",
            ErrorCode::Syntax(DiagnosticCode::Name),
            2,
            "'gecode.foo' cannot be defined: names starting with 'gecode.' are Gecode constants",
        ),
    ] {
        let result = error(source.as_bytes());
        assert_eq!(result.code, code, "{source}");
        assert_eq!(result.location.line, line, "{source}");
        assert_eq!(result.message, message, "{source}");
    }
}
#[test]
fn global_and_operand_order() {
    for (source, code, line) in [
        (
            "a[0]:int let z=sum(\nx in a:x)\nx:int",
            ErrorCode::NameConflict,
            2,
        ),
        (
            "type t=int\nt:int\na[0]:int\nlet z=sum(x in a:x)\nx:int",
            ErrorCode::NameConflict,
            4,
        ),
        (
            "let z=(1).x\nrepeat 0 as rows {a:int}\nrows:int",
            ErrorCode::NameConflict,
            2,
        ),
        (
            "a[rows]:0..(2).bad\nrepeat 0 as rows {}",
            ErrorCode::Field,
            1,
        ),
        (
            "repeat 0 as rows {each row a:int}\nlet z=row[rows]",
            ErrorCode::RecordScope,
            2,
        ),
        (
            "repeat 0 as rows {each row a:int}\ncheck rows[0][(1).bad]",
            ErrorCode::Field,
            2,
        ),
        (
            "repeat 0 as rows {each row a:int}\ncheck min(rows,(1).bad)",
            ErrorCode::RecordValue,
            2,
        ),
    ] {
        let result = error(source.as_bytes());
        assert_eq!(result.code, code, "{source}");
        assert_eq!(result.location.line, line, "{source}");
    }
}

#[test]
fn unclosed_blocks_report_missing_brace() {
    for source in [
        "sections { \"A\"",
        "sections {",
        "sections { \"A\" {let x=1}",
        "repeat 1 {",
        "instances {",
        "if 1 {} else {",
    ] {
        let result = error(source.as_bytes());
        assert_eq!(
            result.code,
            ErrorCode::Syntax(DiagnosticCode::Syntax),
            "{source}"
        );
        assert_eq!(result.message, "missing '}'", "{source}");
        assert_eq!((result.location.line, result.location.column), (1, None));
    }
}

#[test]
fn an_error_past_its_statement_keeps_its_range_and_names_its_owner() {
    let error = error(b"x[2]: rest\ny: int\n");
    assert_eq!(error.location.line, 2);
    assert_eq!(usize::from(error.range.start()), 11);
    assert_eq!(error.owner.map(usize::from), Some(0));
}

#[test]
fn a_block_owns_what_follows_its_statements() {
    let error = error(b"  repeat 1 {\n  x: int\n");
    assert_eq!(error.message, "missing '}'");
    assert_eq!(error.owner.map(usize::from), Some(2));
}
