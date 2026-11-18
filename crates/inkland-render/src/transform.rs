//! The 2D transforms of SVG: read a `transform` attribute and find how
//! much a transform makes a line thinner.

/// An affine transform `[a b c d e f]`, as SVG writes it:
/// `x' = a x + c y + e`, `y' = b x + d y + f`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine(pub [f64; 6]);

impl Affine {
    /// The transform that changes nothing.
    pub const IDENTITY: Affine = Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    /// A move.
    pub fn translate(x: f64, y: f64) -> Affine {
        Affine([1.0, 0.0, 0.0, 1.0, x, y])
    }

    /// A scale.
    pub fn scale(sx: f64, sy: f64) -> Affine {
        Affine([sx, 0.0, 0.0, sy, 0.0, 0.0])
    }

    /// `self` after `inner`: `inner` applies first to a point, then
    /// `self`. In SVG, a parent is `self` and its child is `inner`.
    pub fn then(&self, inner: &Affine) -> Affine {
        let [a, b, c, d, e, f] = self.0;
        let [a2, b2, c2, d2, e2, f2] = inner.0;
        Affine([
            a * a2 + c * b2,
            b * a2 + d * b2,
            a * c2 + c * d2,
            b * c2 + d * d2,
            a * e2 + c * f2 + e,
            b * e2 + d * f2 + f,
        ])
    }

    /// The factor of the thinnest line: a stroke `w` wide becomes at
    /// least `w * thin_factor()` wide. This is the smaller singular
    /// value of the linear part.
    pub fn thin_factor(&self) -> f64 {
        let [a, b, c, d, _, _] = self.0;
        let t = a * a + b * b + c * c + d * d;
        let det = a * d - b * c;
        let root = libm::sqrt((t * t - 4.0 * det * det).max(0.0));
        libm::sqrt(((t - root) / 2.0).max(0.0))
    }

    /// Read the value of a `transform` attribute. A part that cannot be
    /// read is left out.
    pub fn parse(text: &str) -> Affine {
        let mut m = Affine::IDENTITY;
        let mut rest = text;
        while let Some(open) = rest.find('(') {
            let name = rest[..open]
                .trim_matches(|c: char| c.is_whitespace() || c == ',')
                .to_owned();
            let Some(close) = rest[open..].find(')').map(|c| open + c) else {
                break;
            };
            let args: Vec<f64> = rest[open + 1..close]
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse().ok())
                .collect();
            rest = &rest[close + 1..];
            let arg = |i: usize, or: f64| args.get(i).copied().unwrap_or(or);
            let part = match name.as_str() {
                "matrix" if args.len() == 6 => {
                    Affine([args[0], args[1], args[2], args[3], args[4], args[5]])
                }
                "translate" => Affine::translate(arg(0, 0.0), arg(1, 0.0)),
                "scale" => {
                    let sx = arg(0, 1.0);
                    Affine::scale(sx, arg(1, sx))
                }
                "rotate" => {
                    let (s, c) = libm::sincos(arg(0, 0.0).to_radians());
                    let (cx, cy) = (arg(1, 0.0), arg(2, 0.0));
                    Affine::translate(cx, cy)
                        .then(&Affine([c, s, -s, c, 0.0, 0.0]))
                        .then(&Affine::translate(-cx, -cy))
                }
                "skewX" => Affine([1.0, 0.0, libm::tan(arg(0, 0.0).to_radians()), 1.0, 0.0, 0.0]),
                "skewY" => Affine([1.0, libm::tan(arg(0, 0.0).to_radians()), 0.0, 1.0, 0.0, 0.0]),
                _ => Affine::IDENTITY,
            };
            m = m.then(&part);
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn each_kind_of_transform_is_read() {
        assert_eq!(
            Affine::parse("matrix(0.18,0,0,0.18,63.25,73.1)").0,
            [0.18, 0.0, 0.0, 0.18, 63.25, 73.1]
        );
        assert_eq!(
            Affine::parse("translate(5)").0,
            [1.0, 0.0, 0.0, 1.0, 5.0, 0.0]
        );
        assert_eq!(Affine::parse("scale(2)").0, [2.0, 0.0, 0.0, 2.0, 0.0, 0.0]);
        let r = Affine::parse("rotate(90 10 0)").0;
        let want = [0.0, 1.0, -1.0, 0.0, 10.0, -10.0];
        assert!(r.iter().zip(want).all(|(a, b)| close(*a, b)), "{r:?}");
        assert_eq!(Affine::parse("").0, Affine::IDENTITY.0);
        assert_eq!(Affine::parse("nothing(3)").0, Affine::IDENTITY.0);
    }

    #[test]
    fn a_list_applies_from_the_right() {
        // The scale applies first, then the move.
        let m = Affine::parse("translate(10 0) scale(2)");
        assert_eq!(m.0, [2.0, 0.0, 0.0, 2.0, 10.0, 0.0]);
        let m = Affine::parse("scale(2),translate(10 0)");
        assert_eq!(m.0, [2.0, 0.0, 0.0, 2.0, 20.0, 0.0]);
    }

    #[test]
    fn the_thin_factor_is_the_smaller_scale() {
        assert!(close(Affine::IDENTITY.thin_factor(), 1.0));
        assert!(close(Affine::parse("scale(-1 1)").thin_factor(), 1.0));
        assert!(close(Affine::parse("scale(3 0.25)").thin_factor(), 0.25));
        assert!(close(
            Affine::parse("rotate(30) scale(0.5 4)").thin_factor(),
            0.5
        ));
        assert!(close(
            Affine::parse("scale(0.5) translate(9 9) scale(0.5)").thin_factor(),
            0.25
        ));
    }
}
