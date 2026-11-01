//! Complete ASCII decimal conversion, separate from diagnostic policy.
pub(crate) fn parse_integer(bytes: &[u8]) -> Option<i64> {
    let digits = if matches!(bytes.first(), Some(b'+' | b'-')) {
        &bytes[1..]
    } else {
        bytes
    };
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(bytes).ok()?.parse().ok()
}
pub(crate) fn parse_float(bytes: &[u8]) -> Option<f64> {
    let mut cursor = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let mut significand_digits = 0;
    let mut has_nonzero_significand = false;
    while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
        has_nonzero_significand |= bytes[cursor] != b'0';
        significand_digits += 1;
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'.') {
        cursor += 1;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            has_nonzero_significand |= bytes[cursor] != b'0';
            significand_digits += 1;
            cursor += 1;
        }
    }
    if significand_digits == 0 {
        return None;
    }
    if matches!(bytes.get(cursor), Some(b'e' | b'E')) {
        cursor += 1;
        if matches!(bytes.get(cursor), Some(b'+' | b'-')) {
            cursor += 1;
        }
        let start = cursor;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            cursor += 1;
        }
        if start == cursor {
            return None;
        }
    }
    if cursor != bytes.len() {
        return None;
    }
    let value: f64 = std::str::from_utf8(bytes).ok()?.parse().ok()?;
    if !value.is_finite() || value.is_subnormal() || (value == 0.0 && has_nonzero_significand) {
        None
    } else {
        Some(value)
    }
}
pub(crate) fn integer_power(mut base: i64, mut exponent: i64) -> Option<i64> {
    debug_assert!(exponent >= 0);
    let mut result = 1i64;
    while exponent > 0 {
        if exponent & 1 != 0 {
            result = result.checked_mul(base)?;
        }
        exponent >>= 1;
        if exponent > 0 {
            base = base.checked_mul(base)?;
        }
    }
    Some(result)
}
