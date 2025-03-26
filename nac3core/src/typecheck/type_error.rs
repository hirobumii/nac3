use std::{collections::HashMap, fmt::Display};

use itertools::Itertools;

use nac3parser::ast::{Cmpop, Location, StrRef};

use super::{
    magic_methods::{Binop, HasOpInfo},
    typedef::{RecordKey, Type, TypeEnum, Unifier},
};

#[derive(Debug, Clone)]
pub enum TypeErrorKind {
    GotMultipleValues {
        name: StrRef,
    },
    TooManyArguments {
        expected_min_count: usize,
        expected_max_count: usize,
        got_count: usize,
    },
    MissingArgs {
        missing_arg_names: Vec<StrRef>,
    },
    UnknownArgName(StrRef),
    IncorrectArgType {
        name: StrRef,
        expected: Type,
        got: Type,
    },
    UnsupportedBinaryOpTypes {
        operator: Binop,
        lhs_type: Type,
        rhs_type: Type,
        expected_rhs_type: Type,
    },
    UnsupportedComparsionOpTypes {
        operator: Cmpop,
        lhs_type: Type,
        rhs_type: Type,
        expected_rhs_type: Type,
    },
    FieldUnificationError {
        field: RecordKey,
        types: (Type, Type),
        loc: (Option<Location>, Option<Location>),
    },
    IncompatibleRange(Type, Vec<Type>),
    IncompatibleTypes(Type, Type),
    MutationError(RecordKey, Type),
    NoSuchField(RecordKey, Type),
    TupleIndexOutOfBounds {
        index: i32,
        len: i32,
    },
    RequiresTypeAnn,
    PolymorphicFunctionPointer,
    NoSuchAttribute(RecordKey, Type),
}

#[derive(Debug, Clone)]
pub struct TypeError {
    pub kind: TypeErrorKind,
    pub loc: Option<Location>,
}

impl TypeError {
    #[must_use]
    pub fn new(kind: TypeErrorKind, loc: Option<Location>) -> TypeError {
        TypeError { kind, loc }
    }

    #[must_use]
    pub fn at(mut self, loc: Option<Location>) -> TypeError {
        self.loc = self.loc.or(loc);
        self
    }

    #[must_use]
    pub fn to_display(self, unifier: &Unifier) -> DisplayTypeError {
        DisplayTypeError { err: self, unifier }
    }
}

pub struct DisplayTypeError<'a> {
    pub err: TypeError,
    pub unifier: &'a Unifier,
}

fn loc_to_str(loc: Option<Location>) -> String {
    match loc {
        Some(loc) => format!("(in {loc})"),
        None => String::new(),
    }
}

