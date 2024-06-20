use std::iter::once;

use helper::{debug_assert_prim_is_allowed, make_exception_fields, PrimDefDetails};
use indexmap::IndexMap;
use inkwell::{
    attributes::{Attribute, AttributeLoc},
    types::{BasicMetadataTypeEnum, BasicType},
    values::{BasicMetadataValueEnum, BasicValue, CallSiteValue},
    IntPredicate,
};
use itertools::Either;
use strum::IntoEnumIterator;

use crate::{
    codegen::{
        builtin_fns,
        classes::{
            ArrayLikeValue, NDArrayValue, ProxyType, ProxyValue, RangeType, RangeValue,
            TypedArrayLikeAccessor,
        },
        expr::destructure_range,
        irrt::*,
        numpy::*,
        stmt::exn_constructor,
    },
    symbol_resolver::SymbolValue,
    toplevel::{helper::PrimDef, numpy::make_ndarray_ty},
    typecheck::typedef::{into_var_map, iter_type_vars, TypeVar, VarMap},
};

use super::*;

type BuiltinInfo = Vec<(Arc<RwLock<TopLevelDef>>, Option<Stmt>)>;

pub fn get_exn_constructor(
    name: &str,
    class_id: usize,
    cons_id: usize,
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
) -> (TopLevelDef, TopLevelDef, Type, Type) {
    let int32 = primitives.int32;
    let int64 = primitives.int64;
    let string = primitives.str;
    let exception_fields = make_exception_fields(int32, int64, string);
    let exn_cons_args = vec![
        FuncArg {
            name: "msg".into(),
            ty: string,
            default_value: Some(SymbolValue::Str(String::new())),
        },
        FuncArg { name: "param0".into(), ty: int64, default_value: Some(SymbolValue::I64(0)) },
        FuncArg { name: "param1".into(), ty: int64, default_value: Some(SymbolValue::I64(0)) },
        FuncArg { name: "param2".into(), ty: int64, default_value: Some(SymbolValue::I64(0)) },
    ];
    let exn_type = unifier.add_ty(TypeEnum::TObj {
        obj_id: DefinitionId(class_id),
        fields: exception_fields
            .clone()
            .into_iter()
            .map(|(name, ty, mutable)| (name, (ty, mutable)))
            .collect(),
        params: VarMap::default(),
    });
    let signature = unifier.add_ty(TypeEnum::TFunc(FunSignature {
        args: exn_cons_args,
        ret: exn_type,
        vars: VarMap::default(),
    }));
    let fun_def = TopLevelDef::Function {
        name: format!("{name}.__init__"),
        simple_name: "__init__".into(),
        signature,
        var_id: Vec::default(),
        instance_to_symbol: HashMap::default(),
        instance_to_stmt: HashMap::default(),
        resolver: None,
        codegen_callback: Some(Arc::new(GenCall::new(Box::new(exn_constructor)))),
        loc: None,
    };
    let class_def = TopLevelDef::Class {
        name: name.into(),
        object_id: DefinitionId(class_id),
        type_vars: Vec::default(),
        fields: exception_fields,
        methods: vec![("__init__".into(), signature, DefinitionId(cons_id))],
        ancestors: vec![
            TypeAnnotation::CustomClass { id: DefinitionId(class_id), params: Vec::default() },
            TypeAnnotation::CustomClass { id: PrimDef::Exception.id(), params: Vec::default() },
        ],
        constructor: Some(signature),
        resolver: None,
        loc: None,
    };
    (fun_def, class_def, signature, exn_type)
}

