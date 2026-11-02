//! Complete ASCII decimal conversion, separate from diagnostic policy.
pub(crate) fn parse_integer(bytes: &[u8]) -> Option<i64> {
    parse_integer_controlled(bytes, &crate::cancellation::Control::default())
        .expect("uncancelled conversion")
}
#[inline]
pub(crate) fn parse_integer_controlled(
    bytes: &[u8],
    control: &crate::cancellation::Control,
) -> Result<Option<i64>, crate::cancellation::Cancelled> {
    if control.is_active() {
        parse_integer_with::<true>(bytes, control)
    } else {
        parse_integer_with::<false>(bytes, control)
    }
}
#[inline]
fn parse_integer_with<const CANCEL: bool>(
    bytes: &[u8],
    control: &crate::cancellation::Control,
) -> Result<Option<i64>, crate::cancellation::Cancelled> {
    let digits = if matches!(bytes.first(), Some(b'+' | b'-')) {
        &bytes[1..]
    } else {
        bytes
    };
    if digits.is_empty() {
        return Ok(None);
    }
    for chunk in digits.chunks(if CANCEL {
        crate::cancellation::SCAN_CHUNK
    } else {
        usize::MAX
    }) {
        if CANCEL {
            control.check()?;
        }
        if !chunk.iter().all(u8::is_ascii_digit) {
            return Ok(None);
        }
    }
    if CANCEL {
        control.check()?;
    }
    if CANCEL {
        // Integer conversion itself can scan arbitrarily many leading zeroes.
        // Keep the std parser on the ordinary path, and use the same checked
        // decimal arithmetic in bounded chunks on the cancellable path.
        let negative = bytes.first() == Some(&b'-');
        let mut value = 0i64;
        for chunk in digits.chunks(crate::cancellation::SCAN_CHUNK) {
            control.check()?;
            for &digit in chunk {
                let Some(shifted) = value.checked_mul(10) else {
                    return Ok(None);
                };
                let next = if negative {
                    shifted.checked_sub(i64::from(digit - b'0'))
                } else {
                    shifted.checked_add(i64::from(digit - b'0'))
                };
                let Some(next) = next else {
                    return Ok(None);
                };
                value = next;
            }
        }
        control.check()?;
        return Ok(Some(value));
    }
    let value = std::str::from_utf8(bytes).ok().and_then(|s| s.parse().ok());
    if CANCEL {
        control.check()?;
    }
    Ok(value)
}
#[inline]
pub(crate) fn parse_float_controlled(
    bytes: &[u8],
    control: &crate::cancellation::Control,
) -> Result<Option<f64>, crate::cancellation::Cancelled> {
    if control.is_active() {
        parse_float_with::<true>(bytes, control)
    } else {
        parse_float_with::<false>(bytes, control)
    }
}
#[inline]
fn parse_float_with<const CANCEL: bool>(
    bytes: &[u8],
    control: &crate::cancellation::Control,
) -> Result<Option<f64>, crate::cancellation::Cancelled> {
    let mut cursor = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let mut significand_digits = 0;
    let mut has_nonzero_significand = false;
    while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
        has_nonzero_significand |= bytes[cursor] != b'0';
        significand_digits += 1;
        if CANCEL && cursor.is_multiple_of(crate::cancellation::SCAN_CHUNK) {
            control.check()?;
        }
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'.') {
        if CANCEL && cursor.is_multiple_of(crate::cancellation::SCAN_CHUNK) {
            control.check()?;
        }
        cursor += 1;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            has_nonzero_significand |= bytes[cursor] != b'0';
            significand_digits += 1;
            if CANCEL && cursor.is_multiple_of(crate::cancellation::SCAN_CHUNK) {
                control.check()?;
            }
            cursor += 1;
        }
    }
    if significand_digits == 0 {
        return Ok(None);
    }
    if matches!(bytes.get(cursor), Some(b'e' | b'E')) {
        if CANCEL && cursor.is_multiple_of(crate::cancellation::SCAN_CHUNK) {
            control.check()?;
        }
        cursor += 1;
        if matches!(bytes.get(cursor), Some(b'+' | b'-')) {
            if CANCEL && cursor.is_multiple_of(crate::cancellation::SCAN_CHUNK) {
                control.check()?;
            }
            cursor += 1;
        }
        let start = cursor;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            if CANCEL && cursor.is_multiple_of(crate::cancellation::SCAN_CHUNK) {
                control.check()?;
            }
            cursor += 1;
        }
        if start == cursor {
            return Ok(None);
        }
    }
    if cursor != bytes.len() {
        return Ok(None);
    }
    if CANCEL {
        control.check()?;
    }
    let Some(value) = std::str::from_utf8(bytes)
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
    else {
        return Ok(None);
    };
    if CANCEL {
        control.check()?;
    }
    if !value.is_finite() || value.is_subnormal() || (value == 0.0 && has_nonzero_significand) {
        Ok(None)
    } else {
        Ok(Some(value))
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

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    use crate::{CancellationToken, ReadOptions, cancellation::Control};
    #[test]
    fn cancellable_decimal_conversion_preserves_std_boundaries() {
        let control = Control::new(&ReadOptions {
            cancellation: Some(CancellationToken::default()),
        });
        for text in [
            "0",
            "+0",
            "-0",
            "9223372036854775807",
            "-9223372036854775808",
            "9223372036854775808",
            "-9223372036854775809",
            "",
            "+",
            "-",
            "1x",
            "1.0",
            "01",
            "+001",
        ] {
            assert_eq!(
                parse_integer_controlled(text.as_bytes(), &control).unwrap(),
                parse_integer(text.as_bytes()),
                "{text}"
            );
        }
        let long = format!("-{}1", "0".repeat(200000));
        assert_eq!(
            parse_integer_controlled(long.as_bytes(), &control).unwrap(),
            Some(-1)
        );
    }
}
