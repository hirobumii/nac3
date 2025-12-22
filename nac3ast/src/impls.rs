use crate::{Constant, ExprKind};

impl<U> ExprKind<U> {
    /// Returns a short name for the node suitable for use in error messages.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::BoolOp { .. } | Self::BinOp { .. } | Self::UnaryOp { .. } => "operator",
            Self::Subscript { .. } => "subscript",
            Self::Await { .. } => "await expression",
            Self::Yield { .. } | Self::YieldFrom { .. } => "yield expression",
            Self::Compare { .. } => "comparison",
            Self::Attribute { .. } => "attribute",
            Self::Call { .. } => "function call",
            Self::Constant { value, .. } => match value {
                Constant::Str(_)
                | Constant::Int(_)
                | Constant::Float(_)
                | Constant::Complex { .. }
                | Constant::Bytes(_) => "literal",
                Constant::Tuple(_) => "tuple",
                Constant::Bool(_) | Constant::None => "keyword",
                Constant::Ellipsis => "ellipsis",
            },
            Self::List { .. } => "list",
            Self::Tuple { .. } => "tuple",
            Self::Dict { .. } => "dict display",
            Self::Set { .. } => "set display",
            Self::ListComp { .. } => "list comprehension",
            Self::DictComp { .. } => "dict comprehension",
            Self::SetComp { .. } => "set comprehension",
            Self::GeneratorExp { .. } => "generator expression",
            Self::Starred { .. } => "starred",
            Self::Slice { .. } => "slice",
            Self::JoinedStr { values } => {
                if values.iter().any(|e| matches!(e.node, Self::JoinedStr { .. })) {
                    "f-string expression"
                } else {
                    "literal"
                }
            }
            Self::FormattedValue { .. } => "f-string expression",
            Self::Name { .. } => "name",
            Self::Lambda { .. } => "lambda",
            Self::IfExp { .. } => "conditional expression",
            Self::NamedExpr { .. } => "named expression",
        }
    }
}
