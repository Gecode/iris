//! The procedural macro behind `gecode_iris::iris!`.
//!
//! Use the macro through `gecode-iris`. That crate's `iris!` passes its own
//! `$crate` path to [`iris!`](macro@iris), so the expansion names the reader
//! API correctly even when a dependent renames `gecode-iris`.
//!
//! The macro checks a reader spec at compile time with `iris-syntax` and
//! `iris-sema`, exactly as `iris check` does, and reports the first language
//! error as a compile error. A valid spec expands to `Spec::parse` on the same
//! bytes and origin. This crate never depends on `iris-core`.
use iris_syntax::{LanguageVersion, ParseError};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::ops::Range;
use std::path::PathBuf;
use syn::{
    Ident, LitStr, Token, bracketed,
    parse::{Parse, ParseStream},
};

/// Check a reader spec and expand to a `Spec`.
///
/// The input is `[CRATE_PATH] SOURCE`, where `CRATE_PATH` names the crate
/// that provides `Spec` and `SOURCE` is a string literal or `file = "PATH"`.
#[proc_macro]
pub fn iris(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    expand_invocation(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_invocation(input: TokenStream) -> syn::Result<TokenStream> {
    let invocation: Invocation = syn::parse2(input)?;
    let spec = load(&invocation.source)?;
    check(&spec)?;
    Ok(expand(&invocation.krate, &spec))
}

/// One macro call: the path of the crate that provides `Spec`, and the spec.
struct Invocation {
    krate: TokenStream,
    source: SpecSource,
}

/// Where the reader spec text comes from.
enum SpecSource {
    /// A string literal holding the spec text.
    Inline(LitStr),
    /// `file = "PATH"`, with PATH relative to `CARGO_MANIFEST_DIR`.
    File(LitStr),
}

const EXPECTED_SOURCE: &str = "expected a string literal or `file = \"PATH\"`";

impl Parse for Invocation {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let krate;
        bracketed!(krate in input);
        Ok(Self {
            krate: krate.parse()?,
            source: input.parse()?,
        })
    }
}

impl Parse for SpecSource {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let source = if input.peek(LitStr) {
            Self::Inline(input.parse()?)
        } else if input.peek(Ident) && input.peek2(Token![=]) {
            let key: Ident = input.parse()?;
            if key != "file" {
                return Err(syn::Error::new(
                    key.span(),
                    format!("unknown key `{key}`; {EXPECTED_SOURCE}"),
                ));
            }
            input.parse::<Token![=]>()?;
            Self::File(input.parse()?)
        } else {
            return Err(input.error(EXPECTED_SOURCE));
        };
        let literal = source.literal();
        if !literal.suffix().is_empty() {
            return Err(syn::Error::new(
                literal.span(),
                format!(
                    "unexpected suffix `{}` on the string literal",
                    literal.suffix()
                ),
            ));
        }
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after the reader spec"));
        }
        Ok(source)
    }
}

impl SpecSource {
    fn literal(&self) -> &LitStr {
        match self {
            Self::Inline(literal) | Self::File(literal) => literal,
        }
    }
}

/// A reader spec's bytes and origin, and the expression that yields the same
/// bytes at run time.
struct LoadedSpec<'a> {
    source: &'a SpecSource,
    bytes: Vec<u8>,
    origin: String,
    bytes_expr: TokenStream,
}

