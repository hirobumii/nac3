use std::{
    collections::{HashMap, HashSet},
    fmt::{Debug, Display},
    rc::Rc,
    sync::Arc,
};

use inkwell::values::{BasicValueEnum, FloatValue, IntValue, PointerValue, StructValue};
use itertools::{Itertools, izip};
use parking_lot::RwLock;

use nac3parser::ast::{Constant, Expr, ExprKind, Location, StrRef};

use crate::{
    codegen::{CodeGenContext, CodeGenerator},
    toplevel::{DefinitionId, TopLevelDef, type_annotation::TypeAnnotation},
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{AttrKind, Type, TypeEnum, Unifier, VarMap},
    },
};

#[derive(Clone, PartialEq, Debug)]
pub enum SymbolValue {
    I32(i32),
    I64(i64),
    U32(u32),
    U64(u64),
    Str(String),
    Double(f64),
    Bool(bool),
    Tuple(Vec<SymbolValue>),
    OptionSome(Box<SymbolValue>),
    OptionNone,
}

impl SymbolValue {
    /// Creates a [`SymbolValue`] from a [`Constant`].
    ///
    /// * `constant` - The constant to create the value from.
    /// * `expected_ty` - The expected type of the [`SymbolValue`].
    pub fn from_constant(
        constant: &Constant,
        expected_ty: Type,
        primitives: &PrimitiveStore,
        unifier: &mut Unifier,
    ) -> Result<Self, String> {
        match constant {
            Constant::None => {
                if unifier.unioned(expected_ty, primitives.option) {
                    Ok(SymbolValue::OptionNone)
                } else {
                    Err(format!("Expected {expected_ty:?}, but got Option"))
                }
            }
            Constant::Bool(b) => {
                if unifier.unioned(expected_ty, primitives.bool) {
                    Ok(SymbolValue::Bool(*b))
                } else {
                    Err(format!("Expected {expected_ty:?}, but got bool"))
                }
            }
            Constant::Str(s) => {
                if unifier.unioned(expected_ty, primitives.str) {
                    Ok(SymbolValue::Str(s.to_string()))
                } else {
                    Err(format!("Expected {expected_ty:?}, but got str"))
                }
            }
            Constant::Int(i) => {
                if unifier.unioned(expected_ty, primitives.int32) {
                    i32::try_from(*i).map(SymbolValue::I32).map_err(|e| e.to_string())
                } else if unifier.unioned(expected_ty, primitives.int64) {
                    i64::try_from(*i).map(SymbolValue::I64).map_err(|e| e.to_string())
                } else if unifier.unioned(expected_ty, primitives.uint32) {
                    u32::try_from(*i).map(SymbolValue::U32).map_err(|e| e.to_string())
                } else if unifier.unioned(expected_ty, primitives.uint64) {
                    u64::try_from(*i).map(SymbolValue::U64).map_err(|e| e.to_string())
                } else {
                    Err(format!("Expected {}, but got int", unifier.stringify(expected_ty)))
                }
            }
            Constant::Tuple(t) => {
                let expected_ty = unifier.get_ty(expected_ty);
                let TypeEnum::TTuple { ty, is_vararg_ctx } = expected_ty.as_ref() else {
                    return Err(format!(
                        "Expected {:?}, but got Tuple",
                        expected_ty.get_type_name()
                    ));
                };

                assert!(*is_vararg_ctx || ty.len() == t.len());

                let elems = t
                    .iter()
                    .zip(ty)
                    .map(|(constant, ty)| Self::from_constant(constant, *ty, primitives, unifier))
                    .collect::<Result<Vec<SymbolValue>, _>>()?;
                Ok(SymbolValue::Tuple(elems))
            }
            Constant::Float(f) => {
                if unifier.unioned(expected_ty, primitives.float) {
                    Ok(SymbolValue::Double(*f))
                } else {
                    Err(format!("Expected {expected_ty:?}, but got float"))
                }
            }
            _ => Err(format!("Unsupported value type {constant:?}")),
        }
    }