/// Creates a NumPy [`TopLevelDef`] function by code generation.
///
/// * `name`: The name of the implemented NumPy function.
/// * `ret_ty`: The return type of this function.
/// * `param_ty`: The parameters accepted by this function, represented by a tuple of the
/// [parameter type][Type] and the parameter symbol name.
/// * `codegen_callback`: A lambda generating LLVM IR for the implementation of this function.
fn create_fn_by_codegen(
    unifier: &mut Unifier,
    var_map: &VarMap,
    name: &'static str,
    ret_ty: Type,
    param_ty: &[(Type, &'static str)],
    codegen_callback: Box<GenCallCallback>,
) -> TopLevelDef {
    TopLevelDef::Function {
        name: name.into(),
        simple_name: name.into(),
        signature: unifier.add_ty(TypeEnum::TFunc(FunSignature {
            args: param_ty
                .iter()
                .map(|p| FuncArg { name: p.1.into(), ty: p.0, default_value: None })
                .collect(),
            ret: ret_ty,
            vars: var_map.clone(),
        })),
        var_id: Vec::default(),
        instance_to_symbol: HashMap::default(),
        instance_to_stmt: HashMap::default(),
        resolver: None,
        codegen_callback: Some(Arc::new(GenCall::new(codegen_callback))),
        loc: None,
    }
}

/// Creates a NumPy [`TopLevelDef`] function using an LLVM intrinsic.
///
/// * `name`: The name of the implemented NumPy function.
/// * `ret_ty`: The return type of this function.
/// * `param_ty`: The parameters accepted by this function, represented by a tuple of the
/// [parameter type][Type] and the parameter symbol name.
/// * `intrinsic_fn`: The fully-qualified name of the LLVM intrinsic function.
fn create_fn_by_intrinsic(
    unifier: &mut Unifier,
    var_map: &VarMap,
    name: &'static str,
    ret_ty: Type,
    params: &[(Type, &'static str)],
    intrinsic_fn: &'static str,
) -> TopLevelDef {
    let param_tys = params.iter().map(|p| p.0).collect_vec();

    create_fn_by_codegen(
        unifier,
        var_map,
        name,
        ret_ty,
        params,
        Box::new(move |ctx, _, fun, args, generator| {
            let args_ty = fun.0.args.iter().map(|a| a.ty).collect_vec();

            assert!(param_tys
                .iter()
                .zip(&args_ty)
                .all(|(expected, actual)| ctx.unifier.unioned(*expected, *actual)));

            let args_val = args_ty
                .iter()
                .zip_eq(args.iter())
                .map(|(ty, arg)| arg.1.clone().to_basic_value_enum(ctx, generator, *ty).unwrap())
                .map_into::<BasicMetadataValueEnum>()
                .collect_vec();

            let intrinsic_fn = ctx.module.get_function(intrinsic_fn).unwrap_or_else(|| {
                let ret_llvm_ty = ctx.get_llvm_abi_type(generator, ret_ty);
                let param_llvm_ty = param_tys
                    .iter()
                    .map(|p| ctx.get_llvm_abi_type(generator, *p))
                    .map_into::<BasicMetadataTypeEnum>()
                    .collect_vec();
                let fn_type = ret_llvm_ty.fn_type(param_llvm_ty.as_slice(), false);

                ctx.module.add_function(intrinsic_fn, fn_type, None)
            });

            let val = ctx
                .builder
                .build_call(intrinsic_fn, args_val.as_slice(), name)
                .map(CallSiteValue::try_as_basic_value)
                .map(Either::unwrap_left)
                .unwrap();
            Ok(val.into())
        }),
    )
}

/// Creates a unary NumPy [`TopLevelDef`] function using an extern function (e.g. from `libc` or
/// `libm`).
///
/// * `name`: The name of the implemented NumPy function.
/// * `ret_ty`: The return type of this function.
/// * `param_ty`: The parameters accepted by this function, represented by a tuple of the
/// [parameter type][Type] and the parameter symbol name.
/// * `extern_fn`: The fully-qualified name of the extern function used as the implementation.
/// * `attrs`: The list of attributes to apply to this function declaration. Note that `nounwind` is
/// already implied by the C ABI.
fn create_fn_by_extern(
    unifier: &mut Unifier,
    var_map: &VarMap,
    name: &'static str,
    ret_ty: Type,
    params: &[(Type, &'static str)],
    extern_fn: &'static str,
    attrs: &'static [&str],
) -> TopLevelDef {
    let param_tys = params.iter().map(|p| p.0).collect_vec();

    create_fn_by_codegen(
        unifier,
        var_map,
        name,
        ret_ty,
        params,
        Box::new(move |ctx, _, fun, args, generator| {
            let args_ty = fun.0.args.iter().map(|a| a.ty).collect_vec();

            assert!(param_tys
                .iter()
                .zip(&args_ty)
                .all(|(expected, actual)| ctx.unifier.unioned(*expected, *actual)));

            let args_val = args_ty
                .iter()
                .zip_eq(args.iter())
                .map(|(ty, arg)| arg.1.clone().to_basic_value_enum(ctx, generator, *ty).unwrap())
                .map_into::<BasicMetadataValueEnum>()
                .collect_vec();

            let intrinsic_fn = ctx.module.get_function(extern_fn).unwrap_or_else(|| {
                let ret_llvm_ty = ctx.get_llvm_abi_type(generator, ret_ty);
                let param_llvm_ty = param_tys
                    .iter()
                    .map(|p| ctx.get_llvm_abi_type(generator, *p))
                    .map_into::<BasicMetadataTypeEnum>()
                    .collect_vec();
                let fn_type = ret_llvm_ty.fn_type(param_llvm_ty.as_slice(), false);
                let func = ctx.module.add_function(extern_fn, fn_type, None);
                func.add_attribute(
                    AttributeLoc::Function,
                    ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
                );

                for attr in attrs {
                    func.add_attribute(
                        AttributeLoc::Function,
                        ctx.ctx.create_enum_attribute(Attribute::get_named_enum_kind_id(attr), 0),
                    );
                }

                func
            });

            let val = ctx
                .builder
                .build_call(intrinsic_fn, &args_val, name)
                .map(CallSiteValue::try_as_basic_value)
                .map(Either::unwrap_left)
                .unwrap();
            Ok(val.into())
        }),
    )
}

pub fn get_builtins(unifier: &mut Unifier, primitives: &PrimitiveStore) -> BuiltinInfo {
    BuiltinBuilder::new(unifier, primitives)
        .build_all_builtins()
        .into_iter()
        .map(|tld| {
            let tld = Arc::new(RwLock::new(tld));
            let ast = None;
            (tld, ast)
        })
        .collect()
}

/// A helper enum used by [`BuiltinBuilder`]
#[derive(Clone, Copy)]
enum SizeVariant {
    Bits32,
    Bits64,
}

impl SizeVariant {
    fn of_int(self, primitives: &PrimitiveStore) -> Type {
        match self {
            SizeVariant::Bits32 => primitives.int32,
            SizeVariant::Bits64 => primitives.int64,
        }
    }
}

struct BuiltinBuilder<'a> {
    unifier: &'a mut Unifier,
    primitives: &'a PrimitiveStore,

    is_some_ty: (Type, bool),
    unwrap_ty: (Type, bool),
    option_tvar: TypeVar,

    ndarray_dtype_tvar: TypeVar,
    ndarray_ndims_tvar: TypeVar,
    ndarray_copy_ty: (Type, bool),
    ndarray_fill_ty: (Type, bool),

    list_int32: Type,

    num_ty: TypeVar,
    num_var_map: VarMap,

    ndarray_float: Type,
    ndarray_float_2d: Type,
    ndarray_num_ty: Type,

    float_or_ndarray_ty: TypeVar,
    float_or_ndarray_var_map: VarMap,

    num_or_ndarray_ty: TypeVar,
    num_or_ndarray_var_map: VarMap,
}

impl<'a> BuiltinBuilder<'a> {
    fn new(unifier: &'a mut Unifier, primitives: &'a PrimitiveStore) -> BuiltinBuilder<'a> {
        let PrimitiveStore {
            int32,
            int64,
            uint32,
            uint64,
            float,
            bool: boolean,
            ndarray,
            option,
            ..
        } = *primitives;

        // Option-related
        let (is_some_ty, unwrap_ty, option_tvar) =
            if let TypeEnum::TObj { fields, params, .. } = unifier.get_ty(option).as_ref() {
                (
                    *fields.get(&PrimDef::OptionIsSome.simple_name().into()).unwrap(),
                    *fields.get(&PrimDef::OptionUnwrap.simple_name().into()).unwrap(),
                    iter_type_vars(params).next().unwrap(),
                )
            } else {
                unreachable!()
            };

        let TypeEnum::TObj { fields: ndarray_fields, params: ndarray_params, .. } =
            &*unifier.get_ty(ndarray)
        else {
            unreachable!()
        };
        let ndarray_dtype_tvar = iter_type_vars(ndarray_params).next().unwrap();
        let ndarray_ndims_tvar = iter_type_vars(ndarray_params).nth(1).unwrap();
        let ndarray_copy_ty =
            *ndarray_fields.get(&PrimDef::NDArrayCopy.simple_name().into()).unwrap();
        let ndarray_fill_ty =
            *ndarray_fields.get(&PrimDef::NDArrayFill.simple_name().into()).unwrap();

        let num_ty = unifier.get_fresh_var_with_range(
            &[int32, int64, float, boolean, uint32, uint64],
            Some("N".into()),
            None,
        );
        let num_var_map = into_var_map([num_ty]);

        let ndarray_float = make_ndarray_ty(unifier, primitives, Some(float), None);
        let ndarray_float_2d = {
            let value = match primitives.size_t {
                64 => SymbolValue::U64(2u64),
                32 => SymbolValue::U32(2u32),
                _ => unreachable!(),
            };
            let ndims = unifier.add_ty(TypeEnum::TLiteral { values: vec![value], loc: None });

            make_ndarray_ty(unifier, primitives, Some(float), Some(ndims))
        };

        let ndarray_num_ty = make_ndarray_ty(unifier, primitives, Some(num_ty.ty), None);
        let float_or_ndarray_ty =
            unifier.get_fresh_var_with_range(&[float, ndarray_float], Some("T".into()), None);
        let float_or_ndarray_var_map = into_var_map([float_or_ndarray_ty]);

        let num_or_ndarray_ty =
            unifier.get_fresh_var_with_range(&[num_ty.ty, ndarray_num_ty], Some("T".into()), None);
        let num_or_ndarray_var_map = into_var_map([num_ty, num_or_ndarray_ty]);

        let list_int32 = unifier.add_ty(TypeEnum::TList { ty: int32 });

        BuiltinBuilder {
            unifier,
            primitives,

            is_some_ty,
            unwrap_ty,
            option_tvar,

            ndarray_dtype_tvar,
            ndarray_ndims_tvar,
            ndarray_copy_ty,
            ndarray_fill_ty,

            list_int32,

            num_ty,
            num_var_map,

            ndarray_float,
            ndarray_float_2d,
            ndarray_num_ty,

            float_or_ndarray_ty,
            float_or_ndarray_var_map,

            num_or_ndarray_ty,
            num_or_ndarray_var_map,
        }
    }

    /// Construct every function from every [`PrimDef`], in the order of [`PrimDef`]'s definition.
    fn build_all_builtins(&mut self) -> Vec<TopLevelDef> {
        PrimDef::iter().map(|prim| self.build_builtin_of_prim(prim)).collect_vec()
    }

    /// Build the [`TopLevelDef`] associated of a [`PrimDef`].
    fn build_builtin_of_prim(&mut self, prim: PrimDef) -> TopLevelDef {
        let tld = match prim {
            PrimDef::Int32
            | PrimDef::Int64
            | PrimDef::UInt32
            | PrimDef::UInt64
            | PrimDef::Float
            | PrimDef::Bool
            | PrimDef::Str
            | PrimDef::Range
            | PrimDef::None => Self::build_simple_primitive_class(prim),

            PrimDef::Exception => self.build_exception_class_related(prim),

            PrimDef::Option
            | PrimDef::OptionIsSome
            | PrimDef::OptionIsNone
            | PrimDef::OptionUnwrap
            | PrimDef::FunSome => self.build_option_class_related(prim),

            PrimDef::NDArray | PrimDef::NDArrayCopy | PrimDef::NDArrayFill => {
                self.build_ndarray_class_related(prim)
            }

            PrimDef::FunInt32
            | PrimDef::FunInt64
            | PrimDef::FunUInt32
            | PrimDef::FunUInt64
            | PrimDef::FunFloat
            | PrimDef::FunBool => self.build_cast_function(prim),

            PrimDef::FunNpNDArray
            | PrimDef::FunNpEmpty
            | PrimDef::FunNpZeros
            | PrimDef::FunNpOnes => self.build_ndarray_from_shape_factory_function(prim),

            PrimDef::FunNpArray
            | PrimDef::FunNpFull
            | PrimDef::FunNpEye
            | PrimDef::FunNpIdentity => self.build_ndarray_other_factory_function(prim),

            PrimDef::FunRange => self.build_range_function(),
            PrimDef::FunStr => self.build_str_function(),

            PrimDef::FunFloor | PrimDef::FunFloor64 | PrimDef::FunCeil | PrimDef::FunCeil64 => {
                self.build_ceil_floor_function(prim)
            }

            PrimDef::FunAbs => self.build_abs_function(),

            PrimDef::FunRound | PrimDef::FunRound64 => self.build_round_function(prim),

            PrimDef::FunNpFloor | PrimDef::FunNpCeil => self.build_np_ceil_floor_function(prim),

            PrimDef::FunNpRound => self.build_np_round_function(),

            PrimDef::FunLen => self.build_len_function(),

            PrimDef::FunMin | PrimDef::FunMax => self.build_min_max_function(prim),

            PrimDef::FunNpMin | PrimDef::FunNpMax => self.build_np_min_max_function(prim),

            PrimDef::FunNpMinimum | PrimDef::FunNpMaximum => {
                self.build_np_minimum_maximum_function(prim)
            }

            PrimDef::FunNpIsNan | PrimDef::FunNpIsInf => self.build_np_float_to_bool_function(prim),

            PrimDef::FunNpAny | PrimDef::FunNpAll => self.build_np_any_all_function(prim),

            PrimDef::FunNpSin
            | PrimDef::FunNpCos
            | PrimDef::FunNpTan
            | PrimDef::FunNpArcsin
            | PrimDef::FunNpArccos
            | PrimDef::FunNpArctan
            | PrimDef::FunNpSinh
            | PrimDef::FunNpCosh
            | PrimDef::FunNpTanh
            | PrimDef::FunNpArcsinh
            | PrimDef::FunNpArccosh
            | PrimDef::FunNpArctanh
            | PrimDef::FunNpExp
            | PrimDef::FunNpExp2
            | PrimDef::FunNpExpm1
            | PrimDef::FunNpLog
            | PrimDef::FunNpLog2
            | PrimDef::FunNpLog10
            | PrimDef::FunNpSqrt
            | PrimDef::FunNpCbrt
            | PrimDef::FunNpFabs
            | PrimDef::FunNpRint
            | PrimDef::FunSpSpecErf
            | PrimDef::FunSpSpecErfc
            | PrimDef::FunSpSpecGamma
            | PrimDef::FunSpSpecGammaln
            | PrimDef::FunSpSpecJ0
            | PrimDef::FunSpSpecJ1 => self.build_np_sp_float_or_ndarray_1ary_function(prim),

            PrimDef::FunNpArctan2
            | PrimDef::FunNpCopysign
            | PrimDef::FunNpFmax
            | PrimDef::FunNpFmin
            | PrimDef::FunNpLdExp
            | PrimDef::FunNpHypot
            | PrimDef::FunNpNextAfter => self.build_np_2ary_function(prim),
        };

        if cfg!(debug_assertions) {
            // Sanity checks on the constructed [`TopLevelDef`]

            match (&tld, prim.details()) {
                (
                    TopLevelDef::Class { name, object_id, .. },
                    PrimDefDetails::PrimClass { name: exp_name },
                ) => {
                    let exp_object_id = prim.id();
                    assert_eq!(name, &exp_name.into());
                    assert_eq!(object_id, &exp_object_id);
                }
                (
                    TopLevelDef::Function { name, simple_name, .. },
                    PrimDefDetails::PrimFunction { name: exp_name, simple_name: exp_simple_name },
                ) => {
                    assert_eq!(name, exp_name);
                    assert_eq!(simple_name, &exp_simple_name.into());
                }
                _ => {
                    panic!("Class/function variant of the constructed TopLevelDef of PrimDef {prim:?} is different than what is defined by {prim:?}")
                }
            }
        }

        tld
    }

    /// Build "simple" primitive classes.
    fn build_simple_primitive_class(prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(
            prim,
            &[
                PrimDef::Int32,
                PrimDef::Int64,
                PrimDef::UInt32,
                PrimDef::UInt64,
                PrimDef::Float,
                PrimDef::Bool,
                PrimDef::Str,
                PrimDef::Range,
                PrimDef::None,
            ],
        );

        TopLevelComposer::make_top_level_class_def(prim.id(), None, prim.name().into(), None, None)
    }

    /// Build the class `Exception` and its associated methods.
    fn build_exception_class_related(&self, prim: PrimDef) -> TopLevelDef {
        // NOTE: currently only contains the class `Exception`
        debug_assert_prim_is_allowed(prim, &[PrimDef::Exception]);

        let PrimitiveStore { int32, int64, str, .. } = *self.primitives;

        match prim {
            PrimDef::Exception => TopLevelDef::Class {
                name: prim.name().into(),
                object_id: prim.id(),
                type_vars: Vec::default(),
                fields: make_exception_fields(int32, int64, str),
                methods: Vec::default(),
                ancestors: vec![],
                constructor: None,
                resolver: None,
                loc: None,
            },
            _ => unreachable!(),
        }
    }

    /// Build the class `Option`, its associated methods and the function `Some()`.
    fn build_option_class_related(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(
            prim,
            &[
                PrimDef::Option,
                PrimDef::OptionIsSome,
                PrimDef::OptionIsNone,
                PrimDef::OptionUnwrap,
                PrimDef::FunSome,
            ],
        );

        match prim {
            PrimDef::Option => TopLevelDef::Class {
                name: prim.name().into(),
                object_id: prim.id(),
                type_vars: vec![self.option_tvar.ty],
                fields: vec![],
                methods: vec![
                    Self::create_method(PrimDef::OptionIsSome, self.is_some_ty.0),
                    Self::create_method(PrimDef::OptionIsNone, self.is_some_ty.0),
                    Self::create_method(PrimDef::OptionUnwrap, self.unwrap_ty.0),
                ],
                ancestors: vec![TypeAnnotation::CustomClass {
                    id: prim.id(),
                    params: Vec::default(),
                }],
                constructor: None,
                resolver: None,
                loc: None,
            },

            PrimDef::OptionUnwrap => TopLevelDef::Function {
                name: prim.name().into(),
                simple_name: prim.simple_name().into(),
                signature: self.unwrap_ty.0,
                var_id: vec![self.option_tvar.id],
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::create_dummy(String::from(
                    "handled in gen_expr",
                )))),
                loc: None,
            },

            PrimDef::OptionIsNone | PrimDef::OptionIsSome => TopLevelDef::Function {
                name: prim.name().to_string(),
                simple_name: prim.simple_name().into(),
                signature: self.is_some_ty.0,
                var_id: vec![self.option_tvar.id],
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                    move |ctx, obj, _, _, generator| {
                        let expect_ty = obj.clone().unwrap().0;
                        let obj_val = obj
                            .unwrap()
                            .1
                            .clone()
                            .to_basic_value_enum(ctx, generator, expect_ty)?;
                        let BasicValueEnum::PointerValue(ptr) = obj_val else {
                            unreachable!("option must be ptr")
                        };

                        let returned_int = match prim {
                            PrimDef::OptionIsNone => {
                                ctx.builder.build_is_null(ptr, prim.simple_name())
                            }
                            PrimDef::OptionIsSome => {
                                ctx.builder.build_is_not_null(ptr, prim.simple_name())
                            }
                            _ => unreachable!(),
                        };
                        Ok(Some(returned_int.map(Into::into).unwrap()))
                    },
                )))),
                loc: None,
            },

            PrimDef::FunSome => TopLevelDef::Function {
                name: prim.name().into(),
                simple_name: prim.simple_name().into(),
                signature: self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                    args: vec![FuncArg {
                        name: "n".into(),
                        ty: self.option_tvar.ty,
                        default_value: None,
                    }],
                    ret: self.primitives.option,
                    vars: into_var_map([self.option_tvar]),
                })),
                var_id: vec![self.option_tvar.id],
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                    |ctx, _, fun, args, generator| {
                        let arg_ty = fun.0.args[0].ty;
                        let arg_val =
                            args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;
                        let alloca = generator
                            .gen_var_alloc(ctx, arg_val.get_type(), Some("alloca_some"))
                            .unwrap();
                        ctx.builder.build_store(alloca, arg_val).unwrap();
                        Ok(Some(alloca.into()))
                    },
                )))),
                loc: None,
            },

            _ => {
                unreachable!()
            }
        }
    }

    /// Build the class `ndarray` and its associated methods.
    fn build_ndarray_class_related(&self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(
            prim,
            &[PrimDef::NDArray, PrimDef::NDArrayCopy, PrimDef::NDArrayFill],
        );

        match prim {
            PrimDef::NDArray => TopLevelDef::Class {
                name: prim.name().into(),
                object_id: prim.id(),
                type_vars: vec![self.ndarray_dtype_tvar.ty, self.ndarray_ndims_tvar.ty],
                fields: Vec::default(),
                methods: vec![
                    Self::create_method(PrimDef::NDArrayCopy, self.ndarray_copy_ty.0),
                    Self::create_method(PrimDef::NDArrayFill, self.ndarray_fill_ty.0),
                ],
                ancestors: Vec::default(),
                constructor: None,
                resolver: None,
                loc: None,
            },

            PrimDef::NDArrayCopy => TopLevelDef::Function {
                name: prim.name().into(),
                simple_name: prim.simple_name().into(),
                signature: self.ndarray_copy_ty.0,
                var_id: vec![self.ndarray_dtype_tvar.id, self.ndarray_ndims_tvar.id],
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                    |ctx, obj, fun, args, generator| {
                        gen_ndarray_copy(ctx, &obj, fun, &args, generator)
                            .map(|val| Some(val.as_basic_value_enum()))
                    },
                )))),
                loc: None,
            },

            PrimDef::NDArrayFill => TopLevelDef::Function {
                name: prim.name().into(),
                simple_name: prim.simple_name().into(),
                signature: self.ndarray_fill_ty.0,
                var_id: vec![self.ndarray_dtype_tvar.id, self.ndarray_ndims_tvar.id],
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                    |ctx, obj, fun, args, generator| {
                        gen_ndarray_fill(ctx, &obj, fun, &args, generator)?;
                        Ok(None)
                    },
                )))),
                loc: None,
            },

            _ => unreachable!(),
        }
    }

    /// Build functions that cast a numeric primitive to another numeric primitive, including booleans.
    fn build_cast_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(
            prim,
            &[
                PrimDef::FunInt32,
                PrimDef::FunInt64,
                PrimDef::FunUInt32,
                PrimDef::FunUInt64,
                PrimDef::FunFloat,
                PrimDef::FunBool,
            ],
        );

        TopLevelDef::Function {
            name: prim.name().into(),
            simple_name: prim.simple_name().into(),
            signature: self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg {
                    name: "n".into(),
                    ty: self.num_or_ndarray_ty.ty,
                    default_value: None,
                }],
                ret: self.num_or_ndarray_ty.ty,
                vars: self.num_or_ndarray_var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                move |ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;

                    let func = match prim {
                        PrimDef::FunInt32 => builtin_fns::call_int32,
                        PrimDef::FunInt64 => builtin_fns::call_int64,
                        PrimDef::FunUInt32 => builtin_fns::call_uint32,
                        PrimDef::FunUInt64 => builtin_fns::call_uint64,
                        PrimDef::FunFloat => builtin_fns::call_float,
                        PrimDef::FunBool => builtin_fns::call_bool,
                        _ => unreachable!(),
                    };
                    Ok(Some(func(generator, ctx, (arg_ty, arg))?))
                },
            )))),
            loc: None,
        }
    }

    /// Build the functions `round()` and `round64()`.
    fn build_round_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(prim, &[PrimDef::FunRound, PrimDef::FunRound64]);

        let float = self.primitives.float;

        let size_variant = match prim {
            PrimDef::FunRound => SizeVariant::Bits32,
            PrimDef::FunRound64 => SizeVariant::Bits64,
            _ => unreachable!(),
        };

        let common_ndim = self.unifier.get_fresh_const_generic_var(
            self.primitives.usize(),
            Some("N".into()),
            None,
        );

        // The size variant of the function determines the size of the returned int.
        let int_sized = size_variant.of_int(self.primitives);

        let ndarray_int_sized =
            make_ndarray_ty(self.unifier, self.primitives, Some(int_sized), Some(common_ndim.ty));
        let ndarray_float =
            make_ndarray_ty(self.unifier, self.primitives, Some(float), Some(common_ndim.ty));

        let p0_ty =
            self.unifier.get_fresh_var_with_range(&[float, ndarray_float], Some("T".into()), None);
        let ret_ty = self.unifier.get_fresh_var_with_range(
            &[int_sized, ndarray_int_sized],
            Some("R".into()),
            None,
        );

        create_fn_by_codegen(
            self.unifier,
            &into_var_map([common_ndim, p0_ty, ret_ty]),
            prim.name(),
            ret_ty.ty,
            &[(p0_ty.ty, "n")],
            Box::new(move |ctx, _, fun, args, generator| {
                let arg_ty = fun.0.args[0].ty;
                let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;

                let ret_elem_ty = size_variant.of_int(&ctx.primitives);
                Ok(Some(builtin_fns::call_round(generator, ctx, (arg_ty, arg), ret_elem_ty)?))
            }),
        )
    }

    /// Build the functions `ceil()` and `floor()` and their 64 bit variants.
    fn build_ceil_floor_function(&mut self, prim: PrimDef) -> TopLevelDef {
        #[derive(Clone, Copy)]
        enum Kind {
            Floor,
            Ceil,
        }

        debug_assert_prim_is_allowed(
            prim,
            &[PrimDef::FunFloor, PrimDef::FunFloor64, PrimDef::FunCeil, PrimDef::FunCeil64],
        );

        let (size_variant, kind) = {
            match prim {
                PrimDef::FunFloor => (SizeVariant::Bits32, Kind::Floor),
                PrimDef::FunFloor64 => (SizeVariant::Bits64, Kind::Floor),
                PrimDef::FunCeil => (SizeVariant::Bits32, Kind::Ceil),
                PrimDef::FunCeil64 => (SizeVariant::Bits64, Kind::Ceil),
                _ => unreachable!(),
            }
        };

        let float = self.primitives.float;

        let common_ndim = self.unifier.get_fresh_const_generic_var(
            self.primitives.usize(),
            Some("N".into()),
            None,
        );

        let ndarray_float =
            make_ndarray_ty(self.unifier, self.primitives, Some(float), Some(common_ndim.ty));

        // The size variant of the function determines the type of int returned
        let int_sized = size_variant.of_int(self.primitives);
        let ndarray_int_sized =
            make_ndarray_ty(self.unifier, self.primitives, Some(int_sized), Some(common_ndim.ty));

        let p0_ty =
            self.unifier.get_fresh_var_with_range(&[float, ndarray_float], Some("T".into()), None);

        let ret_ty = self.unifier.get_fresh_var_with_range(
            &[int_sized, ndarray_int_sized],
            Some("R".into()),
            None,
        );

        create_fn_by_codegen(
            self.unifier,
            &into_var_map([common_ndim, p0_ty, ret_ty]),
            prim.name(),
            ret_ty.ty,
            &[(p0_ty.ty, "n")],
            Box::new(move |ctx, _, fun, args, generator| {
                let arg_ty = fun.0.args[0].ty;
                let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;

                let ret_elem_ty = size_variant.of_int(&ctx.primitives);
                let func = match kind {
                    Kind::Ceil => builtin_fns::call_ceil,
                    Kind::Floor => builtin_fns::call_floor,
                };
                Ok(Some(func(generator, ctx, (arg_ty, arg), ret_elem_ty)?))
            }),
        )
    }

    /// Build ndarray factory functions that only take in an argument `shape` of type `list[int32]` and return an ndarray.
    fn build_ndarray_from_shape_factory_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(
            prim,
            &[PrimDef::FunNpNDArray, PrimDef::FunNpEmpty, PrimDef::FunNpZeros, PrimDef::FunNpOnes],
        );

        create_fn_by_codegen(
            self.unifier,
            &VarMap::new(),
            prim.name(),
            self.ndarray_float,
            // We are using List[int32] here, as I don't know a way to specify an n-tuple bound on a
            // type variable
            &[(self.list_int32, "shape")],
            Box::new(move |ctx, obj, fun, args, generator| {
                let func = match prim {
                    PrimDef::FunNpNDArray | PrimDef::FunNpEmpty => gen_ndarray_empty,
                    PrimDef::FunNpZeros => gen_ndarray_zeros,
                    PrimDef::FunNpOnes => gen_ndarray_ones,
                    _ => unreachable!(),
                };
                func(ctx, &obj, fun, &args, generator).map(|val| Some(val.as_basic_value_enum()))
            }),
        )
    }

    /// Build ndarray factory functions that do not fit in any other `build_ndarray_*_factory_function` categories in [`BuiltinBuilder`].
    ///
    /// See also [`BuiltinBuilder::build_ndarray_from_shape_factory_function`].
    fn build_ndarray_other_factory_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(
            prim,
            &[PrimDef::FunNpArray, PrimDef::FunNpFull, PrimDef::FunNpEye, PrimDef::FunNpIdentity],
        );

        let PrimitiveStore { int32, bool, ndarray, .. } = *self.primitives;

        match prim {
            PrimDef::FunNpArray => {
                let tv = self.unifier.get_fresh_var(Some("T".into()), None);

                TopLevelDef::Function {
                    name: prim.name().into(),
                    simple_name: prim.simple_name().into(),
                    signature: self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                        args: vec![
                            FuncArg { name: "object".into(), ty: tv.ty, default_value: None },
                            FuncArg {
                                name: "copy".into(),
                                ty: bool,
                                default_value: Some(SymbolValue::Bool(true)),
                            },
                            FuncArg {
                                name: "ndmin".into(),
                                ty: int32,
                                default_value: Some(SymbolValue::U32(0)),
                            },
                        ],
                        ret: ndarray,
                        vars: into_var_map([tv]),
                    })),
                    var_id: vec![tv.id],
                    instance_to_symbol: HashMap::default(),
                    instance_to_stmt: HashMap::default(),
                    resolver: None,
                    codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                        |ctx, obj, fun, args, generator| {
                            gen_ndarray_array(ctx, &obj, fun, &args, generator)
                                .map(|val| Some(val.as_basic_value_enum()))
                        },
                    )))),
                    loc: None,
                }
            }
            PrimDef::FunNpFull => {
                let tv = self.unifier.get_fresh_var(Some("T".into()), None);

                create_fn_by_codegen(
                    self.unifier,
                    &into_var_map([tv]),
                    prim.name(),
                    self.primitives.ndarray,
                    // We are using List[int32] here, as I don't know a way to specify an n-tuple bound on a
                    // type variable
                    &[(self.list_int32, "shape"), (tv.ty, "fill_value")],
                    Box::new(move |ctx, obj, fun, args, generator| {
                        gen_ndarray_full(ctx, &obj, fun, &args, generator)
                            .map(|val| Some(val.as_basic_value_enum()))
                    }),
                )
            }
            PrimDef::FunNpEye => {
                TopLevelDef::Function {
                    name: prim.name().into(),
                    simple_name: prim.simple_name().into(),
                    signature: self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                        args: vec![
                            FuncArg { name: "N".into(), ty: int32, default_value: None },
                            // TODO(Derppening): Default values current do not work?
                            FuncArg {
                                name: "M".into(),
                                ty: int32,
                                default_value: Some(SymbolValue::OptionNone),
                            },
                            FuncArg {
                                name: "k".into(),
                                ty: int32,
                                default_value: Some(SymbolValue::I32(0)),
                            },
                        ],
                        ret: self.ndarray_float_2d,
                        vars: VarMap::default(),
                    })),
                    var_id: Vec::default(),
                    instance_to_symbol: HashMap::default(),
                    instance_to_stmt: HashMap::default(),
                    resolver: None,
                    codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                        |ctx, obj, fun, args, generator| {
                            gen_ndarray_eye(ctx, &obj, fun, &args, generator)
                                .map(|val| Some(val.as_basic_value_enum()))
                        },
                    )))),
                    loc: None,
                }
            }
            PrimDef::FunNpIdentity => create_fn_by_codegen(
                self.unifier,
                &VarMap::new(),
                prim.name(),
                self.ndarray_float_2d,
                &[(int32, "n")],
                Box::new(|ctx, obj, fun, args, generator| {
                    gen_ndarray_identity(ctx, &obj, fun, &args, generator)
                        .map(|val| Some(val.as_basic_value_enum()))
                }),
            ),
            _ => unreachable!(),
        }
    }

    /// Build the `range()` function.
    fn build_range_function(&mut self) -> TopLevelDef {
        let prim = PrimDef::FunRange;

        let PrimitiveStore { int32, range, .. } = *self.primitives;

        TopLevelDef::Function {
            name: prim.name().into(),
            simple_name: prim.simple_name().into(),
            signature: self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![
                    FuncArg { name: "start".into(), ty: int32, default_value: None },
                    FuncArg {
                        name: "stop".into(),
                        ty: int32,
                        // placeholder
                        default_value: Some(SymbolValue::I32(0)),
                    },
                    FuncArg {
                        name: "step".into(),
                        ty: int32,
                        default_value: Some(SymbolValue::I32(1)),
                    },
                ],
                ret: range,
                vars: VarMap::default(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, _, args, generator| {
                    let mut start = None;
                    let mut stop = None;
                    let mut step = None;
                    let int32 = ctx.ctx.i32_type();
                    let ty_i32 = ctx.primitives.int32;
                    for (i, arg) in args.iter().enumerate() {
                        if arg.0 == Some("start".into()) {
                            start = Some(
                                arg.1
                                    .clone()
                                    .to_basic_value_enum(ctx, generator, ty_i32)?
                                    .into_int_value(),
                            );
                        } else if arg.0 == Some("stop".into()) {
                            stop = Some(
                                arg.1
                                    .clone()
                                    .to_basic_value_enum(ctx, generator, ty_i32)?
                                    .into_int_value(),
                            );
                        } else if arg.0 == Some("step".into()) {
                            step = Some(
                                arg.1
                                    .clone()
                                    .to_basic_value_enum(ctx, generator, ty_i32)?
                                    .into_int_value(),
                            );
                        } else if i == 0 {
                            start = Some(
                                arg.1
                                    .clone()
                                    .to_basic_value_enum(ctx, generator, ty_i32)?
                                    .into_int_value(),
                            );
                        } else if i == 1 {
                            stop = Some(
                                arg.1
                                    .clone()
                                    .to_basic_value_enum(ctx, generator, ty_i32)?
                                    .into_int_value(),
                            );
                        } else if i == 2 {
                            step = Some(
                                arg.1
                                    .clone()
                                    .to_basic_value_enum(ctx, generator, ty_i32)?
                                    .into_int_value(),
                            );
                        }
                    }
                    let step = match step {
                        Some(step) => {
                            // assert step != 0, throw exception if not
                            let not_zero = ctx
                                .builder
                                .build_int_compare(
                                    IntPredicate::NE,
                                    step,
                                    step.get_type().const_zero(),
                                    "range_step_ne",
                                )
                                .unwrap();
                            ctx.make_assert(
                                generator,
                                not_zero,
                                "0:ValueError",
                                "range() step must not be zero",
                                [None, None, None],
                                ctx.current_loc,
                            );
                            step
                        }
                        None => int32.const_int(1, false),
                    };
                    let stop = stop.unwrap_or_else(|| {
                        let v = start.unwrap();
                        start = None;
                        v
                    });
                    let start = start.unwrap_or_else(|| int32.const_zero());

                    let ptr = RangeType::new(ctx.ctx).new_value(generator, ctx, Some("range"));
                    ptr.store_start(ctx, start);
                    ptr.store_end(ctx, stop);
                    ptr.store_step(ctx, step);
                    Ok(Some(ptr.as_base_value().into()))
                },
            )))),
            loc: None,
        }
    }

    /// Build the `str()` function.
    fn build_str_function(&mut self) -> TopLevelDef {
        let prim = PrimDef::FunStr;

        let str = self.primitives.str;

        TopLevelDef::Function {
            name: prim.name().into(),
            simple_name: prim.simple_name().into(),
            signature: self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "s".into(), ty: str, default_value: None }],
                ret: str,
                vars: VarMap::default(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let arg_ty = fun.0.args[0].ty;
                    Ok(Some(args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?))
                },
            )))),
            loc: None,
        }
    }

    /// Build functions `np_ceil()` and `np_floor()`.
    fn build_np_ceil_floor_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(prim, &[PrimDef::FunNpCeil, PrimDef::FunNpFloor]);

        create_fn_by_codegen(
            self.unifier,
            &self.float_or_ndarray_var_map,
            prim.name(),
            self.float_or_ndarray_ty.ty,
            &[(self.float_or_ndarray_ty.ty, "n")],
            Box::new(move |ctx, _, fun, args, generator| {
                let arg_ty = fun.0.args[0].ty;
                let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;

                let func = match prim {
                    PrimDef::FunNpCeil => builtin_fns::call_ceil,
                    PrimDef::FunNpFloor => builtin_fns::call_floor,
                    _ => unreachable!(),
                };
                Ok(Some(func(generator, ctx, (arg_ty, arg), ctx.primitives.float)?))
            }),
        )
    }

    /// Build the `np_round()` function.
    fn build_np_round_function(&mut self) -> TopLevelDef {
        let prim = PrimDef::FunNpRound;

        create_fn_by_codegen(
            self.unifier,
            &self.float_or_ndarray_var_map,
            prim.name(),
            self.float_or_ndarray_ty.ty,
            &[(self.float_or_ndarray_ty.ty, "n")],
            Box::new(|ctx, _, fun, args, generator| {
                let arg_ty = fun.0.args[0].ty;
                let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;
                Ok(Some(builtin_fns::call_numpy_round(generator, ctx, (arg_ty, arg))?))
            }),
        )
    }

    /// Build the `len()` function.
    fn build_len_function(&mut self) -> TopLevelDef {
        let prim = PrimDef::FunLen;

        let PrimitiveStore { uint64, int32, .. } = *self.primitives;

        let tvar = self.unifier.get_fresh_var(Some("L".into()), None);
        let list = self.unifier.add_ty(TypeEnum::TList { ty: tvar.ty });
        let ndims = self.unifier.get_fresh_const_generic_var(uint64, Some("N".into()), None);
        let ndarray = make_ndarray_ty(self.unifier, self.primitives, Some(tvar.ty), Some(ndims.ty));

        let arg_ty = self.unifier.get_fresh_var_with_range(
            &[list, ndarray, self.primitives.range],
            Some("I".into()),
            None,
        );
        TopLevelDef::Function {
            name: prim.name().into(),
            simple_name: prim.simple_name().into(),
            signature: self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg { name: "ls".into(), ty: arg_ty.ty, default_value: None }],
                ret: int32,
                vars: into_var_map([tvar, arg_ty]),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                move |ctx, _, fun, args, generator| {
                    let range_ty = ctx.primitives.range;
                    let arg_ty = fun.0.args[0].ty;
                    let arg = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;
                    Ok(if ctx.unifier.unioned(arg_ty, range_ty) {
                        let arg = RangeValue::from_ptr_val(arg.into_pointer_value(), Some("range"));
                        let (start, end, step) = destructure_range(ctx, arg);
                        Some(calculate_len_for_slice_range(generator, ctx, start, end, step).into())
                    } else {
                        match &*ctx.unifier.get_ty_immutable(arg_ty) {
                            TypeEnum::TList { .. } => {
                                let int32 = ctx.ctx.i32_type();
                                let zero = int32.const_zero();
                                let len = ctx
                                    .build_gep_and_load(
                                        arg.into_pointer_value(),
                                        &[zero, int32.const_int(1, false)],
                                        None,
                                    )
                                    .into_int_value();
                                if len.get_type().get_bit_width() == 32 {
                                    Some(len.into())
                                } else {
                                    Some(
                                        ctx.builder
                                            .build_int_truncate(len, int32, "len2i32")
                                            .map(Into::into)
                                            .unwrap(),
                                    )
                                }
                            }
                            TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
                                let llvm_i32 = ctx.ctx.i32_type();
                                let llvm_usize = generator.get_size_type(ctx.ctx);

                                let arg = NDArrayValue::from_ptr_val(
                                    arg.into_pointer_value(),
                                    llvm_usize,
                                    None,
                                );

                                let ndims = arg.dim_sizes().size(ctx, generator);
                                ctx.make_assert(
                                    generator,
                                    ctx.builder
                                        .build_int_compare(
                                            IntPredicate::NE,
                                            ndims,
                                            llvm_usize.const_zero(),
                                            "",
                                        )
                                        .unwrap(),
                                    "0:TypeError",
                                    &format!("{name}() of unsized object", name = prim.name()),
                                    [None, None, None],
                                    ctx.current_loc,
                                );

                                let len = unsafe {
                                    arg.dim_sizes().get_typed_unchecked(
                                        ctx,
                                        generator,
                                        &llvm_usize.const_zero(),
                                        None,
                                    )
                                };

                                if len.get_type().get_bit_width() == 32 {
                                    Some(len.into())
                                } else {
                                    Some(
                                        ctx.builder
                                            .build_int_truncate(len, llvm_i32, "len")
                                            .map(Into::into)
                                            .unwrap(),
                                    )
                                }
                            }
                            _ => unreachable!(),
                        }
                    })
                },
            )))),
            loc: None,
        }
    }

    /// Build the functions `min()` and `max()`.
    fn build_min_max_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(prim, &[PrimDef::FunMin, PrimDef::FunMax]);

        TopLevelDef::Function {
            name: prim.name().into(),
            simple_name: prim.simple_name().into(),
            signature: self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![
                    FuncArg { name: "m".into(), ty: self.num_ty.ty, default_value: None },
                    FuncArg { name: "n".into(), ty: self.num_ty.ty, default_value: None },
                ],
                ret: self.num_ty.ty,
                vars: self.num_var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                move |ctx, _, fun, args, generator| {
                    let m_ty = fun.0.args[0].ty;
                    let n_ty = fun.0.args[1].ty;
                    let m_val = args[0].1.clone().to_basic_value_enum(ctx, generator, m_ty)?;
                    let n_val = args[1].1.clone().to_basic_value_enum(ctx, generator, n_ty)?;

                    let func = match prim {
                        PrimDef::FunMin => builtin_fns::call_min,
                        PrimDef::FunMax => builtin_fns::call_max,
                        _ => unreachable!(),
                    };
                    Ok(Some(func(ctx, (m_ty, m_val), (n_ty, n_val))))
                },
            )))),
            loc: None,
        }
    }

    /// Build the functions `np_min()` and `np_max()`.
    fn build_np_min_max_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(prim, &[PrimDef::FunNpMin, PrimDef::FunNpMax]);

        let ret_ty = self.unifier.get_fresh_var(Some("R".into()), None);
        let var_map = self
            .num_or_ndarray_var_map
            .clone()
            .into_iter()
            .chain(once((ret_ty.id, ret_ty.ty)))
            .collect::<IndexMap<_, _>>();

        create_fn_by_codegen(
            self.unifier,
            &var_map,
            prim.name(),
            ret_ty.ty,
            &[(self.float_or_ndarray_ty.ty, "a")],
            Box::new(move |ctx, _, fun, args, generator| {
                let a_ty = fun.0.args[0].ty;
                let a = args[0].1.clone().to_basic_value_enum(ctx, generator, a_ty)?;

                let func = match prim {
                    PrimDef::FunNpMin => builtin_fns::call_numpy_min,
                    PrimDef::FunNpMax => builtin_fns::call_numpy_max,
                    _ => unreachable!(),
                };

                Ok(Some(func(generator, ctx, (a_ty, a))?))
            }),
        )
    }

    /// Build the functions `np_minimum()` and `np_maximum()`.
    fn build_np_minimum_maximum_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(prim, &[PrimDef::FunNpMinimum, PrimDef::FunNpMaximum]);

        let x1_ty = self.new_type_or_ndarray_ty(self.num_ty.ty);
        let x2_ty = self.new_type_or_ndarray_ty(self.num_ty.ty);
        let param_ty = &[(x1_ty.ty, "x1"), (x2_ty.ty, "x2")];
        let ret_ty = self.unifier.get_fresh_var(None, None);

        TopLevelDef::Function {
            name: prim.name().into(),
            simple_name: prim.simple_name().into(),
            signature: self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: param_ty
                    .iter()
                    .map(|p| FuncArg { name: p.1.into(), ty: p.0, default_value: None })
                    .collect(),
                ret: ret_ty.ty,
                vars: into_var_map([x1_ty, x2_ty, ret_ty]),
            })),
            var_id: vec![x1_ty.id, x2_ty.id],
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                move |ctx, _, fun, args, generator| {
                    let x1_ty = fun.0.args[0].ty;
                    let x2_ty = fun.0.args[1].ty;
                    let x1_val = args[0].1.clone().to_basic_value_enum(ctx, generator, x1_ty)?;
                    let x2_val = args[1].1.clone().to_basic_value_enum(ctx, generator, x2_ty)?;

                    let func = match prim {
                        PrimDef::FunNpMinimum => builtin_fns::call_numpy_minimum,
                        PrimDef::FunNpMaximum => builtin_fns::call_numpy_maximum,
                        _ => unreachable!(),
                    };

                    Ok(Some(func(generator, ctx, (x1_ty, x1_val), (x2_ty, x2_val))?))
                },
            )))),
            loc: None,
        }
    }

    /// Build the `abs()` function.
    fn build_abs_function(&mut self) -> TopLevelDef {
        let prim = PrimDef::FunAbs;

        TopLevelDef::Function {
            name: prim.name().into(),
            simple_name: prim.simple_name().into(),
            signature: self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg {
                    name: "n".into(),
                    ty: self.num_or_ndarray_ty.ty,
                    default_value: None,
                }],
                ret: self.num_or_ndarray_ty.ty,
                vars: self.num_or_ndarray_var_map.clone(),
            })),
            var_id: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                |ctx, _, fun, args, generator| {
                    let n_ty = fun.0.args[0].ty;
                    let n_val = args[0].1.clone().to_basic_value_enum(ctx, generator, n_ty)?;

                    Ok(Some(builtin_fns::call_abs(generator, ctx, (n_ty, n_val))?))
                },
            )))),
            loc: None,
        }
    }

    /// Build numpy functions that take in a float and return a boolean.
    fn build_np_float_to_bool_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(prim, &[PrimDef::FunNpIsInf, PrimDef::FunNpIsNan]);

        let PrimitiveStore { bool, float, .. } = *self.primitives;

        create_fn_by_codegen(
            self.unifier,
            &VarMap::new(),
            prim.name(),
            bool,
            &[(float, "x")],
            Box::new(move |ctx, _, fun, args, generator| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone().to_basic_value_enum(ctx, generator, x_ty)?;

                let func = match prim {
                    PrimDef::FunNpIsInf => builtin_fns::call_numpy_isinf,
                    PrimDef::FunNpIsNan => builtin_fns::call_numpy_isnan,
                    _ => unreachable!(),
                };

                Ok(Some(func(generator, ctx, (x_ty, x_val))?))
            }),
        )
    }

    /// Build 1-ary numpy/scipy functions that take in a float or an ndarray and return a value of the same type as the input.
    fn build_np_sp_float_or_ndarray_1ary_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(
            prim,
            &[
                PrimDef::FunNpSin,
                PrimDef::FunNpCos,
                PrimDef::FunNpTan,
                PrimDef::FunNpArcsin,
                PrimDef::FunNpArccos,
                PrimDef::FunNpArctan,
                PrimDef::FunNpSinh,
                PrimDef::FunNpCosh,
                PrimDef::FunNpTanh,
                PrimDef::FunNpArcsinh,
                PrimDef::FunNpArccosh,
                PrimDef::FunNpArctanh,
                PrimDef::FunNpExp,
                PrimDef::FunNpExp2,
                PrimDef::FunNpExpm1,
                PrimDef::FunNpLog,
                PrimDef::FunNpLog2,
                PrimDef::FunNpLog10,
                PrimDef::FunNpSqrt,
                PrimDef::FunNpCbrt,
                PrimDef::FunNpFabs,
                PrimDef::FunNpRint,
                PrimDef::FunSpSpecErf,
                PrimDef::FunSpSpecErfc,
                PrimDef::FunSpSpecGamma,
                PrimDef::FunSpSpecGammaln,
                PrimDef::FunSpSpecJ0,
                PrimDef::FunSpSpecJ1,
            ],
        );

        // The parameter name of the sole input of this function.
        // Usually this is just "x", but some functions have a different parameter name.
        let arg_name = match prim {
            PrimDef::FunSpSpecErf => "z",
            _ => "x",
        };

        create_fn_by_codegen(
            self.unifier,
            &self.float_or_ndarray_var_map,
            prim.name(),
            self.float_or_ndarray_ty.ty,
            &[(self.float_or_ndarray_ty.ty, arg_name)],
            Box::new(move |ctx, _, fun, args, generator| {
                let arg_ty = fun.0.args[0].ty;
                let arg_val = args[0].1.clone().to_basic_value_enum(ctx, generator, arg_ty)?;

                let func = match prim {
                    PrimDef::FunNpSin => builtin_fns::call_numpy_sin,
                    PrimDef::FunNpCos => builtin_fns::call_numpy_cos,
                    PrimDef::FunNpTan => builtin_fns::call_numpy_tan,

                    PrimDef::FunNpArcsin => builtin_fns::call_numpy_arcsin,
                    PrimDef::FunNpArccos => builtin_fns::call_numpy_arccos,
                    PrimDef::FunNpArctan => builtin_fns::call_numpy_arctan,

                    PrimDef::FunNpSinh => builtin_fns::call_numpy_sinh,
                    PrimDef::FunNpCosh => builtin_fns::call_numpy_cosh,
                    PrimDef::FunNpTanh => builtin_fns::call_numpy_tanh,

                    PrimDef::FunNpArcsinh => builtin_fns::call_numpy_arcsinh,
                    PrimDef::FunNpArccosh => builtin_fns::call_numpy_arccosh,
                    PrimDef::FunNpArctanh => builtin_fns::call_numpy_arctanh,

                    PrimDef::FunNpExp => builtin_fns::call_numpy_exp,
                    PrimDef::FunNpExp2 => builtin_fns::call_numpy_exp2,
                    PrimDef::FunNpExpm1 => builtin_fns::call_numpy_expm1,

                    PrimDef::FunNpLog => builtin_fns::call_numpy_log,
                    PrimDef::FunNpLog2 => builtin_fns::call_numpy_log2,
                    PrimDef::FunNpLog10 => builtin_fns::call_numpy_log10,

                    PrimDef::FunNpSqrt => builtin_fns::call_numpy_sqrt,
                    PrimDef::FunNpCbrt => builtin_fns::call_numpy_cbrt,

                    PrimDef::FunNpFabs => builtin_fns::call_numpy_fabs,
                    PrimDef::FunNpRint => builtin_fns::call_numpy_rint,

                    PrimDef::FunSpSpecErf => builtin_fns::call_scipy_special_erf,
                    PrimDef::FunSpSpecErfc => builtin_fns::call_scipy_special_erfc,

                    PrimDef::FunSpSpecGamma => builtin_fns::call_scipy_special_gamma,
                    PrimDef::FunSpSpecGammaln => builtin_fns::call_scipy_special_gammaln,

                    PrimDef::FunSpSpecJ0 => builtin_fns::call_scipy_special_j0,
                    PrimDef::FunSpSpecJ1 => builtin_fns::call_scipy_special_j1,

                    _ => unreachable!(),
                };
                Ok(Some(func(generator, ctx, (arg_ty, arg_val))?))
            }),
        )
    }

    /// Build 2-ary numpy functions. The exact argument types of the two input arguments can be controlled.
    fn build_np_2ary_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(
            prim,
            &[
                PrimDef::FunNpArctan2,
                PrimDef::FunNpCopysign,
                PrimDef::FunNpFmax,
                PrimDef::FunNpFmin,
                PrimDef::FunNpLdExp,
                PrimDef::FunNpHypot,
                PrimDef::FunNpNextAfter,
            ],
        );

        let PrimitiveStore { float, int32, .. } = *self.primitives;

        // The argument types of the two input arguments are controlled here.
        let (x1_ty, x2_ty) = match prim {
            PrimDef::FunNpArctan2
            | PrimDef::FunNpCopysign
            | PrimDef::FunNpFmax
            | PrimDef::FunNpFmin
            | PrimDef::FunNpHypot
            | PrimDef::FunNpNextAfter => (float, float),
            PrimDef::FunNpLdExp => (float, int32),
            _ => unreachable!(),
        };

        let x1_ty = self.new_type_or_ndarray_ty(x1_ty);
        let x2_ty = self.new_type_or_ndarray_ty(x2_ty);

        let param_ty = &[(x1_ty.ty, "x1"), (x2_ty.ty, "x2")];
        let ret_ty = self.unifier.get_fresh_var(None, None);

        TopLevelDef::Function {
            name: prim.name().into(),
            simple_name: prim.simple_name().into(),
            signature: self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: param_ty
                    .iter()
                    .map(|p| FuncArg { name: p.1.into(), ty: p.0, default_value: None })
                    .collect(),
                ret: ret_ty.ty,
                vars: into_var_map([x1_ty, x2_ty, ret_ty]),
            })),
            var_id: vec![ret_ty.id],
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                move |ctx, _, fun, args, generator| {
                    let x1_ty = fun.0.args[0].ty;
                    let x1_val = args[0].1.clone().to_basic_value_enum(ctx, generator, x1_ty)?;
                    let x2_ty = fun.0.args[1].ty;
                    let x2_val = args[1].1.clone().to_basic_value_enum(ctx, generator, x2_ty)?;

                    let func = match prim {
                        PrimDef::FunNpArctan2 => builtin_fns::call_numpy_arctan2,
                        PrimDef::FunNpCopysign => builtin_fns::call_numpy_copysign,
                        PrimDef::FunNpFmax => builtin_fns::call_numpy_fmax,
                        PrimDef::FunNpFmin => builtin_fns::call_numpy_fmin,
                        PrimDef::FunNpLdExp => builtin_fns::call_numpy_ldexp,
                        PrimDef::FunNpHypot => builtin_fns::call_numpy_hypot,
                        PrimDef::FunNpNextAfter => builtin_fns::call_numpy_nextafter,
                        _ => unreachable!(),
                    };

                    Ok(Some(func(generator, ctx, (x1_ty, x1_val), (x2_ty, x2_val))?))
                },
            )))),
            loc: None,
        }
    }

    fn build_np_any_all_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(prim, &[PrimDef::FunNpAny, PrimDef::FunNpAll]);
        let param_ty = &[(self.ndarray_num_ty, "a")];
        let ret_ty = self.primitives.bool;
        let var_map = &into_var_map([]);
        let codegen_callback: Box<GenCallCallback> =
            Box::new(move |ctx, obj, fun, args, generator| {
                let func = match prim {
                    PrimDef::FunNpAny => gen_ndarray_any,
                    PrimDef::FunNpAll => gen_ndarray_all,
                    _ => unreachable!(),
                };
                func(ctx, &obj, fun, &args, generator).map(|val| Some(val.as_basic_value_enum()))
            });

        create_fn_by_codegen(self.unifier, var_map, prim.name(), ret_ty, param_ty, codegen_callback)
    }

    fn create_method(prim: PrimDef, method_ty: Type) -> (StrRef, Type, DefinitionId) {
        (prim.simple_name().into(), method_ty, prim.id())
    }

    fn new_type_or_ndarray_ty(&mut self, scalar_ty: Type) -> TypeVar {
        let ndarray = make_ndarray_ty(self.unifier, self.primitives, Some(scalar_ty), None);

        self.unifier.get_fresh_var_with_range(&[scalar_ty, ndarray], Some("T".into()), None)
    }
}
