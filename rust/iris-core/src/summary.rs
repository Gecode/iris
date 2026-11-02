use crate::{Data, Value};
use std::io::{self, Write};

pub fn write_summary(
    out: &mut impl Write,
    path: &str,
    format: Option<&str>,
    data: &Data,
) -> io::Result<()> {
    write!(out, "{path}")?;
    if let Some(name) = format {
        write!(out, " [{name}]")?;
    }
    writeln!(out)?;
    for entry in data.entries() {
        write!(out, "  {}: ", entry.name())?;
        write_shape(out, entry.value(), &mut Limits::unlimited())?;
        writeln!(out)?;
    }
    Ok(())
}

fn write_shape(out: &mut impl Write, value: &Value, limits: &mut Limits) -> io::Result<()> {
    let root = value.select(&[]).expect("whole entry");
    write!(out, "{} ", value.value_type())?;
    if value.depth() == 0 {
        return match value.value_type() {
            "int" => write!(out, "{}", root.as_int().expect("integer scalar")),
            "float" => write!(out, "{}", root.as_float().expect("float scalar")),
            "string" => write!(
                out,
                "{:?}",
                limits.text(root.as_str().expect("string scalar"))
            ),
            _ => unreachable!("data has only int, float, and string leaves"),
        };
    }
    let rows = root.size().expect("array");
    write!(out, "array {rows}")?;
    if value.depth() == 1 {
        return Ok(());
    }
    if limits.rows < rows {
        limits.rows = 0;
        write!(out, " x ... (shape truncated)")?;
        return Ok(());
    }
    limits.rows -= rows;
    let mut shortest = usize::MAX;
    let mut longest = 0;
    for index in 0..rows {
        limits.visit();
        let length = value
            .select(&[index])
            .expect("existing row")
            .size()
            .expect("second array level");
        shortest = shortest.min(length);
        longest = longest.max(length);
    }
    if rows == 0 {
        shortest = 0;
    }
    write!(out, " x {shortest}")?;
    if longest != shortest {
        write!(out, "..{longest}")?;
    }
    // Like gecode-instance, deeper dimensions are elided. No subtree is copied
    // or traversed; work is O(entries + second-level rows + output bytes).
    for _ in 2..value.depth().min(limits.dimensions) {
        limits.visit();
        write!(out, " x ...")?;
    }
    if value.depth() > limits.dimensions {
        write!(
            out,
            " x ... ({} dimensions omitted)",
            value.depth() - limits.dimensions
        )?;
    }
    Ok(())
}

/// Fixed editor limits bound both inspection and intermediate output.
pub const PREVIEW_ENTRIES: usize = 64;
pub const PREVIEW_ROWS: usize = 1024;
pub const PREVIEW_DIMENSIONS: usize = 16;
pub const PREVIEW_SCALARS: usize = 160;
pub const PREVIEW_BYTES: usize = 16 * 1024;

struct Limits {
    rows: usize,
    dimensions: usize,
    bounded: bool,
    #[cfg(test)]
    work: usize,
}
impl Limits {
    #[inline]
    fn visit(&mut self) {
        #[cfg(test)]
        {
            self.work += 1;
        }
    }
    fn unlimited() -> Self {
        Self {
            rows: usize::MAX,
            dimensions: usize::MAX,
            bounded: false,
            #[cfg(test)]
            work: 0,
        }
    }
    fn text<'a>(&self, text: &'a str) -> std::borrow::Cow<'a, str> {
        if self.bounded {
            std::borrow::Cow::Owned(short_text(text))
        } else {
            std::borrow::Cow::Borrowed(text)
        }
    }
}
/// Limit before quoting/escaping so huge names and strings never enter a buffer.
pub fn short_text(text: &str) -> String {
    let mut chars = text.chars();
    let mut out: String = chars.by_ref().take(PREVIEW_SCALARS).collect();
    if chars.next().is_some() {
        out.pop();
        out.push('…');
    }
    out
}

