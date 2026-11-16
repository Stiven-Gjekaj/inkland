//! Numbers in output files.
//!
//! Each writer rounds coordinates to a fixed number of decimals, so float
//! noise from the last bits never gets into a file.

/// Round to `decimals` decimal places. `-0` becomes `0`.
pub fn round(v: f64, decimals: u32) -> f64 {
    let f = 10f64.powi(decimals as i32);
    let r = (v * f).round() / f;
    if r == 0.0 { 0.0 } else { r }
}

/// Text of `v` rounded to `decimals` places, without trailing zeros.
/// For example `fixed(167.0, 3)` is `"167"` and `fixed(0.17639, 3)` is
/// `"0.176"`.
pub fn fixed(v: f64, decimals: u32) -> String {
    let mut s = format!("{:.*}", decimals as usize, round(v, decimals));
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" { "0".to_owned() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_trims_and_rounds() {
        assert_eq!(fixed(167.0, 3), "167");
        assert_eq!(fixed(0.176_39, 3), "0.176");
        assert_eq!(fixed(1.0005, 3), "1.001");
        assert_eq!(fixed(-0.0001, 3), "0");
        assert_eq!(fixed(-2.5, 3), "-2.5");
        assert_eq!(fixed(0.1 + 0.2, 3), "0.3");
    }

    #[test]
    fn round_drops_negative_zero() {
        assert_eq!(round(-0.0001, 3).to_bits(), 0.0f64.to_bits());
    }
}