    /// Creates a [`SymbolValue`] from a [`Constant`], with its type being inferred from the constant value.
    ///
    /// * `constant` - The constant to create the value from.
    pub fn from_constant_inferred(constant: &Constant) -> Result<Self, String> {
        match constant {
            Constant::None => Ok(SymbolValue::OptionNone),
            Constant::Bool(b) => Ok(SymbolValue::Bool(*b)),
            Constant::Str(s) => Ok(SymbolValue::Str(s.to_string())),
            Constant::Int(i) => {
                let i = *i;
                if i >= 0 {
                    i32::try_from(i)
                        .map(SymbolValue::I32)
                        .or_else(|_| i64::try_from(i).map(SymbolValue::I64))
                        .map_err(|_| {
                            format!("Literal cannot be expressed as any integral type: {i}")
                        })
                } else {
                    u32::try_from(i)
                        .map(SymbolValue::U32)
                        .or_else(|_| u64::try_from(i).map(SymbolValue::U64))
                        .map_err(|_| {
                            format!("Literal cannot be expressed as any integral type: {i}")
                        })
                }
            }
            Constant::Tuple(t) => {
                let elems = t
                    .iter()
                    .map(Self::from_constant_inferred)
                    .collect::<Result<Vec<SymbolValue>, _>>()?;
                Ok(SymbolValue::Tuple(elems))
            }
            Constant::Float(f) => Ok(SymbolValue::Double(*f)),
            _ => Err(format!("Unsupported value type {constant:?}")),
        }
    }

    /// Returns the [`Type`] representing the data type of this value.
    pub fn get_type(&self, primitives: &PrimitiveStore, unifier: &mut Unifier) -> Type {
        match self {
            SymbolValue::I32(_) => primitives.int32,
            SymbolValue::I64(_) => primitives.int64,
            SymbolValue::U32(_) => primitives.uint32,
            SymbolValue::U64(_) => primitives.uint64,
            SymbolValue::Str(_) => primitives.str,
            SymbolValue::Double(_) => primitives.float,
            SymbolValue::Bool(_) => primitives.bool,
            SymbolValue::Tuple(vs) => {
                let vs_tys = vs.iter().map(|v| v.get_type(primitives, unifier)).collect::<Vec<_>>();
                unifier.add_ty(TypeEnum::TTuple { ty: vs_tys, is_vararg_ctx: false })
            }
            SymbolValue::OptionSome(_) | SymbolValue::OptionNone => primitives.option,
        }
    }

    /// Returns the [`TypeAnnotation`] representing the data type of this value.
    pub fn get_type_annotation(
        &self,
        primitives: &PrimitiveStore,
        unifier: &mut Unifier,
    ) -> TypeAnnotation {
        match self {
            SymbolValue::Bool(..)
            | SymbolValue::Double(..)
            | SymbolValue::I32(..)
            | SymbolValue::I64(..)
            | SymbolValue::U32(..)
            | SymbolValue::U64(..)
            | SymbolValue::Str(..) => TypeAnnotation::Primitive(self.get_type(primitives, unifier)),
            SymbolValue::Tuple(vs) => {
                let vs_tys = vs
                    .iter()
                    .map(|v| v.get_type_annotation(primitives, unifier))
                    .collect::<Vec<_>>();
                TypeAnnotation::Tuple(vs_tys)
            }
            SymbolValue::OptionNone => TypeAnnotation::CustomClass {
                id: primitives.option.obj_id(unifier).unwrap(),
                params: Vec::default(),
            },
            SymbolValue::OptionSome(v) => {
                let ty = v.get_type_annotation(primitives, unifier);
                TypeAnnotation::CustomClass {
                    id: primitives.option.obj_id(unifier).unwrap(),
                    params: vec![ty],
                }
            }
        }
    }

    /// Returns the [`TypeEnum`] representing the data type of this value.
    pub fn get_type_enum(
        &self,
        primitives: &PrimitiveStore,
        unifier: &mut Unifier,
    ) -> Rc<TypeEnum> {
        let ty = self.get_type(primitives, unifier);
        unifier.get_ty(ty)
    }
}

impl Display for SymbolValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolValue::I32(i) => write!(f, "{i}"),
            SymbolValue::I64(i) => write!(f, "int64({i})"),
            SymbolValue::U32(i) => write!(f, "uint32({i})"),
            SymbolValue::U64(i) => write!(f, "uint64({i})"),
            SymbolValue::Str(s) => write!(f, "\"{s}\""),
            SymbolValue::Double(d) => write!(f, "{d}"),
            SymbolValue::Bool(b) => {
                if *b {
                    write!(f, "True")
                } else {
                    write!(f, "False")
                }
            }
            SymbolValue::Tuple(t) => {
                write!(f, "({})", t.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(", "))
            }
            SymbolValue::OptionSome(v) => write!(f, "Some({v})"),
            SymbolValue::OptionNone => write!(f, "none"),
        }
    }
}

