use iris_core::Data;
use serde::Serialize;

const ENTRY_LIMIT: usize = 64;
const NODE_LIMIT: usize = 1024;
const DEPTH_LIMIT: usize = 16;
const TEXT_LIMIT: usize = 64 << 10;

#[derive(Serialize)]
pub struct Table {
    entries: Vec<Entry>,
    truncated: bool,
}
#[derive(Serialize)]
struct Entry {
    name: String,
    r#type: &'static str,
    depth: usize,
    rows: Vec<Row>,
    truncated: bool,
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Row {
    Array {
        path: Vec<usize>,
        length: usize,
    },
    Scalar {
        path: Vec<usize>,
        text: String,
        clipped: bool,
    },
}

/// Walk only retained nodes. Each stack frame holds a path and next child,
/// so wide omitted arrays do not allocate a list of pending children.
pub fn table(data: &Data) -> Table {
    let mut entries = Vec::new();
    let mut nodes = 0;
    let mut remaining_text = TEXT_LIMIT;
    for source in data.entries().iter().take(ENTRY_LIMIT) {
        if nodes == NODE_LIMIT || remaining_text == 0 {
            break;
        }
        let (name, name_clipped) = clip(source.name(), &mut remaining_text);
        let mut entry = Entry {
            name,
            r#type: source.value().value_type(),
            depth: source.value().depth(),
            rows: Vec::new(),
            truncated: name_clipped,
        };
        let mut path = Vec::new();
        let mut open: Vec<(usize, usize)> = Vec::new();
        loop {
            if nodes == NODE_LIMIT || remaining_text == 0 {
                entry.truncated = true;
                break;
            }
            nodes += 1;
            let value = source.value().select(&path).expect("stored path");
            if value.depth() > 0 {
                let length = value.size().expect("array");
                entry.rows.push(Row::Array {
                    path: path.clone(),
                    length,
                });
                if length > 0 && path.len() < DEPTH_LIMIT {
                    open.push((1, length));
                    path.push(0);
                    continue;
                }
                entry.truncated |= length > 0;
            } else {
                let (text, clipped) = match value.value_type() {
                    "int" => clip(
                        &value.as_int().expect("integer").to_string(),
                        &mut remaining_text,
                    ),
                    "float" => clip(
                        &value.as_float().expect("float").to_string(),
                        &mut remaining_text,
                    ),
                    _ => clip(value.as_str().expect("string"), &mut remaining_text),
                };
                entry.truncated |= clipped;
                entry.rows.push(Row::Scalar {
                    path: path.clone(),
                    text,
                    clipped,
                });
            }
            while let Some((next, length)) = open.last_mut() {
                if *next < *length {
                    *path.last_mut().expect("child path") = *next;
                    *next += 1;
                    break;
                }
                open.pop();
                path.pop();
            }
            if open.is_empty() {
                break;
            }
        }
        entries.push(entry);
    }
    let truncated =
        entries.len() < data.entries().len() || entries.iter().any(|entry| entry.truncated);
    Table { entries, truncated }
}
fn clip(text: &str, remaining: &mut usize) -> (String, bool) {
    let mut end = text.len().min(*remaining);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    *remaining -= end;
    (text[..end].into(), end < text.len())
}