fn load(source: &SpecSource) -> syn::Result<LoadedSpec<'_>> {
    match source {
        SpecSource::Inline(literal) => Ok(LoadedSpec {
            source,
            bytes: literal.value().into_bytes(),
            origin: "iris!".into(),
            bytes_expr: quote!(#literal.as_bytes()),
        }),
        SpecSource::File(literal) => {
            let written = literal.value();
            let path = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default())
                .join(&written);
            let bytes = std::fs::read(&path)
                .map_err(|error| syn::Error::new(literal.span(), format!("{written}: {error}")))?;
            let path = path.to_str().ok_or_else(|| {
                syn::Error::new(
                    literal.span(),
                    format!("{written}: the resolved path is not valid UTF-8"),
                )
            })?;
            // include_bytes! also records the file in Cargo's dependency
            // information, so editing the spec rebuilds the caller.
            Ok(LoadedSpec {
                source,
                bytes,
                origin: written,
                bytes_expr: quote!(::core::include_bytes!(#path)),
            })
        }
    }
}

/// Reject the spec with its first error, in the order `iris check` uses.
fn check(spec: &LoadedSpec) -> syn::Result<()> {
    let parsed = match iris_syntax::parse(&spec.bytes, &spec.origin, LanguageVersion::V0_1) {
        Ok(parsed) => parsed,
        Err(ParseError::InvalidUtf8(diagnostic)) => {
            return Err(spec.language_error(&iris_sema::Error::from(&diagnostic)));
        }
        Err(error) => {
            return Err(syn::Error::new(
                spec.source.literal().span(),
                format!("{}: {error}", spec.origin),
            ));
        }
    };
    match iris_sema::analyze(&parsed).first_error() {
        Some(error) => Err(spec.language_error(error)),
        None => Ok(()),
    }
}

impl LoadedSpec<'_> {
    /// The registry code, then the rendering `iris check` prints.
    fn language_error(&self, error: &iris_sema::Error) -> syn::Error {
        let message = format!(
            "{}: {}",
            error.code.as_str(),
            iris_sema::render_error(error, &self.bytes)
        );
        syn::Error::new(self.error_span(error.range.into()), message)
    }

    fn error_span(&self, range: Range<usize>) -> Span {
        match self.source {
            SpecSource::Inline(literal) => {
                let token = literal.token();
                token_range(&token.to_string(), range)
                    .and_then(|range| token.subspan(range))
                    .unwrap_or_else(|| literal.span())
            }
            SpecSource::File(path) => path.span(),
        }
    }
}

/// Map a byte range of a string literal's value to a range of its token text.
///
/// Returns `None` when escapes break the one-to-one byte mapping. The result
/// stays within the content and covers at least one byte, so an error at the
/// end of the spec marks the closing quote.
fn token_range(token: &str, range: Range<usize>) -> Option<Range<usize>> {
    let (open, close) = if let Some(raw) = token.strip_prefix('r') {
        let hashes = raw.len() - raw.trim_start_matches('#').len();
        (hashes + 2, hashes + 1)
    } else if token.contains('\\') {
        return None;
    } else {
        (1, 1)
    };
    let content_end = token.len() - close;
    let start = (open + range.start).min(content_end);
    let end = (open + range.end).min(content_end).max(start + 1);
    Some(start..end)
}

/// The spec as a run-time value. builders-005 extends this expansion with
/// typed handles; keep it the only place that shapes the result.
fn expand(krate: &TokenStream, spec: &LoadedSpec) -> TokenStream {
    let bytes = &spec.bytes_expr;
    let origin = &spec.origin;
    quote! {
        #krate::Spec::parse(#bytes, #origin)
            .expect("iris! checked this reader spec at compile time")
    }
}

#[cfg(test)]
mod tests {
    use super::token_range;

    #[test]
    fn raw_literals_offset_by_their_prefix() {
        assert_eq!(token_range(r#"r"ab""#, 1..2), Some(3..4));
        assert_eq!(token_range(r###"r##"a#b"##"###, 0..3), Some(4..7));
    }

    #[test]
    fn cooked_literals_without_escapes_offset_by_the_quote() {
        assert_eq!(token_range(r#""x:int""#, 2..5), Some(3..6));
    }

    #[test]
    fn cooked_literals_with_escapes_have_no_mapping() {
        assert_eq!(token_range(r#""x\n""#, 0..1), None);
    }

    #[test]
    fn empty_and_end_ranges_cover_one_byte() {
        assert_eq!(token_range(r#""ab""#, 1..1), Some(2..3));
        assert_eq!(token_range(r#""ab""#, 2..2), Some(3..4));
        assert_eq!(token_range(r##"r#"ab"#"##, 5..9), Some(5..6));
    }
}
