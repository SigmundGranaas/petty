//! XPath 3.1 Math Functions
//!
//! Implements the math namespace functions from the XPath 3.1 specification.
//! All trigonometric functions work in radians.

use crate::error::XPath31Error;
use crate::types::*;
use std::f64::consts::PI;

/// math:pi() - Returns the value of pi.
///
/// Returns an approximation to the mathematical constant π.
pub fn math_pi<N: Clone>(_args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    Ok(XdmValue::from_double(PI))
}

/// math:exp($arg as xs:double?) as xs:double?
///
/// Returns the value of e^arg.
pub fn math_exp<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("math:exp", "Expected 1 argument"));
    }

    let arg = args.remove(0);
    if arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    let n = arg.to_double();
    Ok(XdmValue::from_double(n.exp()))
}

/// math:exp10($arg as xs:double?) as xs:double?
///
/// Returns the value of 10^arg.
pub fn math_exp10<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("math:exp10", "Expected 1 argument"));
    }

    let arg = args.remove(0);
    if arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    let n = arg.to_double();
    Ok(XdmValue::from_double(10_f64.powf(n)))
}

/// math:log($arg as xs:double?) as xs:double?
///
/// Returns the natural logarithm of the argument.
pub fn math_log<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("math:log", "Expected 1 argument"));
    }

    let arg = args.remove(0);
    if arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    let n = arg.to_double();
    Ok(XdmValue::from_double(n.ln()))
}

/// math:log10($arg as xs:double?) as xs:double?
///
/// Returns the base-10 logarithm of the argument.
pub fn math_log10<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("math:log10", "Expected 1 argument"));
    }

    let arg = args.remove(0);
    if arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    let n = arg.to_double();
    Ok(XdmValue::from_double(n.log10()))
}

/// math:pow($x as xs:double?, $y as xs:numeric) as xs:double?
///
/// Returns the value of x raised to the power y.
pub fn math_pow<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function("math:pow", "Expected 2 arguments"));
    }

    let y = args.remove(1).to_double();
    let x_arg = args.remove(0);

    if x_arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    let x = x_arg.to_double();

    // Special cases per XPath 3.1 spec
    if y == 0.0 {
        return Ok(XdmValue::from_double(1.0));
    }
    if x == 1.0 {
        return Ok(XdmValue::from_double(1.0));
    }
    if x == -1.0 && (y.is_infinite()) {
        return Ok(XdmValue::from_double(1.0));
    }

    Ok(XdmValue::from_double(x.powf(y)))
}

/// math:sqrt($arg as xs:double?) as xs:double?
///
/// Returns the non-negative square root of the argument.
pub fn math_sqrt<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("math:sqrt", "Expected 1 argument"));
    }

    let arg = args.remove(0);
    if arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    let n = arg.to_double();
    Ok(XdmValue::from_double(n.sqrt()))
}

/// math:sin($theta as xs:double?) as xs:double?
///
/// Returns the sine of the argument in radians.
pub fn math_sin<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("math:sin", "Expected 1 argument"));
    }

    let arg = args.remove(0);
    if arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    let theta = arg.to_double();
    Ok(XdmValue::from_double(theta.sin()))
}

/// math:cos($theta as xs:double?) as xs:double?
///
/// Returns the cosine of the argument in radians.
pub fn math_cos<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("math:cos", "Expected 1 argument"));
    }

    let arg = args.remove(0);
    if arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    let theta = arg.to_double();
    Ok(XdmValue::from_double(theta.cos()))
}

/// math:tan($theta as xs:double?) as xs:double?
///
/// Returns the tangent of the argument in radians.
pub fn math_tan<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("math:tan", "Expected 1 argument"));
    }

    let arg = args.remove(0);
    if arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    let theta = arg.to_double();
    Ok(XdmValue::from_double(theta.tan()))
}

/// math:asin($arg as xs:double?) as xs:double?
///
/// Returns the arc sine of the argument in radians.
pub fn math_asin<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("math:asin", "Expected 1 argument"));
    }

    let arg = args.remove(0);
    if arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    let n = arg.to_double();
    Ok(XdmValue::from_double(n.asin()))
}

