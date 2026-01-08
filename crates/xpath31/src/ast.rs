//! XPath 3.1 expression AST types.
//!
//! Core types: [`Expression`], [`SequenceType`], [`QName`].

use std::fmt;

pub use petty_xpath1::ast::{
    Axis, BinaryOperator, LocationPath, NodeTest, NodeTypeTest, Step, UnaryOperator,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(Literal),
    LocationPath(LocationPath),
    Variable(String),
    FunctionCall {
        name: QName,
        args: Vec<Expression>,
    },
    BinaryOp {
        left: Box<Expression>,
        op: BinaryOperator,
        right: Box<Expression>,
    },
    UnaryOp {
        op: UnaryOperator,
        expr: Box<Expression>,
    },

    LetExpr {
        bindings: Vec<(String, Box<Expression>)>,
        return_expr: Box<Expression>,
    },
    IfExpr {
        condition: Box<Expression>,
        then_expr: Box<Expression>,
        else_expr: Box<Expression>,
    },
    ForExpr {
        bindings: Vec<(String, Box<Expression>)>,
        return_expr: Box<Expression>,
    },
    QuantifiedExpr {
        quantifier: Quantifier,
        bindings: Vec<(String, Box<Expression>)>,
        satisfies: Box<Expression>,
    },

    MapConstructor(Vec<MapEntry>),
    ArrayConstructor(ArrayConstructorKind),
    InlineFunction {
        params: Vec<Param>,
        return_type: Option<SequenceType>,
        body: Box<Expression>,
    },
    NamedFunctionRef {
        name: QName,
        arity: usize,
    },

    ArrowExpr {
        base: Box<Expression>,
        steps: Vec<ArrowStep>,
    },
    SimpleMapExpr {
        base: Box<Expression>,
        mapping: Box<Expression>,
    },
    LookupExpr {
        base: Box<Expression>,
        key: LookupKey,
    },
    UnaryLookup(LookupKey),

    RangeExpr {
        start: Box<Expression>,
        end: Box<Expression>,
    },
    StringConcat {
        left: Box<Expression>,
        right: Box<Expression>,
    },

    InstanceOf {
        expr: Box<Expression>,
        sequence_type: SequenceType,
    },
    TreatAs {
        expr: Box<Expression>,
        sequence_type: SequenceType,
    },
    CastAs {
        expr: Box<Expression>,
        single_type: SingleType,
    },
    CastableAs {
        expr: Box<Expression>,
        single_type: SingleType,
    },

    ContextItem,
    Sequence(Vec<Expression>),
    FilterExpr {
        base: Box<Expression>,
        predicates: Vec<Expression>,
    },
    DynamicFunctionCall {
        function_expr: Box<Expression>,
        args: Vec<Expression>,
    },
    ArgumentPlaceholder,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    String(String),
    Integer(i64),
    Decimal(String),
    Double(f64),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QName {
    pub prefix: Option<String>,
    pub local_part: String,
}

impl QName {
    pub fn new(local_part: impl Into<String>) -> Self {
        Self {
            prefix: None,
            local_part: local_part.into(),
        }
    }

    pub fn with_prefix(prefix: impl Into<String>, local_part: impl Into<String>) -> Self {
        Self {
            prefix: Some(prefix.into()),
            local_part: local_part.into(),
        }
    }
}

impl fmt::Display for QName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.prefix {
            Some(p) => write!(f, "{}:{}", p, self.local_part),
            None => write!(f, "{}", self.local_part),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantifier {
    Some,
    Every,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapEntry {
    pub key: Box<Expression>,
    pub value: Box<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayConstructorKind {
    Square(Vec<Expression>),
    Curly(Box<Expression>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub type_decl: Option<SequenceType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrowStep {
    pub function_name: QName,
    pub args: Vec<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LookupKey {
    Wildcard,
    NCName(String),
    Integer(i64),
    Parenthesized(Box<Expression>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SequenceType {
    pub item_type: ItemType,
    pub occurrence: OccurrenceIndicator,
}

impl SequenceType {
    pub fn single(item_type: ItemType) -> Self {
        Self {
            item_type,
            occurrence: OccurrenceIndicator::ExactlyOne,
        }
    }

    pub fn zero_or_one(item_type: ItemType) -> Self {
        Self {
            item_type,
            occurrence: OccurrenceIndicator::ZeroOrOne,
        }
    }

    pub fn zero_or_more(item_type: ItemType) -> Self {
        Self {
            item_type,
            occurrence: OccurrenceIndicator::ZeroOrMore,
        }
    }

    pub fn one_or_more(item_type: ItemType) -> Self {
        Self {
            item_type,
            occurrence: OccurrenceIndicator::OneOrMore,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemType {
    Item,
    AtomicOrUnion(QName),
    KindTest(KindTest),
    FunctionTest(Option<Vec<SequenceType>>, Option<Box<SequenceType>>),
    MapTest(Option<Box<AtomicType>>, Option<Box<SequenceType>>),
    ArrayTest(Option<Box<SequenceType>>),
    ParenthesizedItemType(Box<ItemType>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum KindTest {
    Document(Option<Box<KindTest>>),
    Element(Option<QName>, Option<QName>),
    Attribute(Option<QName>, Option<QName>),
    SchemaElement(QName),
    SchemaAttribute(QName),
    PITest(Option<String>),
    CommentTest,
    TextTest,
    NamespaceNodeTest,
    AnyKindTest,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomicType {
    pub name: QName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OccurrenceIndicator {
    ExactlyOne,
    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SingleType {
    pub type_name: QName,
    pub optional: bool,
}

impl Expression {
    pub fn literal_string(s: impl Into<String>) -> Self {
        Expression::Literal(Literal::String(s.into()))
    }

    pub fn literal_integer(i: i64) -> Self {
        Expression::Literal(Literal::Integer(i))
    }

    pub fn literal_double(d: f64) -> Self {
        Expression::Literal(Literal::Double(d))
    }

    pub fn variable(name: impl Into<String>) -> Self {
        Expression::Variable(name.into())
    }

    pub fn function_call(name: QName, args: Vec<Expression>) -> Self {
        Expression::FunctionCall { name, args }
    }

    pub fn binary_op(left: Expression, op: BinaryOperator, right: Expression) -> Self {
        Expression::BinaryOp {
            left: Box::new(left),
            op,
            right: Box::new(right),
        }
    }

    pub fn let_expr(bindings: Vec<(String, Expression)>, return_expr: Expression) -> Self {
        Expression::LetExpr {
            bindings: bindings
                .into_iter()
                .map(|(n, e)| (n, Box::new(e)))
                .collect(),
            return_expr: Box::new(return_expr),
        }
    }

    pub fn if_expr(condition: Expression, then_expr: Expression, else_expr: Expression) -> Self {
        Expression::IfExpr {
            condition: Box::new(condition),
            then_expr: Box::new(then_expr),
            else_expr: Box::new(else_expr),
        }
    }

    pub fn for_expr(bindings: Vec<(String, Expression)>, return_expr: Expression) -> Self {
        Expression::ForExpr {
            bindings: bindings
                .into_iter()
                .map(|(n, e)| (n, Box::new(e)))
                .collect(),
            return_expr: Box::new(return_expr),
        }
    }

    pub fn arrow_expr(base: Expression, steps: Vec<ArrowStep>) -> Self {
        Expression::ArrowExpr {
            base: Box::new(base),
            steps,
        }
    }

    pub fn simple_map(base: Expression, mapping: Expression) -> Self {
        Expression::SimpleMapExpr {
            base: Box::new(base),
            mapping: Box::new(mapping),
        }
    }

    pub fn lookup(base: Expression, key: LookupKey) -> Self {
        Expression::LookupExpr {
            base: Box::new(base),
            key,
        }
    }

    pub fn range(start: Expression, end: Expression) -> Self {
        Expression::RangeExpr {
            start: Box::new(start),
            end: Box::new(end),
        }
    }

    pub fn string_concat(left: Expression, right: Expression) -> Self {
        Expression::StringConcat {
            left: Box::new(left),
            right: Box::new(right),
        }
    }
}
