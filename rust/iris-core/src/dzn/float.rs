/// Appendix B.3's precision search, returning text and conversion attempts.
/// Rust's precision formatting rounds the exact binary64 value to nearest,
/// ties to even; parsing does the same in the opposite direction. Neither uses
/// the locale or the host floating point rounding mode. Each attempt handles
/// at most 17 digits, and there are at most 17 attempts.
pub(super) fn format(value: f64) -> (String, usize) {
    if value == 0.0 {
        return (
            (if value.is_sign_negative() {
                "-0.0"
            } else {
                "0.0"
            })
            .into(),
            0,
        );
    }
    let magnitude = value.abs();
    let mut initial = 1;
    if (1.0..1e17).contains(&magnitude) {
        // These powers of ten are exactly representable as binary64. Counting
        // comparisons avoids log10's rounding error just below a power of ten.
        let mut boundary = 10.0;
        while magnitude >= boundary {
            initial += 1;
            boundary *= 10.0;
        }
    }
    for precision in initial..=17 {
        let rounded = format!("{:.*e}", precision - 1, magnitude);
        if precision == 17 || rounded.parse::<f64>().expect("formatted finite decimal") == magnitude
        {
            return (
                notation(&rounded, precision, value.is_sign_negative()),
                precision - initial + 1,
            );
        }
    }
    unreachable!("precision search always selects its 17-digit decimal")
}

fn notation(rounded: &str, precision: usize, negative: bool) -> String {
    let (mantissa, exponent) = rounded.split_once('e').expect("scientific formatting");
    let exponent: i32 = exponent.parse().expect("formatted exponent");
    let mut digits: String = mantissa.chars().filter(|&c| c != '.').collect();
    while digits.ends_with('0') {
        digits.pop();
    }
    let mut text = String::with_capacity(25);
    if negative {
        text.push('-');
    }
    if exponent < -4 || exponent >= precision as i32 {
        text.push_str(&digits[..1]);
        if digits.len() > 1 {
            text.push('.');
            text.push_str(&digits[1..]);
        }
        text.push_str(&format!("e{exponent:+03}"));
    } else {
        let point = exponent + 1;
        if point <= 0 {
            text.push_str("0.");
            text.extend(std::iter::repeat_n('0', (-point) as usize));
            text.push_str(&digits);
        } else if point as usize >= digits.len() {
            text.push_str(&digits);
            text.extend(std::iter::repeat_n('0', point as usize - digits.len()));
            text.push_str(".0");
        } else {
            text.push_str(&digits[..point as usize]);
            text.push('.');
            text.push_str(&digits[point as usize..]);
        }
    }
    text
}