/// math:acos($arg as xs:double?) as xs:double?
///
/// Returns the arc cosine of the argument in radians.
pub fn math_acos<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("math:acos", "Expected 1 argument"));
    }

    let arg = args.remove(0);
    if arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    let n = arg.to_double();
    Ok(XdmValue::from_double(n.acos()))
}

/// math:atan($arg as xs:double?) as xs:double?
///
/// Returns the arc tangent of the argument in radians.
pub fn math_atan<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("math:atan", "Expected 1 argument"));
    }

    let arg = args.remove(0);
    if arg.is_empty() {
        return Ok(XdmValue::empty());
    }

    let n = arg.to_double();
    Ok(XdmValue::from_double(n.atan()))
}

/// math:atan2($y as xs:double, $x as xs:double) as xs:double
///
/// Returns the angle in radians subtended at the origin by the point
/// on a plane with coordinates (x, y) and the positive x-axis.
pub fn math_atan2<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 2 {
        return Err(XPath31Error::function("math:atan2", "Expected 2 arguments"));
    }

    let x = args.remove(1).to_double();
    let y = args.remove(0).to_double();

    Ok(XdmValue::from_double(y.atan2(x)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{E, FRAC_PI_2, FRAC_PI_4};

    const EPSILON: f64 = 1e-10;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPSILON || (a.is_nan() && b.is_nan())
    }

    #[test]
    fn test_math_pi() {
        let result: XdmValue<()> = math_pi(vec![]).unwrap();
        assert!(approx_eq(result.to_double(), PI));
    }

    #[test]
    fn test_math_exp() {
        let result: XdmValue<()> = math_exp(vec![XdmValue::from_double(0.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 1.0));

        let result: XdmValue<()> = math_exp(vec![XdmValue::from_double(1.0)]).unwrap();
        assert!(approx_eq(result.to_double(), E));

        // Empty sequence returns empty
        let result: XdmValue<()> = math_exp(vec![XdmValue::empty()]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_math_exp10() {
        let result: XdmValue<()> = math_exp10(vec![XdmValue::from_double(0.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 1.0));

        let result: XdmValue<()> = math_exp10(vec![XdmValue::from_double(2.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 100.0));
    }

    #[test]
    fn test_math_log() {
        let result: XdmValue<()> = math_log(vec![XdmValue::from_double(1.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 0.0));

        let result: XdmValue<()> = math_log(vec![XdmValue::from_double(E)]).unwrap();
        assert!(approx_eq(result.to_double(), 1.0));

        // log(0) = -infinity
        let result: XdmValue<()> = math_log(vec![XdmValue::from_double(0.0)]).unwrap();
        assert!(result.to_double().is_infinite() && result.to_double() < 0.0);

        // log(-1) = NaN
        let result: XdmValue<()> = math_log(vec![XdmValue::from_double(-1.0)]).unwrap();
        assert!(result.to_double().is_nan());
    }

    #[test]
    fn test_math_log10() {
        let result: XdmValue<()> = math_log10(vec![XdmValue::from_double(1.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 0.0));

        let result: XdmValue<()> = math_log10(vec![XdmValue::from_double(100.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 2.0));
    }

    #[test]
    fn test_math_pow() {
        let result: XdmValue<()> =
            math_pow(vec![XdmValue::from_double(2.0), XdmValue::from_double(3.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 8.0));

        // x^0 = 1
        let result: XdmValue<()> =
            math_pow(vec![XdmValue::from_double(5.0), XdmValue::from_double(0.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 1.0));

        // 1^y = 1
        let result: XdmValue<()> = math_pow(vec![
            XdmValue::from_double(1.0),
            XdmValue::from_double(100.0),
        ])
        .unwrap();
        assert!(approx_eq(result.to_double(), 1.0));

        // (-1)^INF = 1
        let result: XdmValue<()> = math_pow(vec![
            XdmValue::from_double(-1.0),
            XdmValue::from_double(f64::INFINITY),
        ])
        .unwrap();
        assert!(approx_eq(result.to_double(), 1.0));

        // Empty sequence returns empty
        let result: XdmValue<()> =
            math_pow(vec![XdmValue::empty(), XdmValue::from_double(2.0)]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_math_sqrt() {
        let result: XdmValue<()> = math_sqrt(vec![XdmValue::from_double(4.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 2.0));

        let result: XdmValue<()> = math_sqrt(vec![XdmValue::from_double(0.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 0.0));

        // sqrt(-1) = NaN
        let result: XdmValue<()> = math_sqrt(vec![XdmValue::from_double(-1.0)]).unwrap();
        assert!(result.to_double().is_nan());
    }

    #[test]
    fn test_math_sin() {
        let result: XdmValue<()> = math_sin(vec![XdmValue::from_double(0.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 0.0));

        let result: XdmValue<()> = math_sin(vec![XdmValue::from_double(FRAC_PI_2)]).unwrap();
        assert!(approx_eq(result.to_double(), 1.0));
    }

    #[test]
    fn test_math_cos() {
        let result: XdmValue<()> = math_cos(vec![XdmValue::from_double(0.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 1.0));

        let result: XdmValue<()> = math_cos(vec![XdmValue::from_double(PI)]).unwrap();
        assert!(approx_eq(result.to_double(), -1.0));
    }

    #[test]
    fn test_math_tan() {
        let result: XdmValue<()> = math_tan(vec![XdmValue::from_double(0.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 0.0));

        let result: XdmValue<()> = math_tan(vec![XdmValue::from_double(FRAC_PI_4)]).unwrap();
        assert!(approx_eq(result.to_double(), 1.0));
    }

    #[test]
    fn test_math_asin() {
        let result: XdmValue<()> = math_asin(vec![XdmValue::from_double(0.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 0.0));

        let result: XdmValue<()> = math_asin(vec![XdmValue::from_double(1.0)]).unwrap();
        assert!(approx_eq(result.to_double(), FRAC_PI_2));

        // asin(2) = NaN (out of domain)
        let result: XdmValue<()> = math_asin(vec![XdmValue::from_double(2.0)]).unwrap();
        assert!(result.to_double().is_nan());
    }

    #[test]
    fn test_math_acos() {
        let result: XdmValue<()> = math_acos(vec![XdmValue::from_double(1.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 0.0));

        let result: XdmValue<()> = math_acos(vec![XdmValue::from_double(0.0)]).unwrap();
        assert!(approx_eq(result.to_double(), FRAC_PI_2));
    }

    #[test]
    fn test_math_atan() {
        let result: XdmValue<()> = math_atan(vec![XdmValue::from_double(0.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 0.0));

        let result: XdmValue<()> = math_atan(vec![XdmValue::from_double(1.0)]).unwrap();
        assert!(approx_eq(result.to_double(), FRAC_PI_4));
    }

    #[test]
    fn test_math_atan2() {
        // atan2(0, 1) = 0
        let result: XdmValue<()> =
            math_atan2(vec![XdmValue::from_double(0.0), XdmValue::from_double(1.0)]).unwrap();
        assert!(approx_eq(result.to_double(), 0.0));

        // atan2(1, 0) = pi/2
        let result: XdmValue<()> =
            math_atan2(vec![XdmValue::from_double(1.0), XdmValue::from_double(0.0)]).unwrap();
        assert!(approx_eq(result.to_double(), FRAC_PI_2));

        // atan2(1, 1) = pi/4
        let result: XdmValue<()> =
            math_atan2(vec![XdmValue::from_double(1.0), XdmValue::from_double(1.0)]).unwrap();
        assert!(approx_eq(result.to_double(), FRAC_PI_4));

        // atan2(-1, 0) = -pi/2
        let result: XdmValue<()> = math_atan2(vec![
            XdmValue::from_double(-1.0),
            XdmValue::from_double(0.0),
        ])
        .unwrap();
        assert!(approx_eq(result.to_double(), -FRAC_PI_2));
    }
}
