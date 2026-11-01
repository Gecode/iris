use iris_core::{Data, Value};
use std::io::{self, Write};

pub(super) fn write_summary(
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
        write_shape(out, entry.value())?;
        writeln!(out)?;
    }
    Ok(())
}

fn write_shape(out: &mut impl Write, value: &Value) -> io::Result<()> {
    let root = value.select(&[]).expect("whole entry");
    write!(out, "{} ", value.value_type())?;
    if value.depth() == 0 {
        return match value.value_type() {
            "int" => write!(out, "{}", root.as_int().expect("integer scalar")),
            "float" => write!(out, "{}", root.as_float().expect("float scalar")),
            "string" => write!(out, "{:?}", root.as_str().expect("string scalar")),
            _ => unreachable!("data has only int, float, and string leaves"),
        };
    }
    let rows = root.size().expect("array");
    write!(out, "array {rows}")?;
    if value.depth() == 1 {
        return Ok(());
    }
    let mut shortest = usize::MAX;
    let mut longest = 0;
    for index in 0..rows {
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
    for _ in 2..value.depth() {
        write!(out, " x ...")?;
    }
    Ok(())
}
