use crate::error::XPath31Error;
use crate::types::XdmValue;

pub fn fn_true<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if !args.is_empty() {
        return Err(XPath31Error::function("true", "Expected 0 arguments"));
    }
    Ok(XdmValue::from_bool(true))
}

pub fn fn_false<N: Clone>(args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if !args.is_empty() {
        return Err(XPath31Error::function("false", "Expected 0 arguments"));
    }
    Ok(XdmValue::from_bool(false))
}

pub fn fn_not<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("not", "Expected 1 argument"));
    }
    let val = args.remove(0);
    Ok(XdmValue::from_bool(!val.effective_boolean_value()))
}

pub fn fn_boolean<N: Clone>(mut args: Vec<XdmValue<N>>) -> Result<XdmValue<N>, XPath31Error> {
    if args.len() != 1 {
        return Err(XPath31Error::function("boolean", "Expected 1 argument"));
    }
    let val = args.remove(0);
    Ok(XdmValue::from_bool(val.effective_boolean_value()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_true_false() {
        let t: XdmValue<()> = fn_true(vec![]).unwrap();
        assert!(t.effective_boolean_value());

        let f: XdmValue<()> = fn_false(vec![]).unwrap();
        assert!(!f.effective_boolean_value());
    }

    #[test]
    fn test_not() {
        let result: XdmValue<()> = fn_not(vec![XdmValue::from_bool(true)]).unwrap();
        assert!(!result.effective_boolean_value());

        let result: XdmValue<()> = fn_not(vec![XdmValue::from_bool(false)]).unwrap();
        assert!(result.effective_boolean_value());
    }

    #[test]
    fn test_boolean() {
        let result: XdmValue<()> = fn_boolean(vec![XdmValue::from_string("hello")]).unwrap();
        assert!(result.effective_boolean_value());

        let result: XdmValue<()> = fn_boolean(vec![XdmValue::from_string("")]).unwrap();
        assert!(!result.effective_boolean_value());

        let result: XdmValue<()> = fn_boolean(vec![XdmValue::from_integer(0)]).unwrap();
        assert!(!result.effective_boolean_value());

        let result: XdmValue<()> = fn_boolean(vec![XdmValue::from_integer(1)]).unwrap();
        assert!(result.effective_boolean_value());
    }
}