impl TryFrom<SymbolValue> for u64 {
    type Error = ();

    /// Tries to convert a [`SymbolValue`] into a [`u64`], returning [`Err`] if the value is not
    /// numeric or if the value cannot be converted into a `u64` without overflow.
    fn try_from(value: SymbolValue) -> Result<Self, Self::Error> {
        match value {
            SymbolValue::I32(v) => u64::try_from(v).map_err(|_| ()),
            SymbolValue::I64(v) => u64::try_from(v).map_err(|_| ()),
            SymbolValue::U32(v) => Ok(u64::from(v)),
            SymbolValue::U64(v) => Ok(v),
            _ => Err(()),
        }
    }
}

impl TryFrom<SymbolValue> for i128 {
    type Error = ();

    /// Tries to convert a [`SymbolValue`] into a [`i128`], returning [`Err`] if the value is not
    /// numeric.
    fn try_from(value: SymbolValue) -> Result<Self, Self::Error> {
        match value {
            SymbolValue::I32(v) => Ok(i128::from(v)),
            SymbolValue::I64(v) => Ok(i128::from(v)),
            SymbolValue::U32(v) => Ok(i128::from(v)),
            SymbolValue::U64(v) => Ok(i128::from(v)),
            _ => Err(()),
        }
    }
}

pub trait StaticValue {
    /// Returns a unique identifier for this value.
    fn get_unique_identifier(&self) -> u64;

    /// Returns the constant object represented by this unique identifier.
    fn get_const_obj<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
    ) -> BasicValueEnum<'ctx>;

    /// Converts this value to a LLVM [`BasicValueEnum`].
    fn to_basic_value_enum<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
        expected_ty: Type,
    ) -> Result<BasicValueEnum<'ctx>, String>;

    /// Returns a field within this value.
    fn get_field<'ctx>(
        &self,
        name: StrRef,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Option<ValueEnum<'ctx>>;

    /// Returns a single element of this tuple.
    fn get_tuple_element<'ctx>(&self, index: u32) -> Option<ValueEnum<'ctx>>;
}

#[derive(Clone)]
pub enum ValueEnum<'ctx> {
    /// [`ValueEnum`] representing a static value.
    Static(Arc<dyn StaticValue + Send + Sync>),

    /// [`ValueEnum`] representing a dynamic value.
    Dynamic(BasicValueEnum<'ctx>),
}

impl<'ctx> From<BasicValueEnum<'ctx>> for ValueEnum<'ctx> {
    fn from(v: BasicValueEnum<'ctx>) -> Self {
        ValueEnum::Dynamic(v)
    }
}

impl<'ctx> From<PointerValue<'ctx>> for ValueEnum<'ctx> {
    fn from(v: PointerValue<'ctx>) -> Self {
        ValueEnum::Dynamic(v.into())
    }
}

impl<'ctx> From<IntValue<'ctx>> for ValueEnum<'ctx> {
    fn from(v: IntValue<'ctx>) -> Self {
        ValueEnum::Dynamic(v.into())
    }
}

impl<'ctx> From<FloatValue<'ctx>> for ValueEnum<'ctx> {
    fn from(v: FloatValue<'ctx>) -> Self {
        ValueEnum::Dynamic(v.into())
    }
}

impl<'ctx> From<StructValue<'ctx>> for ValueEnum<'ctx> {
    fn from(v: StructValue<'ctx>) -> Self {
        ValueEnum::Dynamic(v.into())
    }
}

impl<'ctx> ValueEnum<'ctx> {
    /// Converts this [`ValueEnum`] to a [`BasicValueEnum`].
    pub fn to_basic_value_enum<'a>(
        &self,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        generator: &mut dyn CodeGenerator,
        expected_ty: Type,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        match *self {
            ValueEnum::Static(ref v) => v.to_basic_value_enum(ctx, generator, expected_ty),
            ValueEnum::Dynamic(v) => Ok(v),
        }
    }
}

pub trait SymbolResolver {
    /// Get type of type variable identifier or top-level function type,
    fn get_symbol_type(
        &self,
        unifier: &mut Unifier,
        top_level_defs: &[Arc<RwLock<TopLevelDef>>],
        primitives: &PrimitiveStore,
        str: StrRef,
    ) -> Result<Type, String>;

    /// Get the top-level definition of identifiers.
    fn get_identifier_def(&self, str: StrRef) -> Result<DefinitionId, HashSet<String>>;

