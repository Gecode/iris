//! Parses arbitrary bytes as a reader spec and checks that the syntax tree is
//! lossless: its text reconstructs the source byte for byte.
#![no_main]

use iris_syntax::{LanguageVersion, ParseError, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|source: &[u8]| {
    match parse(source, "fuzz.iris", LanguageVersion::V0_1) {
        Ok(parsed) => assert_eq!(
            parsed.syntax().to_string().as_bytes(),
            source,
            "syntax tree text differs from the source"
        ),
        Err(ParseError::InvalidUtf8(_)) => assert!(std::str::from_utf8(source).is_err()),
        Err(ParseError::SourceTooLarge { .. }) => unreachable!("libFuzzer inputs fit in u32"),
    }
});
