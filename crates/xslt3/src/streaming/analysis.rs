use petty_xpath31::Expression;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posture {
    Grounded,
    Striding,
    Crawling,
    Climbing,
    Roaming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sweep {
    Motionless,
    Consuming,
    FreeRanging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Usage {
    Absorption,
    Inspection,
    Transmission,
}

#[derive(Debug, Clone)]
pub struct StreamabilityResult {
    pub posture: Posture,
    pub sweep: Sweep,
    pub streamable: bool,
    pub reason: Option<String>,
}

impl StreamabilityResult {
    pub fn grounded() -> Self {
        Self {
            posture: Posture::Grounded,
            sweep: Sweep::Motionless,
            streamable: true,
            reason: None,
        }
    }

    pub fn striding() -> Self {
        Self {
            posture: Posture::Striding,
            sweep: Sweep::Consuming,
            streamable: true,
            reason: None,
        }
    }

    pub fn not_streamable(reason: impl Into<String>) -> Self {
        Self {
            posture: Posture::Roaming,
            sweep: Sweep::FreeRanging,
            streamable: false,
            reason: Some(reason.into()),
        }
    }

    pub fn combine(&self, other: &Self) -> Self {
        let (posture, streamable, reason) = match (self.posture, other.posture) {
            (Posture::Grounded, p) | (p, Posture::Grounded) => {
                (p, self.streamable && other.streamable, None)
            }
            (Posture::Striding, Posture::Striding) => (Posture::Striding, true, None),
            (Posture::Striding, Posture::Crawling) | (Posture::Crawling, Posture::Striding) => (
                Posture::Roaming,
                false,
                Some("Cannot combine striding and crawling".to_string()),
            ),
            (Posture::Climbing, Posture::Climbing) => (Posture::Climbing, true, None),
            (Posture::Roaming, _) | (_, Posture::Roaming) => (
                Posture::Roaming,
                false,
                self.reason.clone().or_else(|| other.reason.clone()),
            ),
            _ => (
                Posture::Roaming,
                false,
                Some("Incompatible postures".to_string()),
            ),
        };

        let sweep = match (self.sweep, other.sweep) {
            (Sweep::Motionless, s) | (s, Sweep::Motionless) => s,
            (Sweep::Consuming, Sweep::Consuming) => Sweep::Consuming,
            _ => Sweep::FreeRanging,
        };

        Self {
            posture,
            sweep,
            streamable,
            reason,
        }
    }
}

pub struct StreamabilityAnalyzer;

impl StreamabilityAnalyzer {
    pub fn analyze_expression(expr: &Expression) -> StreamabilityResult {
        match expr {
            Expression::Literal(_) => StreamabilityResult::grounded(),
            Expression::Variable(_) => StreamabilityResult::grounded(),
            Expression::ContextItem => StreamabilityResult {
                posture: Posture::Striding,
                sweep: Sweep::Motionless,
                streamable: true,
                reason: None,
            },

            Expression::LocationPath(path) => Self::analyze_location_path(path),

            Expression::FunctionCall { name, args } => {
                Self::analyze_function_call(&name.to_string(), args)
            }

            Expression::BinaryOp { left, right, .. } => {
                let l = Self::analyze_expression(left);
                let r = Self::analyze_expression(right);
                l.combine(&r)
            }

            Expression::LetExpr {
                bindings,
                return_expr,
            } => {
                let mut result = StreamabilityResult::grounded();
                for (_, binding_expr) in bindings {
                    result = result.combine(&Self::analyze_expression(binding_expr));
                }
                result.combine(&Self::analyze_expression(return_expr))
            }

            Expression::IfExpr {
                condition,
                then_expr,
                else_expr,
            } => {
                let c = Self::analyze_expression(condition);
                let t = Self::analyze_expression(then_expr);
                let e = Self::analyze_expression(else_expr);
                c.combine(&t).combine(&e)
            }

            Expression::ForExpr {
                bindings,
                return_expr,
            } => {
                let mut result = StreamabilityResult::grounded();
                for (_, binding_expr) in bindings {
                    let binding_result = Self::analyze_expression(binding_expr);
                    if binding_result.posture != Posture::Grounded {
                        return StreamabilityResult::not_streamable(
                            "for expression bindings must be grounded",
                        );
                    }
                    result = result.combine(&binding_result);
                }
                result.combine(&Self::analyze_expression(return_expr))
            }

            Expression::MapConstructor(entries) => {
                let mut result = StreamabilityResult::grounded();
                for entry in entries {
                    result = result.combine(&Self::analyze_expression(&entry.key));
                    result = result.combine(&Self::analyze_expression(&entry.value));
                }
                result
            }

            Expression::ArrayConstructor(arr_kind) => match arr_kind {
                petty_xpath31::ast::ArrayConstructorKind::Square(items) => {
                    let mut result = StreamabilityResult::grounded();
                    for item in items {
                        result = result.combine(&Self::analyze_expression(item));
                    }
                    result
                }
                petty_xpath31::ast::ArrayConstructorKind::Curly(expr) => {
                    Self::analyze_expression(expr)
                }
            },

            _ => StreamabilityResult::grounded(),
        }
    }

    fn analyze_location_path(path: &petty_xpath31::ast::LocationPath) -> StreamabilityResult {
        use petty_xpath31::ast::Axis;

        if path.steps.is_empty() {
            return if path.is_absolute {
                StreamabilityResult::not_streamable("Absolute path to root in streaming")
            } else {
                StreamabilityResult {
                    posture: Posture::Striding,
                    sweep: Sweep::Motionless,
                    streamable: true,
                    reason: None,
                }
            };
        }

        let mut posture = if path.is_absolute {
            Posture::Roaming
        } else {
            Posture::Striding
        };
        let mut sweep = Sweep::Motionless;
        let mut streamable = !path.is_absolute;

        for step in &path.steps {
            match step.axis {
                Axis::Child => {
                    if posture == Posture::Striding {
                        posture = Posture::Striding;
                        sweep = Sweep::Consuming;
                    }
                }
                Axis::Attribute | Axis::SelfAxis => {
                    sweep = Sweep::Motionless;
                }
                Axis::Parent | Axis::Ancestor => {
                    posture = Posture::Climbing;
                }
                Axis::Descendant | Axis::DescendantOrSelf => {
                    posture = Posture::Crawling;
                    sweep = Sweep::Consuming;
                }
                Axis::FollowingSibling
                | Axis::PrecedingSibling
                | Axis::Following
                | Axis::Preceding => {
                    posture = Posture::Roaming;
                    sweep = Sweep::FreeRanging;
                    streamable = false;
                }
            }
        }

        StreamabilityResult {
            posture,
            sweep,
            streamable,
            reason: if streamable {
                None
            } else {
                Some("Non-streamable axis".to_string())
            },
        }
    }

    fn analyze_function_call(name: &str, args: &[Expression]) -> StreamabilityResult {
        let absorbing_functions = [
            "count",
            "sum",
            "avg",
            "min",
            "max",
            "string",
            "string-join",
            "normalize-space",
            "data",
            "boolean",
            "number",
        ];

        let mut result = StreamabilityResult::grounded();
        for arg in args {
            result = result.combine(&Self::analyze_expression(arg));
        }

        if absorbing_functions.iter().any(|&f| name.ends_with(f))
            && (result.posture == Posture::Striding || result.posture == Posture::Crawling)
        {
            return StreamabilityResult {
                posture: Posture::Grounded,
                sweep: Sweep::Consuming,
                streamable: true,
                reason: None,
            };
        }

        result
    }

    pub fn validate_streaming_template(
        template: &crate::ast::PreparsedTemplate,
    ) -> Result<(), crate::error::Xslt3Error> {
        for instruction in &template.0 {
            Self::validate_streaming_instruction(instruction)?;
        }
        Ok(())
    }

    pub fn validate_streaming_instruction(
        instruction: &crate::ast::Xslt3Instruction,
    ) -> Result<(), crate::error::Xslt3Error> {
        use crate::ast::Xslt3Instruction;
        use crate::error::Xslt3Error;

        match instruction {
            Xslt3Instruction::Text(_) => Ok(()),
            Xslt3Instruction::TextValueTemplate(tvt) => {
                for part in &tvt.0 {
                    if let crate::ast::TvtPart::Dynamic(expr) = part {
                        let result = Self::analyze_expression(expr);
                        if !result.streamable {
                            return Err(Xslt3Error::streaming(format!(
                                "Text value template expression is not streamable: {}",
                                result.reason.unwrap_or_default()
                            )));
                        }
                    }
                }
                Ok(())
            }
            Xslt3Instruction::ValueOf { select, .. } => {
                let result = Self::analyze_expression(select);
                if !result.streamable {
                    return Err(Xslt3Error::streaming(format!(
                        "xsl:value-of select expression is not streamable: {}",
                        result.reason.unwrap_or_default()
                    )));
                }
                Ok(())
            }
            Xslt3Instruction::If { test, body } => {
                let result = Self::analyze_expression(test);
                if result.posture == Posture::Roaming {
                    return Err(Xslt3Error::streaming(
                        "xsl:if test expression cannot be roaming in streaming mode",
                    ));
                }
                Self::validate_streaming_template(body)
            }
            Xslt3Instruction::Choose { whens, otherwise } => {
                for when in whens {
                    let result = Self::analyze_expression(&when.test);
                    if result.posture == Posture::Roaming {
                        return Err(Xslt3Error::streaming(
                            "xsl:when test expression cannot be roaming in streaming mode",
                        ));
                    }
                    Self::validate_streaming_template(&when.body)?;
                }
                if let Some(otherwise_body) = otherwise {
                    Self::validate_streaming_template(otherwise_body)?;
                }
                Ok(())
            }
            Xslt3Instruction::Variable {
                name, select, body, ..
            } => {
                if let Some(sel) = select {
                    let result = Self::analyze_expression(sel);
                    if result.posture == Posture::Roaming {
                        return Err(Xslt3Error::streaming(format!(
                            "Variable ${} expression cannot be roaming in streaming mode",
                            name
                        )));
                    }
                }
                if let Some(body_template) = body {
                    Self::validate_streaming_template(body_template)?;
                }
                Ok(())
            }
            Xslt3Instruction::ContentTag { body, .. } => Self::validate_streaming_template(body),
            Xslt3Instruction::CopyOf { select } => {
                let result = Self::analyze_expression(select);
                if result.posture == Posture::Roaming {
                    return Err(Xslt3Error::streaming(
                        "xsl:copy-of select expression cannot be roaming in streaming mode",
                    ));
                }
                Ok(())
            }
            Xslt3Instruction::ForEach { select, body, .. } => {
                let result = Self::analyze_expression(select);
                if result.posture == Posture::Roaming {
                    return Err(Xslt3Error::streaming(
                        "xsl:for-each select expression cannot be roaming in streaming mode",
                    ));
                }
                Self::validate_streaming_template(body)
            }
            Xslt3Instruction::ApplyTemplates { select, .. } => {
                if let Some(expr) = select {
                    let result = Self::analyze_expression(expr);
                    if result.posture == Posture::Roaming {
                        return Err(Xslt3Error::streaming(
                            "xsl:apply-templates select expression cannot be roaming in streaming mode",
                        ));
                    }
                }
                Ok(())
            }
            Xslt3Instruction::AccumulatorBefore { .. }
            | Xslt3Instruction::AccumulatorAfter { .. } => Ok(()),
            Xslt3Instruction::Sequence { select } => {
                let result = Self::analyze_expression(select);
                if !result.streamable {
                    return Err(Xslt3Error::streaming(format!(
                        "xsl:sequence select expression is not streamable: {}",
                        result.reason.unwrap_or_default()
                    )));
                }
                Ok(())
            }
            Xslt3Instruction::Copy { body, .. } => Self::validate_streaming_template(body),
            Xslt3Instruction::Element { body, .. } => Self::validate_streaming_template(body),
            Xslt3Instruction::Attribute { body, .. } => Self::validate_streaming_template(body),
            Xslt3Instruction::Comment { body } => Self::validate_streaming_template(body),
            Xslt3Instruction::ProcessingInstruction { body, .. } => {
                Self::validate_streaming_template(body)
            }
            Xslt3Instruction::CallTemplate { params, .. } => {
                for param in params {
                    let result = Self::analyze_expression(&param.select);
                    if result.posture == Posture::Roaming {
                        return Err(Xslt3Error::streaming(format!(
                            "xsl:with-param {} expression cannot be roaming in streaming mode",
                            param.name
                        )));
                    }
                }
                Ok(())
            }
            Xslt3Instruction::Message { body, select, .. } => {
                if let Some(s) = select {
                    let result = Self::analyze_expression(s);
                    if result.posture == Posture::Roaming {
                        return Err(Xslt3Error::streaming(
                            "xsl:message select expression cannot be roaming in streaming mode",
                        ));
                    }
                }
                if let Some(b) = body {
                    Self::validate_streaming_template(b)?;
                }
                Ok(())
            }
            _ => Err(Xslt3Error::streaming(format!(
                "Instruction {:?} is not supported in streaming mode",
                std::mem::discriminant(instruction)
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petty_xpath31::ast::{Axis, Literal, LocationPath, NodeTest, Step};

    fn make_step(axis: Axis, name: &str) -> Step {
        Step {
            axis,
            node_test: NodeTest::Name(name.to_string()),
            predicates: vec![],
        }
    }

    fn make_path(is_absolute: bool, steps: Vec<Step>) -> LocationPath {
        LocationPath {
            start_point: None,
            is_absolute,
            steps,
        }
    }

    #[test]
    fn test_grounded_literal() {
        let expr = Expression::Literal(Literal::String("hello".to_string()));
        let result = StreamabilityAnalyzer::analyze_expression(&expr);
        assert_eq!(result.posture, Posture::Grounded);
        assert!(result.streamable);
    }

    #[test]
    fn test_grounded_integer() {
        let expr = Expression::Literal(Literal::Integer(42));
        let result = StreamabilityAnalyzer::analyze_expression(&expr);
        assert_eq!(result.posture, Posture::Grounded);
        assert!(result.streamable);
    }

    #[test]
    fn test_grounded_variable() {
        let expr = Expression::Variable("x".to_string());
        let result = StreamabilityAnalyzer::analyze_expression(&expr);
        assert_eq!(result.posture, Posture::Grounded);
        assert!(result.streamable);
    }

    #[test]
    fn test_context_item_striding() {
        let expr = Expression::ContextItem;
        let result = StreamabilityAnalyzer::analyze_expression(&expr);
        assert_eq!(result.posture, Posture::Striding);
        assert!(result.streamable);
    }

    #[test]
    fn test_child_axis_striding() {
        let path = make_path(false, vec![make_step(Axis::Child, "item")]);
        let expr = Expression::LocationPath(path);
        let result = StreamabilityAnalyzer::analyze_expression(&expr);
        assert_eq!(result.posture, Posture::Striding);
        assert!(result.streamable);
    }

    #[test]
    fn test_descendant_axis_crawling() {
        let path = make_path(false, vec![make_step(Axis::Descendant, "item")]);
        let expr = Expression::LocationPath(path);
        let result = StreamabilityAnalyzer::analyze_expression(&expr);
        assert_eq!(result.posture, Posture::Crawling);
        assert!(result.streamable);
    }

    #[test]
    fn test_ancestor_axis_climbing() {
        let path = make_path(false, vec![make_step(Axis::Ancestor, "parent")]);
        let expr = Expression::LocationPath(path);
        let result = StreamabilityAnalyzer::analyze_expression(&expr);
        assert_eq!(result.posture, Posture::Climbing);
        assert!(result.streamable);
    }

    #[test]
    fn test_following_sibling_not_streamable() {
        let path = make_path(false, vec![make_step(Axis::FollowingSibling, "sibling")]);
        let expr = Expression::LocationPath(path);
        let result = StreamabilityAnalyzer::analyze_expression(&expr);
        assert_eq!(result.posture, Posture::Roaming);
        assert!(!result.streamable);
    }

    #[test]
    fn test_absolute_path_not_streamable() {
        let path = make_path(true, vec![make_step(Axis::Child, "root")]);
        let expr = Expression::LocationPath(path);
        let result = StreamabilityAnalyzer::analyze_expression(&expr);
        assert!(!result.streamable);
    }

    #[test]
    fn test_combine_grounded_striding() {
        let grounded = StreamabilityResult::grounded();
        let striding = StreamabilityResult::striding();
        let combined = grounded.combine(&striding);
        assert_eq!(combined.posture, Posture::Striding);
        assert!(combined.streamable);
    }

    #[test]
    fn test_combine_striding_crawling_not_streamable() {
        let striding = StreamabilityResult::striding();
        let crawling = StreamabilityResult {
            posture: Posture::Crawling,
            sweep: Sweep::Consuming,
            streamable: true,
            reason: None,
        };
        let combined = striding.combine(&crawling);
        assert_eq!(combined.posture, Posture::Roaming);
        assert!(!combined.streamable);
    }

    #[test]
    fn test_absorbing_function_grounded() {
        let path = make_path(false, vec![make_step(Axis::Child, "item")]);
        let args = vec![Expression::LocationPath(path)];
        let result = StreamabilityAnalyzer::analyze_function_call("count", &args);
        assert_eq!(result.posture, Posture::Grounded);
        assert!(result.streamable);
    }

    #[test]
    fn test_let_expression_streamability() {
        let expr = Expression::LetExpr {
            bindings: vec![(
                "x".to_string(),
                Box::new(Expression::Literal(Literal::Integer(1))),
            )],
            return_expr: Box::new(Expression::ContextItem),
        };
        let result = StreamabilityAnalyzer::analyze_expression(&expr);
        assert_eq!(result.posture, Posture::Striding);
        assert!(result.streamable);
    }

    #[test]
    fn test_if_expression_streamability() {
        let expr = Expression::IfExpr {
            condition: Box::new(Expression::Literal(Literal::Integer(1))),
            then_expr: Box::new(Expression::ContextItem),
            else_expr: Box::new(Expression::Literal(Literal::String("default".to_string()))),
        };
        let result = StreamabilityAnalyzer::analyze_expression(&expr);
        assert_eq!(result.posture, Posture::Striding);
        assert!(result.streamable);
    }

    #[test]
    fn test_sweep_motionless() {
        let result = StreamabilityResult::grounded();
        assert_eq!(result.sweep, Sweep::Motionless);
    }

    #[test]
    fn test_sweep_consuming() {
        let result = StreamabilityResult::striding();
        assert_eq!(result.sweep, Sweep::Consuming);
    }

    #[test]
    fn test_validate_streaming_template_text() {
        use crate::ast::{PreparsedTemplate, Xslt3Instruction};

        let template = PreparsedTemplate(vec![Xslt3Instruction::Text("Hello".to_string())]);
        let result = StreamabilityAnalyzer::validate_streaming_template(&template);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_streaming_template_value_of_grounded() {
        use crate::ast::{PreparsedTemplate, Xslt3Instruction};

        let template = PreparsedTemplate(vec![Xslt3Instruction::ValueOf {
            select: Expression::Literal(Literal::String("test".to_string())),
            separator: None,
        }]);
        let result = StreamabilityAnalyzer::validate_streaming_template(&template);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_streaming_template_rejects_roaming() {
        use crate::ast::{PreparsedTemplate, Xslt3Instruction};

        let template = PreparsedTemplate(vec![Xslt3Instruction::ValueOf {
            select: Expression::LocationPath(make_path(
                false,
                vec![make_step(Axis::PrecedingSibling, "item")],
            )),
            separator: None,
        }]);
        let result = StreamabilityAnalyzer::validate_streaming_template(&template);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_streaming_template_if_grounded() {
        use crate::ast::{PreparsedTemplate, Xslt3Instruction};

        let template = PreparsedTemplate(vec![Xslt3Instruction::If {
            test: Expression::Literal(Literal::Integer(1)),
            body: PreparsedTemplate(vec![Xslt3Instruction::Text("yes".to_string())]),
        }]);
        let result = StreamabilityAnalyzer::validate_streaming_template(&template);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_streaming_template_if_roaming_rejected() {
        use crate::ast::{PreparsedTemplate, Xslt3Instruction};

        let template = PreparsedTemplate(vec![Xslt3Instruction::If {
            test: Expression::LocationPath(make_path(
                false,
                vec![make_step(Axis::PrecedingSibling, "item")],
            )),
            body: PreparsedTemplate(vec![Xslt3Instruction::Text("yes".to_string())]),
        }]);
        let result = StreamabilityAnalyzer::validate_streaming_template(&template);
        assert!(result.is_err());
    }
}
