use crate::{Data, Value, value::LeafStorage};
use std::{
    io::{self, Write},
    ops::Range,
};

mod float;
#[cfg(test)]
mod tests;

impl Data {
    /// Render the exact dzn bytes defined by Specification appendix B.
    ///
    /// Entries retain definition order. Traversal is iterative and linear in
    /// stored data and output size, with O(depth) temporary storage. Rendering
    /// performs no execution and consumes no language work budget.
    /// I/O errors propagate; the destination may contain a partial document.
    /// Buffer the destination when partial output must not be published.
    pub fn write_dzn(&self, out: &mut impl Write) -> io::Result<()> {
        Writer::new(out).write_data(self)
    }

    /// Render dzn into an owned byte vector, like [`Data::to_json`].
    pub fn to_dzn(&self) -> io::Result<Vec<u8>> {
        let mut out = Vec::new();
        self.write_dzn(&mut out)?;
        Ok(out)
    }
}

struct Writer<'a, W> {
    out: &'a mut W,
    #[cfg(test)]
    work: usize,
}
impl<'a, W: Write> Writer<'a, W> {
    fn new(out: &'a mut W) -> Self {
        Self {
            out,
            #[cfg(test)]
            work: 0,
        }
    }
    fn write_data(&mut self, data: &Data) -> io::Result<()> {
        for entry in &data.entries {
            let value = &entry.value;
            let shape = self.rectangular_shape(value);
            if shape.is_none() {
                self.write_type_comment(&entry.name, value)?;
            }
            self.write_name(&entry.name)?;
            self.write_bytes(b" = ")?;
            match shape {
                Some(shape) => self.write_rectangular(value, &shape)?,
                None => self.write_nested(value)?,
            }
            self.write_bytes(b";\n")?;
        }
        Ok(())
    }
    fn count_work(&mut self, _units: usize) {
        #[cfg(test)]
        {
            self.work += _units;
        }
    }
    fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.count_work(bytes.len());
        self.out.write_all(bytes)
    }
    fn write_name(&mut self, name: &str) -> io::Result<()> {
        let plain = name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            && !KEYWORDS.contains(&name);
        if !plain {
            self.write_bytes(b"'")?;
        }
        self.write_bytes(name.as_bytes())?;
        if !plain {
            self.write_bytes(b"'")?;
        }
        Ok(())
    }
    // At most six levels need shape inspection. All rows at a level share one
    // offset table, so this includes rows under different parents.
    fn rectangular_shape(&mut self, value: &Value) -> Option<Vec<usize>> {
        if value.depth() > 6 {
            return None;
        }
        let mut shape = Vec::with_capacity(value.depth());
        for level in &value.levels {
            self.count_work(1);
            let extent = level.offsets.get(1).copied().unwrap_or(0);
            for row in level.offsets.windows(2) {
                self.count_work(1);
                if row[1] - row[0] != extent {
                    return None;
                }
            }
            shape.push(extent);
        }
        Some(shape)
    }
    fn write_type_comment(&mut self, name: &str, value: &Value) -> io::Result<()> {
        self.write_bytes(b"% ")?;
        self.write_bytes(name.as_bytes())?;
        self.write_bytes(b": ")?;
        for _ in 1..value.depth() {
            self.write_bytes(b"array[int] of tuple(")?;
        }
        self.write_bytes(b"array[int] of ")?;
        self.write_bytes(value.value_type().as_bytes())?;
        for _ in 1..value.depth() {
            self.write_bytes(b")")?;
        }
        self.write_bytes(b"\n")
    }
    fn write_rectangular(&mut self, value: &Value, shape: &[usize]) -> io::Result<()> {
        match shape {
            [] => self.write_leaf(value, 0),
            [_] => self.write_leaves(value),
            [rows, columns] if *rows > 0 && *columns > 0 => {
                self.write_bytes(b"[| ")?;
                for index in 0..value.leaf_count() {
                    if index > 0 {
                        self.write_bytes(if index % columns == 0 {
                            b"\n  | "
                        } else {
                            b", "
                        })?;
                    }
                    self.write_leaf(value, index)?;
                }
                self.write_bytes(b" |]")
            }
            _ => {
                self.write_bytes(format!("array{}d(", shape.len()).as_bytes())?;
                for extent in shape {
                    self.write_bytes(format!("1..{extent}, ").as_bytes())?;
                }
                self.write_leaves(value)?;
                self.write_bytes(b")")
            }
        }
    }
    fn write_leaves(&mut self, value: &Value) -> io::Result<()> {
        self.write_bytes(b"[")?;
        for index in 0..value.leaf_count() {
            if index > 0 {
                self.write_bytes(b", ")?;
            }
            self.write_leaf(value, index)?;
        }
        self.write_bytes(b"]")
    }
    fn write_nested(&mut self, value: &Value) -> io::Result<()> {
        struct Array {
            rows: Range<usize>,
            started: bool,
        }
        self.write_bytes(b"[")?;
        let mut open = vec![Array {
            rows: 0..value.levels[0].offsets[1],
            started: false,
        }];
        while let Some(array) = open.last_mut() {
            self.count_work(1);
            if let Some(row) = array.rows.next() {
                if std::mem::replace(&mut array.started, true) {
                    self.write_bytes(b", ")?;
                }
                let level = open.len();
                if level == value.depth() {
                    self.write_leaf(value, row)?;
                } else {
                    self.write_bytes(b"([")?;
                    let offsets = &value.levels[level].offsets;
                    open.push(Array {
                        rows: offsets[row]..offsets[row + 1],
                        started: false,
                    });
                }
            } else {
                open.pop();
                self.write_bytes(if open.is_empty() { b"]" } else { b"],)" })?;
            }
        }
        Ok(())
    }
    fn write_leaf(&mut self, value: &Value, index: usize) -> io::Result<()> {
        self.count_work(1);
        match &value.leaves {
            LeafStorage::Int(values) => self.write_bytes(values[index].to_string().as_bytes()),
            LeafStorage::Float(values) => {
                let (text, attempts) = float::format(values[index]);
                self.count_work(attempts);
                self.write_bytes(text.as_bytes())
            }
            LeafStorage::String(values) => self.write_string(&values[index]),
        }
    }
    fn write_string(&mut self, text: &str) -> io::Result<()> {
        self.write_bytes(b"\"")?;
        for byte in text.bytes() {
            self.count_work(1);
            match byte {
                b'"' => self.write_bytes(b"\\\"")?,
                b'\\' => self.write_bytes(b"\\\\")?,
                b'\n' => self.write_bytes(b"\\n")?,
                b'\t' => self.write_bytes(b"\\t")?,
                0..=31 => {
                    const HEX: &[u8; 16] = b"0123456789abcdef";
                    self.write_bytes(&[
                        b'\\',
                        b'x',
                        HEX[(byte / 16) as usize],
                        HEX[(byte % 16) as usize],
                    ])?;
                }
                _ => self.write_bytes(&[byte])?,
            }
        }
        self.write_bytes(b"\"")
    }
}

// Frozen by appendix B.2, independently of installed MiniZinc versions.
const KEYWORDS: &[&str] = &[
    "ann",
    "annotation",
    "any",
    "array",
    "bool",
    "case",
    "constraint",
    "default",
    "diff",
    "div",
    "else",
    "elseif",
    "endif",
    "enum",
    "false",
    "float",
    "function",
    "if",
    "in",
    "include",
    "int",
    "intersect",
    "let",
    "list",
    "maximize",
    "minimize",
    "mod",
    "not",
    "of",
    "op",
    "opt",
    "output",
    "par",
    "predicate",
    "record",
    "satisfy",
    "set",
    "solve",
    "string",
    "subset",
    "superset",
    "symdiff",
    "test",
    "then",
    "true",
    "tuple",
    "type",
    "union",
    "var",
    "where",
    "xor",
];