/// Render a bounded summary for editor caches; reading itself is never truncated.
pub fn preview(label: &str, format: Option<&str>, data: &Data) -> String {
    preview_counted(label, format, data, &mut 0)
}
fn preview_counted(label: &str, format: Option<&str>, data: &Data, _work: &mut usize) -> String {
    let mut out = Vec::new();
    write!(out, "{}", short_text(label).escape_debug()).expect("Vec write");
    if let Some(format) = format {
        write!(out, " [{}]", short_text(format).escape_debug()).expect("Vec write");
    }
    writeln!(out).expect("Vec write");
    let mut limits = Limits {
        rows: PREVIEW_ROWS,
        dimensions: PREVIEW_DIMENSIONS,
        bounded: true,
        #[cfg(test)]
        work: 0,
    };
    for entry in data.entries().iter().take(PREVIEW_ENTRIES) {
        limits.visit();
        write!(out, "  {}: ", short_text(entry.name()).escape_debug()).expect("Vec write");
        write_shape(&mut out, entry.value(), &mut limits).expect("Vec write");
        writeln!(out).expect("Vec write");
        if out.len() >= PREVIEW_BYTES {
            break;
        }
    }
    if data.entries().len() > PREVIEW_ENTRIES {
        writeln!(
            out,
            "  … ({} entries omitted)",
            data.entries().len() - PREVIEW_ENTRIES
        )
        .expect("Vec write");
    }
    #[cfg(test)]
    {
        _work.clone_from(&limits.work);
    }
    let mut text = String::from_utf8(out).expect("summary is UTF-8");
    if text.len() > PREVIEW_BYTES {
        let mut end = PREVIEW_BYTES - "\n… (preview truncated)".len();
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str("\n… (preview truncated)");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Entry,
        value::{ArrayLevel, LeafStorage},
    };
    #[test]
    fn preview_bounds_output_shape_work_and_unicode_scalars() {
        let make = |n: usize| Data {
            entries: (0..n)
                .map(|i| Entry {
                    name: format!("x{i}"),
                    value: Value {
                        leaves: LeafStorage::Int(vec![]),
                        levels: vec![
                            ArrayLevel {
                                offsets: vec![0, 700],
                            },
                            ArrayLevel {
                                offsets: vec![0; 701],
                            },
                        ],
                    },
                })
                .collect(),
        };
        let mut small = 0;
        let text = preview_counted("sample", None, &make(128), &mut small);
        let mut large = 0;
        let larger = preview_counted("sample", None, &make(512), &mut large);
        assert!(large <= 6 * small);
        assert_eq!(small, 764);
        assert_eq!(large, small);
        assert!(text.contains("x0: int array 700 x 0"));
        assert!(text.contains("x1: int array 700 x ... (shape truncated)"));
        assert!(larger.contains("448 entries omitted"));
        let data = Data {
            entries: (0..100)
                .map(|_| Entry {
                    name: "é".repeat(10000),
                    value: Value::string("😀\n".repeat(10000)),
                })
                .collect(),
        };
        let preview = preview("\nlabel", Some("format"), &data);
        assert!(preview.len() <= PREVIEW_BYTES);
        assert!(preview.contains("preview truncated"));
        assert!(preview.starts_with("\\nlabel"));
        assert!(!preview.contains("😀\n"));
        assert_eq!(
            short_text(&"😀".repeat(10000)).chars().count(),
            PREVIEW_SCALARS
        );
        let deep = Data {
            entries: vec![Entry {
                name: "deep".into(),
                value: Value {
                    leaves: LeafStorage::Int(vec![]),
                    levels: (0..100000)
                        .map(|_| ArrayLevel {
                            offsets: vec![0, 0],
                        })
                        .collect(),
                },
            }],
        };
        let mut work = 0;
        let text = preview_counted("sample", None, &deep, &mut work);
        assert!(text.contains("99984 dimensions omitted"));
        assert!(work <= PREVIEW_DIMENSIONS);
    }
}
