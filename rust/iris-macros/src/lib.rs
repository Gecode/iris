//! The procedural macro behind `gecode_iris::iris!`.
//!
//! Use the macro through `gecode-iris`. That crate's `iris!` passes its own
//! `$crate` path to [`iris!`](macro@iris), so the expansion names the reader
//! API correctly even when a dependent renames `gecode-iris`.
//!
//! The macro checks a reader spec at compile time with `gecode-iris-syntax` and
//! `gecode-iris-sema`, exactly as `iris check` does, and reports the first language
//! error as a compile error. A valid spec expands to a value with a `spec`
//! field, `Spec::parse` on the same bytes and origin, and one field per typed
//! handle, the declarations that `gecode-iris-gen` prints for `iris gen --lang
//! rust`. This crate never depends on `gecode-iris-core`.
use gecode_iris_gen::{RustView, rust_view};
use gecode_iris_syntax::{LanguageVersion, ParseError};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::ops::Range;
use std::path::PathBuf;
use syn::{
    Ident, LitStr, Token, bracketed,
    parse::{Parse, ParseStream},
};

/// Check a reader spec and expand to its `Spec` and typed handles.
///
/// The input is `[CRATE_PATH] SOURCE`, where `CRATE_PATH` names the crate
/// that provides `Spec` and `build`, and `SOURCE` is a string literal or
/// `file = "PATH"`.
#[proc_macro]
pub fn iris(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    expand_invocation(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Parse a line pattern literal and expand to `Pattern` calls.
///
/// The input is `[CRATE_PATH] "LITERAL", NAME = EXPR, ...`, where `CRATE_PATH`
/// names the crate that provides `__private::Pattern`.
#[proc_macro]
pub fn pattern(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    expand_pattern(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_invocation(input: TokenStream) -> syn::Result<TokenStream> {
    let invocation: Invocation = syn::parse2(input)?;
    let spec = load(&invocation.source)?;
    let view = checked_view(&spec)?;
    Ok(expand(&invocation.krate, &spec, &view))
}

/// One macro call: the path of the crate that provides `Spec` and `build`, and
/// the spec.
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

/// The spec's typed handles, or its first error in the order `iris check` uses.
fn checked_view(spec: &LoadedSpec) -> syn::Result<RustView> {
    let parsed = match gecode_iris_syntax::parse(&spec.bytes, &spec.origin, LanguageVersion::V0_1) {
        Ok(parsed) => parsed,
        Err(ParseError::InvalidUtf8(diagnostic)) => {
            return Err(spec.language_error(&gecode_iris_sema::Error::from(&diagnostic)));
        }
        Err(error) => {
            return Err(syn::Error::new(
                spec.source.literal().span(),
                format!("{}: {error}", spec.origin),
            ));
        }
    };
    rust_view(&gecode_iris_sema::analyze(&parsed)).map_err(|error| spec.language_error(&error))
}

impl LoadedSpec<'_> {
    /// The registry code, then the rendering `iris check` prints.
    fn language_error(&self, error: &gecode_iris_sema::Error) -> syn::Error {
        let message = format!(
            "{}: {}",
            error.code.as_str(),
            gecode_iris_sema::render_error(error, &self.bytes)
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

/// The value of the macro: a struct declared in the expansion, with the spec in
/// `spec` and one field per handle. The handle types name the imported items of
/// `build` exactly as the view text does.
fn expand(krate: &TokenStream, spec: &LoadedSpec, view: &RustView) -> TokenStream {
    let bytes = &spec.bytes_expr;
    let origin = &spec.origin;
    let imports = view.imports.iter().map(|name| identifier(name));
    let fields: Vec<Ident> = view
        .handles
        .iter()
        .map(|handle| identifier(&handle.identifier))
        .collect();
    let types = view.handles.iter().map(|handle| {
        syn::parse_str::<syn::Type>(&handle.handle_type)
            .expect("the view generator prints Rust types")
    });
    let entries = view.handles.iter().map(|handle| &handle.entry);
    let import = (!view.imports.is_empty()).then(|| quote!(use #krate::build::{#(#imports),*};));
    quote! {
        {
            #import
            struct TypedSpec {
                pub spec: #krate::Spec,
                #(pub #fields: #types,)*
            }
            TypedSpec {
                spec: #krate::Spec::parse(#bytes, #origin)
                    .expect("iris! checked this reader spec at compile time"),
                #(#fields: Field::declared(#entries),)*
            }
        }
    }
}

/// An identifier that the view generator chose, which is never a keyword.
fn identifier(name: &str) -> Ident {
    Ident::new(name, Span::call_site())
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

/// One `pattern!` call.
struct PatternInvocation {
    krate: TokenStream,
    literal: LitStr,
    arguments: Vec<(Ident, syn::Expr)>,
}

impl Parse for PatternInvocation {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let krate;
        bracketed!(krate in input);
        let krate = krate.parse()?;
        let literal = input.parse()?;
        let mut arguments = Vec::new();
        while input.parse::<Option<Token![,]>>()?.is_some() && !input.is_empty() {
            let name = input.parse()?;
            input.parse::<Token![=]>()?;
            arguments.push((name, input.parse()?));
        }
        Ok(Self {
            krate,
            literal,
            arguments,
        })
    }
}

/// A piece of a pattern literal.
#[derive(Debug, PartialEq, Eq)]
enum PatternPiece {
    Text(String),
    Interpolation(String),
}

/// Split a literal into text and `{name}` interpolations.
fn split_pattern(literal: &str) -> Result<Vec<PatternPiece>, String> {
    if literal.contains("...") {
        return Err("`...` has no spelling in a literal; use `rest_of_line()`".into());
    }
    let mut pieces = Vec::new();
    let mut rest = literal;
    while let Some(open) = rest.find('{') {
        let (text, tail) = rest.split_at(open);
        if !text.is_empty() {
            pieces.push(PatternPiece::Text(text.into()));
        }
        let tail = &tail[1..];
        if tail.starts_with('{') {
            return Err("`{{` has no spelling: a pattern has no literal `{`".into());
        }
        let Some(close) = tail.find('}') else {
            return Err("this `{` is not closed".into());
        };
        let name = &tail[..close];
        if syn::parse_str::<Ident>(name).is_err() {
            return Err(format!(
                "`{{{name}}}` is not an interpolation; write `{{name}}`, and use \
                 `blank()`, `discard()`, `reference(x)` or `bind(x, type)` for other captures"
            ));
        }
        pieces.push(PatternPiece::Interpolation(name.into()));
        rest = &tail[close + 1..];
    }
    if !rest.is_empty() {
        pieces.push(PatternPiece::Text(rest.into()));
    }
    Ok(pieces)
}

fn expand_pattern(input: TokenStream) -> syn::Result<TokenStream> {
    let PatternInvocation {
        krate,
        literal,
        arguments,
    } = syn::parse2(input)?;
    let pieces = split_pattern(&literal.value())
        .map_err(|message| syn::Error::new(literal.span(), message))?;
    let mut calls = Vec::with_capacity(pieces.len());
    let mut used = vec![false; arguments.len()];
    for piece in pieces {
        calls.push(match piece {
            PatternPiece::Text(text) => quote!(.text(#text)),
            PatternPiece::Interpolation(name) => {
                let argument = arguments.iter().position(|(key, _)| *key == name);
                if let Some(index) = argument {
                    used[index] = true;
                    let value = &arguments[index].1;
                    quote!(.part(#value))
                } else {
                    let name = Ident::new(&name, literal.span());
                    quote!(.part(#name))
                }
            }
        });
    }
    if let Some(index) = used.iter().position(|used| !used) {
        return Err(syn::Error::new(
            arguments[index].0.span(),
            "this argument is not used in the pattern",
        ));
    }
    Ok(quote!(#krate::__private::Pattern::new() #(#calls)*))
}

/// The handle fields of an expansion against the declarations that `iris gen
/// --lang rust` prints for the same spec.
#[cfg(test)]
mod handle_field_tests {
    use super::expand_invocation;
    use gecode_iris_gen::{Language, RustHandle, RustView, view_code};
    use gecode_iris_syntax::LanguageVersion;
    use quote::{ToTokens, quote};
    use std::path::Path;

    /// What the expansion of an inline `source` declares beside `spec`, with
    /// each handle type as the text of its tokens.
    fn expanded(source: &str) -> RustView {
        let tokens = expand_invocation(quote!([::gecode_iris] #source)).unwrap();
        let syn::Expr::Block(block) = syn::parse2(tokens).unwrap() else {
            panic!("the expansion is a block");
        };
        let mut imports = Vec::new();
        let mut fields = Vec::new();
        let mut entries = Vec::new();
        for statement in block.block.stmts {
            match statement {
                syn::Stmt::Item(syn::Item::Use(item)) => imports = imported(&item.tree),
                syn::Stmt::Item(syn::Item::Struct(item)) => {
                    fields = item.fields.into_iter().collect()
                }
                syn::Stmt::Expr(syn::Expr::Struct(value), None) => {
                    entries = value.fields.into_iter().map(|field| field.expr).collect();
                }
                other => panic!("unexpected statement {}", other.to_token_stream()),
            }
        }
        let spec = fields.remove(0);
        assert_eq!(spec.ident.unwrap(), "spec");
        assert_eq!(type_text(&spec.ty), ":: gecode_iris :: Spec");
        entries.remove(0);
        let handles = fields
            .iter()
            .zip(&entries)
            .map(|(field, value)| RustHandle {
                identifier: field
                    .ident
                    .as_ref()
                    .expect("the fields are named")
                    .to_string(),
                handle_type: type_text(&field.ty),
                entry: entry(value),
            })
            .collect();
        RustView { imports, handles }
    }

    /// What `iris gen --lang rust` declares for `source`.
    fn printed(source: &str) -> RustView {
        let parsed =
            gecode_iris_syntax::parse(source.as_bytes(), "x.iris", LanguageVersion::V0_1).unwrap();
        declared_in(&view_code(&gecode_iris_sema::analyze(&parsed), Language::Rust).unwrap())
    }

    /// What a view module's `text` declares, as [`expanded`] gives it.
    fn declared_in(text: &str) -> RustView {
        let mut view = RustView::default();
        for item in syn::parse_file(text).unwrap().items {
            match item {
                syn::Item::Use(item) => view.imports = imported(&item.tree),
                syn::Item::Const(item) => view.handles.push(RustHandle {
                    identifier: item.ident.to_string(),
                    handle_type: type_text(&item.ty),
                    entry: entry(&item.expr),
                }),
                other => panic!("unexpected item {}", other.to_token_stream()),
            }
        }
        view
    }

    /// The names a `use` brings in from its last path segment.
    fn imported(tree: &syn::UseTree) -> Vec<String> {
        match tree {
            syn::UseTree::Path(path) => imported(&path.tree),
            syn::UseTree::Group(group) => group.items.iter().flat_map(imported).collect(),
            syn::UseTree::Name(name) => vec![name.ident.to_string()],
            other => panic!("unexpected import {}", other.to_token_stream()),
        }
    }

    fn type_text(handle_type: &syn::Type) -> String {
        handle_type.to_token_stream().to_string()
    }

    /// The entry of `Field::declared("entry")`.
    fn entry(expr: &syn::Expr) -> String {
        if let syn::Expr::Call(call) = expr
            && call.func.to_token_stream().to_string() == "Field :: declared"
            && let Some(syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(entry),
                ..
            })) = call.args.first()
        {
            return entry.value();
        }
        panic!("not a declared handle: {}", expr.to_token_stream());
    }

    #[test]
    fn the_fields_of_every_format_are_the_handles_iris_gen_prints() {
        let formats = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../formats");
        for name in [
            "dimacs-cnf",
            "jobshop",
            "nonogram",
            "sudoku",
            "tsplib",
            "tsptw",
        ] {
            let source =
                std::fs::read_to_string(formats.join(name).join(format!("{name}.iris"))).unwrap();
            let declarations = expanded(&source);
            assert!(!declarations.handles.is_empty(), "{name}");
            assert_eq!(declarations, printed(&source), "{name}");
        }
    }

    /// `iris-cli/tests/fixtures/jobshop-view.rs` records what the `iris` binary
    /// prints for jobshop, and the CLI's `gen` tests keep it current.
    #[test]
    fn the_fields_of_jobshop_are_the_recorded_iris_gen_output() {
        let rust = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let read = |path: &str| std::fs::read_to_string(rust.join(path)).unwrap();
        assert_eq!(
            expanded(&read("../formats/jobshop/jobshop.iris")),
            declared_in(&read("iris-cli/tests/fixtures/jobshop-view.rs"))
        );
    }

    #[test]
    fn open_shapes_and_unusual_names_match_iris_gen() {
        let specs = [
            "x:int let x=\"s\" y:int y:float z:word u:num",
            "n:int param p a[1]:int param q=a repeat n {let r=a}",
            "x:int if x {y:float} if x {m[1][1]:num}",
            "fn:int self:int Self:int a_b:int aB:int spec:int spec_2:int crate:int \
             repeat until \"END\" store _ {t:int}",
            &("repeat 1 {".repeat(9) + "v:int w:int" + &"}".repeat(9)),
            "skip line\n",
        ];
        for source in specs {
            assert_eq!(expanded(source), printed(source), "{source}");
        }
    }

    #[test]
    fn an_entry_named_spec_leaves_the_field_to_the_spec() {
        let handles = expanded("spec:int").handles;
        assert_eq!(
            handles,
            [RustHandle {
                identifier: "spec_2".into(),
                handle_type: "Field < Int >".into(),
                entry: "spec".into(),
            }]
        );
    }
}