impl Display for DisplayTypeError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use TypeErrorKind::*;
        let mut notes = Some(HashMap::new());
        match &self.err.kind {
            GotMultipleValues { name } => {
                write!(f, "For multiple values for parameter {name}")
            }
            TooManyArguments { expected_min_count, expected_max_count, got_count } => {
                debug_assert!(expected_min_count <= expected_max_count);
                if expected_min_count == expected_max_count {
                    let expected_count = expected_min_count; // or expected_max_count
                    write!(f, "Too many arguments. Expected {expected_count} but got {got_count}")
                } else {
                    write!(
                        f,
                        "Too many arguments. Expected {expected_min_count} to {expected_max_count} arguments but got {got_count}"
                    )
                }
            }
            MissingArgs { missing_arg_names } => {
                let args = missing_arg_names.iter().join(", ");
                write!(f, "Missing arguments: {args}")
            }
            UnsupportedBinaryOpTypes { operator, lhs_type, rhs_type, expected_rhs_type } => {
                let op_symbol = operator.op_info().symbol;

                let lhs_type_str = self.unifier.stringify_with_notes(*lhs_type, &mut notes);
                let rhs_type_str = self.unifier.stringify_with_notes(*rhs_type, &mut notes);
                let expected_rhs_type_str =
                    self.unifier.stringify_with_notes(*expected_rhs_type, &mut notes);

                write!(
                    f,
                    "Unsupported operand type(s) for {op_symbol}: '{lhs_type_str}' and '{rhs_type_str}' (right operand should have type {expected_rhs_type_str})"
                )
            }
            UnsupportedComparsionOpTypes { operator, lhs_type, rhs_type, expected_rhs_type } => {
                let op_symbol = operator.op_info().symbol;

                let lhs_type_str = self.unifier.stringify_with_notes(*lhs_type, &mut notes);
                let rhs_type_str = self.unifier.stringify_with_notes(*rhs_type, &mut notes);
                let expected_rhs_type_str =
                    self.unifier.stringify_with_notes(*expected_rhs_type, &mut notes);

                write!(
                    f,
                    "'{op_symbol}' not supported between instances of '{lhs_type_str}' and '{rhs_type_str}' (right operand should have type {expected_rhs_type_str})"
                )
            }
            UnknownArgName(name) => {
                write!(f, "Unknown argument name: {name}")
            }
            IncorrectArgType { name, expected, got } => {
                let expected = self.unifier.stringify_with_notes(*expected, &mut notes);
                let got = self.unifier.stringify_with_notes(*got, &mut notes);
                write!(
                    f,
                    "Incorrect argument type for parameter {name}. Expected {expected}, but got {got}"
                )
            }
            FieldUnificationError { field, types, loc } => {
                let lhs = self.unifier.stringify_with_notes(types.0, &mut notes);
                let rhs = self.unifier.stringify_with_notes(types.1, &mut notes);
                write!(
                    f,
                    "Unable to unify field {}: Got types {}{} and {}{}",
                    field,
                    lhs,
                    loc_to_str(loc.0),
                    rhs,
                    loc_to_str(loc.1)
                )
            }
            IncompatibleRange(t, ts) => {
                let t = self.unifier.stringify_with_notes(*t, &mut notes);
                let ts = ts
                    .iter()
                    .map(|t| self.unifier.stringify_with_notes(*t, &mut notes))
                    .collect::<Vec<_>>();
                write!(f, "Expected any one of these types: {}, but got {}", ts.join(", "), t)
            }
            IncompatibleTypes(t1, t2) => {
                let type1 = self.unifier.get_ty_immutable(*t1);
                let type2 = self.unifier.get_ty_immutable(*t2);
                match (&*type1, &*type2) {
                    (TypeEnum::TCall(calls), _) => {
                        let loc = self.unifier.calls[calls[0].0].loc;
                        let result = write!(
                            f,
                            "{} is not callable",
                            self.unifier.stringify_with_notes(*t2, &mut notes)
                        );
                        if let Some(loc) = loc {
                            result?;
                            write!(f, " (in {loc})")?;
                            return Ok(());
                        }
                        result
                    }
                    (
                        TypeEnum::TTuple { ty: ty1, is_vararg_ctx: is_vararg1 },
                        TypeEnum::TTuple { ty: ty2, is_vararg_ctx: is_vararg2 },
                    ) if !is_vararg1 && !is_vararg2 && ty1.len() != ty2.len() => {
                        let t1 = self.unifier.stringify_with_notes(*t1, &mut notes);
                        let t2 = self.unifier.stringify_with_notes(*t2, &mut notes);
                        write!(f, "Tuple length mismatch: got {t1} and {t2}")
                    }
                    _ => {
                        let t1 = self.unifier.stringify_with_notes(*t1, &mut notes);
                        let t2 = self.unifier.stringify_with_notes(*t2, &mut notes);
                        write!(f, "Incompatible types: {t1} and {t2}")
                    }
                }
            }
            MutationError(name, t) => {
                if let TypeEnum::TTuple { .. } = &*self.unifier.get_ty_immutable(*t) {
                    write!(f, "Cannot assign to an element of a tuple")
                } else {
                    let t = self.unifier.stringify_with_notes(*t, &mut notes);
                    write!(f, "Cannot assign to field {name} of {t}, which is immutable")
                }
            }
            NoSuchField(name, t) => {
                let t = self.unifier.stringify_with_notes(*t, &mut notes);
                write!(f, "`{t}::{name}` field/method does not exist")
            }
            NoSuchAttribute(name, t) => {
                let t = self.unifier.stringify_with_notes(*t, &mut notes);
                write!(f, "`{t}::{name}` is not a class attribute")
            }
            TupleIndexOutOfBounds { index, len } => {
                write!(
                    f,
                    "Tuple index out of bounds. Got {index} but tuple has only {len} elements"
                )
            }
            RequiresTypeAnn => {
                write!(f, "Unable to infer virtual object type: Type annotation required")
            }
            PolymorphicFunctionPointer => {
                write!(f, "Polymorphic function pointers is not supported")
            }
        }?;
        if let Some(loc) = self.err.loc {
            write!(f, " at {loc}")?;
        }
        let notes = notes.unwrap();
        if !notes.is_empty() {
            write!(f, "\n\nNotes:")?;
            for line in notes.values() {
                write!(f, "\n    {line}")?;
            }
        }
        Ok(())
    }
}
