use crate::{
    Data, Error,
    value::{LeafStorage, ValueRef},
};
use serde::Serialize;
use std::{io::Write, ops::Range};

impl Data {
    pub fn to_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut out = Vec::new();
        self.write_json(&mut out)?;
        Ok(out)
    }

    /// Write canonical JSON, including its final newline, without buffering it.
    pub fn write_json(&self, out: &mut impl Write) -> Result<(), serde_json::Error> {
        out.write_all(b"{\"iris\":\"0.1\",\"entries\":[")
            .map_err(serde_json::Error::io)?;
        for (index, entry) in self.entries.iter().enumerate() {
            if index > 0 {
                out.write_all(b",").map_err(serde_json::Error::io)?;
            }
            out.write_all(b"{\"name\":")
                .map_err(serde_json::Error::io)?;
            serde_json::to_writer(&mut *out, &entry.name)?;
            out.write_all(b",\"type\":")
                .map_err(serde_json::Error::io)?;
            serde_json::to_writer(&mut *out, entry.value.value_type())?;
            out.write_all(b",\"depth\":")
                .map_err(serde_json::Error::io)?;
            serde_json::to_writer(&mut *out, &entry.value.depth())?;
            out.write_all(b",\"value\":")
                .map_err(serde_json::Error::io)?;
            write_value(out, entry.value.root())?;
            out.write_all(b"}").map_err(serde_json::Error::io)?;
        }
        out.write_all(b"]}\n").map_err(serde_json::Error::io)?;
        Ok(())
    }
}
/// Writes `root` with an explicit stack: §5.12 allows any number of
/// dimensions, so recursion could exhaust the call stack.
fn write_value(out: &mut impl Write, root: ValueRef<'_>) -> Result<(), serde_json::Error> {
    // For each open array: the level of its rows, the rows still to write, and
    // whether a row has been written.
    let mut open: Vec<(usize, Range<usize>, bool)> = Vec::new();
    let mut next = Some(root);
    loop {
        if let Some(selected) = next.take() {
            if selected.depth() > 0 {
                out.write_all(b"[").map_err(serde_json::Error::io)?;
                let rows = selected.children().expect("array selection");
                open.push((selected.level + 1, rows, false));
            } else {
                write_leaf(out, selected)?;
            }
        }
        let Some((level, rows, started)) = open.last_mut() else {
            return Ok(());
        };
        match rows.next() {
            Some(row) => {
                if std::mem::replace(started, true) {
                    out.write_all(b",").map_err(serde_json::Error::io)?;
                }
                next = Some(ValueRef {
                    value: root.value,
                    level: *level,
                    row,
                });
            }
            None => {
                out.write_all(b"]").map_err(serde_json::Error::io)?;
                open.pop();
            }
        }
    }
}
fn write_leaf(out: &mut impl Write, selected: ValueRef<'_>) -> Result<(), serde_json::Error> {
    match &selected.value.leaves {
        LeafStorage::Int(values) => out
            .write_all(values[selected.row].to_string().as_bytes())
            .map_err(serde_json::Error::io)?,
        LeafStorage::Float(values) => out
            .write_all(canonical_float(values[selected.row]).as_bytes())
            .map_err(serde_json::Error::io)?,
        LeafStorage::String(values) => serde_json::to_writer(out, &values[selected.row])?,
    }
    Ok(())
}
impl Error {
    pub fn to_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        #[derive(Serialize)]
        struct Document<'a> {
            code: &'static str,
            message: &'a str,
            input_location: &'a Option<crate::Location>,
            spec_location: &'a Option<crate::Location>,
            context: &'a Option<String>,
            record_path: &'a Option<Vec<usize>>,
        }
        serde_json::to_vec(&Document {
            code: self.code.as_str(),
            message: &self.details.message,
            input_location: &self.details.input_location,
            spec_location: &self.details.spec_location,
            context: &self.details.context,
            record_path: &self.details.record_path,
        })
    }
}
/// Ryū supplies shortest round-tripping digits; §3.5 chooses their notation.
fn canonical_float(value: f64) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0"
        } else {
            "0"
        }
        .into();
    }
    let mut buffer = ryu::Buffer::new();
    let shortest = buffer.format_finite(value.abs());
    let (mantissa, exponent) = shortest.split_once('e').map_or((shortest, 0), |(m, e)| {
        (m, e.parse::<i32>().expect("Ryū exponent"))
    });
    let point = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let mut digits: String = mantissa.chars().filter(|&c| c != '.').collect();
    let leading = digits.bytes().take_while(|&b| b == b'0').count();
    digits.drain(..leading);
    while digits.ends_with('0') {
        digits.pop();
    }
    let decimal_point = point + exponent - leading as i32;
    let fixed = if decimal_point <= 0 {
        format!("0.{}{}", "0".repeat((-decimal_point) as usize), digits)
    } else if decimal_point as usize >= digits.len() {
        format!(
            "{}{}",
            digits,
            "0".repeat(decimal_point as usize - digits.len())
        )
    } else {
        format!(
            "{}.{}",
            &digits[..decimal_point as usize],
            &digits[decimal_point as usize..]
        )
    };
    let significand = if digits.len() == 1 {
        digits
    } else {
        format!("{}.{}", &digits[..1], &digits[1..])
    };
    let scientific = format!("{significand}e{}", decimal_point - 1);
    let spelling = if fixed.len() <= scientific.len() {
        fixed
    } else {
        scientific
    };
    if value.is_sign_negative() {
        format!("-{spelling}")
    } else {
        spelling
    }
}
