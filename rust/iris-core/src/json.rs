use crate::{
    Data, Error,
    value::{LeafStorage, ValueRef},
};
use serde::Serialize;

impl Data {
    pub fn to_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut out = b"{\"iris\":\"0.1\",\"entries\":[".to_vec();
        for (index, entry) in self.entries.iter().enumerate() {
            if index > 0 {
                out.push(b',');
            }
            out.extend_from_slice(b"{\"name\":");
            serde_json::to_writer(&mut out, &entry.name)?;
            out.extend_from_slice(b",\"type\":");
            serde_json::to_writer(&mut out, entry.value.value_type())?;
            out.extend_from_slice(b",\"depth\":");
            serde_json::to_writer(&mut out, &entry.value.depth())?;
            out.extend_from_slice(b",\"value\":");
            write_value(&mut out, entry.value.root())?;
            out.push(b'}');
        }
        out.extend_from_slice(b"]}\n");
        Ok(out)
    }
}
fn write_value(out: &mut Vec<u8>, selected: ValueRef<'_>) -> Result<(), serde_json::Error> {
    if selected.depth() > 0 {
        out.push(b'[');
        for (index, row) in selected.children().expect("array selection").enumerate() {
            if index > 0 {
                out.push(b',');
            }
            write_value(
                out,
                ValueRef {
                    value: selected.value,
                    level: selected.level + 1,
                    row,
                },
            )?;
        }
        out.push(b']');
    } else {
        match &selected.value.leaves {
            LeafStorage::Int(values) => {
                out.extend_from_slice(values[selected.row].to_string().as_bytes())
            }
            LeafStorage::Float(values) => {
                out.extend_from_slice(canonical_float(values[selected.row]).as_bytes())
            }
            LeafStorage::String(values) => serde_json::to_writer(out, &values[selected.row])?,
        }
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
