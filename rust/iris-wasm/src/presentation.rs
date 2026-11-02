use iris_syntax::{Parse, SyntaxKind as K, TextRange};
use serde::Serialize;

#[derive(Serialize)]
pub struct ColorRun {
    start: usize,
    end: usize,
    kind: &'static str,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    byte_start: usize,
    byte_end: usize,
    start: usize,
    end: usize,
}
pub fn range(range: TextRange, source: &str) -> Range {
    let start = usize::from(range.start()).min(source.len());
    let end = usize::from(range.end()).min(source.len());
    Range {
        byte_start: start,
        byte_end: end,
        start: source[..start].encode_utf16().count(),
        end: source[..end].encode_utf16().count(),
    }
}
/// Token text partitions the source; count UTF-16 once in source order.
pub fn colors(parsed: &Parse, source: &str) -> Vec<ColorRun> {
    if source.len() > 64 << 10 {
        return Vec::new();
    }
    let mut runs = Vec::new();
    let mut start = 0;
    for token in parsed
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        let end = start + token.text().encode_utf16().count();
        let kind = match token.kind() {
            K::Comment => "comment",
            K::String | K::PatternText | K::PatternToken => "string",
            K::Integer | K::Float => "number",
            K::Punct => "punctuation",
            K::Ident
                if token
                    .parent()
                    .is_some_and(|node| matches!(node.kind(), K::BaseType | K::TypeName)) =>
            {
                "type"
            }
            K::Ident
                if token.parent().is_some_and(|node| {
                    !matches!(
                        node.kind(),
                        K::Name | K::Field | K::CurrentValue | K::Capture | K::Declaration
                    )
                }) =>
            {
                "keyword"
            }
            _ => "plain",
        };
        runs.push(ColorRun { start, end, kind });
        start = end;
    }
    runs
}