    fn get_symbol_value<'ctx>(
        &self,
        str: StrRef,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
    ) -> Option<ValueEnum<'ctx>>;

    fn get_default_param_value(&self, expr: &Expr) -> Option<SymbolValue>;
    fn get_string_id(&self, s: &str) -> i32;
    fn get_exception_id(&self, tyid: usize) -> usize;

    fn handle_deferred_eval(
        &self,
        _unifier: &mut Unifier,
        _top_level_defs: &[Arc<RwLock<TopLevelDef>>],
        _primitives: &PrimitiveStore,
    ) -> Result<(), String> {
        Ok(())
    }
}

thread_local! {
    static IDENTIFIER_ID: [StrRef; 11] = [
        "int32".into(),
        "int64".into(),
        "float".into(),
        "bool".into(),
        "virtual".into(),
        "tuple".into(),
        "str".into(),
        "Exception".into(),
        "uint32".into(),
        "uint64".into(),
        "Literal".into(),
    ];
}

/// Converts a type annotation into a [Type].
pub fn parse_type_annotation<T>(
    resolver: &dyn SymbolResolver,
    top_level_defs: &[Arc<RwLock<TopLevelDef>>],
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
    expr: &Expr<T>,
) -> Result<Type, HashSet<String>> {
    let ids = IDENTIFIER_ID.with(|ids| *ids);
    let int32_id = ids[0];
    let int64_id = ids[1];
    let float_id = ids[2];
    let bool_id = ids[3];
    let virtual_id = ids[4];
    let tuple_id = ids[5];
    let str_id = ids[6];
    let exn_id = ids[7];
    let uint32_id = ids[8];
    let uint64_id = ids[9];
    let literal_id = ids[10];

    let name_handling = |id: &StrRef, loc: Location, unifier: &mut Unifier| {
        if *id == int32_id {
            Ok(primitives.int32)
        } else if *id == int64_id {
            Ok(primitives.int64)
        } else if *id == uint32_id {
            Ok(primitives.uint32)
        } else if *id == uint64_id {
            Ok(primitives.uint64)
        } else if *id == float_id {
            Ok(primitives.float)
        } else if *id == bool_id {
            Ok(primitives.bool)
        } else if *id == str_id {
            Ok(primitives.str)
        } else if *id == exn_id {
            Ok(primitives.exception)
        } else {
            let obj_id = resolver.get_identifier_def(*id);
            if let Ok(obj_id) = obj_id {
                let def = top_level_defs[obj_id.0].read();
                if let TopLevelDef::Class { fields, methods, type_vars, .. } = &*def {
                    if !type_vars.is_empty() {
                        return Err(HashSet::from([format!(
                            "Unexpected number of type parameters: expected {} but got 0",
                            type_vars.len()
                        )]));
                    }
                    let fields = fields
                        .iter()
                        .map(|(k, v, m)| (*k, (*v, AttrKind::Field { mutable: *m })))
                        .chain(methods.iter().map(|(k, v, _)| (*k, (*v, AttrKind::Method))))
                        .collect();
                    Ok(unifier.add_ty(TypeEnum::TObj { obj_id, fields, params: VarMap::default() }))
                } else {
                    Err(HashSet::from([format!("Cannot use function name as type at {loc}")]))
                }
            } else {
                let ty =
                    resolver.get_symbol_type(unifier, top_level_defs, primitives, *id).map_err(
                        |e| HashSet::from([format!("Unknown type annotation at {loc}: {e}")]),
                    )?;
                if let TypeEnum::TVar { .. } = &*unifier.get_ty(ty) {
                    Ok(ty)
                } else {
                    Err(HashSet::from([format!("Unknown type annotation {id} at {loc}")]))
                }
            }
        }
    };

    let subscript_name_handle = |id: &StrRef, slice: &Expr<T>, unifier: &mut Unifier| {
        if *id == virtual_id {
            let ty = parse_type_annotation(resolver, top_level_defs, unifier, primitives, slice)?;
            Ok(unifier.add_ty(TypeEnum::TVirtual { ty }))
        } else if *id == tuple_id {
            if let ExprKind::Tuple { elts, .. } = &slice.node {
                let ty = elts
                    .iter()
                    .map(|elt| {
                        parse_type_annotation(resolver, top_level_defs, unifier, primitives, elt)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(unifier.add_ty(TypeEnum::TTuple { ty, is_vararg_ctx: false }))
            } else {
                Err(HashSet::from(["Expected multiple elements for tuple".into()]))
            }
        } else if *id == literal_id {
            let mut parse_literal = |elt: &Expr<T>| {
                let ty = parse_type_annotation(resolver, top_level_defs, unifier, primitives, elt)?;
                let ty_enum = &*unifier.get_ty_immutable(ty);
                match ty_enum {
                    TypeEnum::TLiteral { values, .. } => Ok(values.clone()),
                    _ => Err(HashSet::from([format!(
                        "Expected literal in type argument for Literal at {}",
                        elt.location
                    )])),
                }
            };

            let values = if let ExprKind::Tuple { elts, .. } = &slice.node {
                elts.iter().map(&mut parse_literal).collect::<Result<Vec<_>, _>>()?
            } else {
                vec![parse_literal(slice)?]
            }
            .into_iter()
            .flatten()
            .collect_vec();

            Ok(unifier.get_fresh_literal(values, Some(slice.location)))
        } else {
            let types = if let ExprKind::Tuple { elts, .. } = &slice.node {
                elts.iter()
                    .map(|v| {
                        parse_type_annotation(resolver, top_level_defs, unifier, primitives, v)
                    })
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                vec![parse_type_annotation(resolver, top_level_defs, unifier, primitives, slice)?]
            };

            let obj_id = resolver.get_identifier_def(*id)?;
            let def = top_level_defs[obj_id.0].read();
            if let TopLevelDef::Class { fields, methods, type_vars, .. } = &*def {
                if types.len() != type_vars.len() {
                    return Err(HashSet::from([format!(
                        "Unexpected number of type parameters: expected {} but got {}",
                        type_vars.len(),
                        types.len()
                    )]));
                }
                let mut subst = VarMap::new();
                for (var, ty) in izip!(type_vars.iter(), types.iter()) {
                    let id = if let TypeEnum::TVar { id, .. } = &*unifier.get_ty(*var) {
                        *id
                    } else {
                        unreachable!()
                    };
                    subst.insert(id, *ty);
                }
                let mut fields = fields
                    .iter()
                    .map(|(attr, ty, is_mutable)| {
                        let ty = unifier.subst(*ty, &subst).unwrap_or(*ty);
                        (*attr, (ty, AttrKind::Field { mutable: *is_mutable }))
                    })
                    .collect::<HashMap<_, _>>();
                fields.extend(methods.iter().map(|(attr, ty, _)| {
                    let ty = unifier.subst(*ty, &subst).unwrap_or(*ty);
                    (*attr, (ty, AttrKind::Method))
                }));
                Ok(unifier.add_ty(TypeEnum::TObj { obj_id, fields, params: subst }))
            } else {
                Err(HashSet::from(["Cannot use function name as type".into()]))
            }
        }
    };

    match &expr.node {
        ExprKind::Name { id, .. } => name_handling(id, expr.location, unifier),
        ExprKind::Subscript { value, slice, .. } => {
            if let ExprKind::Name { id, .. } = &value.node {
                subscript_name_handle(id, slice, unifier)
            } else {
                Err(HashSet::from([format!("unsupported type expression at {}", expr.location)]))
            }
        }
        ExprKind::Constant { value, .. } => SymbolValue::from_constant_inferred(value)
            .map(|v| unifier.get_fresh_literal(vec![v], Some(expr.location)))
            .map_err(|err| HashSet::from([err])),
        _ => Err(HashSet::from([format!("unsupported type expression at {}", expr.location)])),
    }
}

impl dyn SymbolResolver + Send + Sync {
    pub fn parse_type_annotation<T>(
        &self,
        top_level_defs: &[Arc<RwLock<TopLevelDef>>],
        unifier: &mut Unifier,
        primitives: &PrimitiveStore,
        expr: &Expr<T>,
    ) -> Result<Type, HashSet<String>> {
        parse_type_annotation(self, top_level_defs, unifier, primitives, expr)
    }

    pub fn get_type_name(
        &self,
        top_level_defs: &[Arc<RwLock<TopLevelDef>>],
        unifier: &mut Unifier,
        ty: Type,
    ) -> String {
        unifier.internal_stringify(
            ty,
            &mut |id| {
                let top_level_def = &*top_level_defs[id].read();
                let (TopLevelDef::Class { name, .. } | TopLevelDef::Module { name, .. }) =
                    top_level_def
                else {
                    unreachable!("expected class/module definition")
                };
                name.to_string()
            },
            &mut |id| format!("typevar{id}"),
            &mut None,
        )
    }
}

impl Debug for dyn SymbolResolver + Send + Sync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "")
    }
}
