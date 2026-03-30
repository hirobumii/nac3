use std::{collections::HashMap, iter::once, sync::Arc};

use indexmap::IndexMap;
use inkwell::{
    IntPredicate,
    values::{BasicValue, BasicValueEnum},
};
use itertools::Itertools as _;
use nac3parser::ast::{Stmt, StrRef};
use parking_lot::RwLock;
use strum::IntoEnumIterator;

use crate::{
    codegen::{
        builtin_fns,
        numpy::{
            gen_ndarray_array, gen_ndarray_copy, gen_ndarray_empty, gen_ndarray_eye,
            gen_ndarray_fill, gen_ndarray_full, gen_ndarray_identity, gen_ndarray_ones,
            gen_ndarray_zeros, ndarray_dot,
        },
        stmt::{exn_constructor, gen_if_callback},
        types::{
            EnumerateType, NDArrayType, OptionType, ProxyTypeBase, RangeField, RangeType,
            RawNDArrayType, ScalarOrNDArray, field, parse_numpy_int_sequence,
        },
    },
    symbol_resolver::SymbolValue,
    toplevel::{
        DefinitionId, GenCall, GenCallCallback, TopLevelDef,
        composer::TopLevelComposer,
        helper::{
            PrimDef, PrimDefDetails, arraylike_flatten_element_type, debug_assert_prim_is_allowed,
            extract_ndims, make_exception_fields,
        },
        numpy::{make_ndarray_ty, unpack_ndarray_var_tys},
        type_annotation::TypeAnnotation,
    },
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{
            AttrKind, FunSignature, FuncArg, Type, TypeEnum, TypeVar, Unifier, VarMap,
            into_var_map, iter_type_vars,
        },
    },
};

type BuiltinInfo = Vec<(Arc<RwLock<TopLevelDef>>, Option<Stmt>)>;

pub fn get_exn_constructor(
    name: &str,
    class_id: usize,
    cons_id: usize,
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
) -> (TopLevelDef, TopLevelDef, Type, Type) {
    let PrimitiveStore { int32, int64, str: string, .. } = *primitives;
    let exception_fields = make_exception_fields(int32, int64, string);
    let exn_cons_args = vec![
        FuncArg {
            name: "msg".into(),
            ty: string,
            default_value: Some(SymbolValue::Str(String::new())),
            is_vararg: false,
        },
        FuncArg {
            name: "param0".into(),
            ty: int64,
            default_value: Some(SymbolValue::I64(0)),
            is_vararg: false,
        },
        FuncArg {
            name: "param1".into(),
            ty: int64,
            default_value: Some(SymbolValue::I64(0)),
            is_vararg: false,
        },
        FuncArg {
            name: "param2".into(),
            ty: int64,
            default_value: Some(SymbolValue::I64(0)),
            is_vararg: false,
        },
    ];
    let exn_type = unifier.add_ty(TypeEnum::TObj {
        obj_id: DefinitionId(class_id),
        fields: exception_fields
            .clone()
            .into_iter()
            .map(|(name, ty, mutable)| (name, (ty, AttrKind::Field { mutable })))
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
        attributes: Vec::default(),
        instance_to_symbol: HashMap::default(),
        instance_to_stmt: HashMap::default(),
        resolver: None,
        codegen_callback: Some(Arc::new(GenCall::new(Box::new(exn_constructor)))),
        loc: None,
    };
    let class_def = TopLevelDef::Class {
        name: name.into(),
        simple_name: name.rsplit_once('.').map_or(name, |(_, nme)| nme).to_string(),
        object_id: DefinitionId(class_id),
        type_vars: Vec::default(),
        fields: exception_fields,
        attributes: Vec::default(),
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
///   [parameter type][Type] and the parameter symbol name.
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
                .map(|p| FuncArg {
                    name: p.1.into(),
                    ty: p.0,
                    default_value: None,
                    is_vararg: false,
                })
                .collect(),
            ret: ret_ty,
            vars: var_map.clone(),
        })),
        var_id: Vec::default(),
        attributes: Vec::default(),
        instance_to_symbol: HashMap::default(),
        instance_to_stmt: HashMap::default(),
        resolver: None,
        codegen_callback: Some(Arc::new(GenCall::new(codegen_callback))),
        loc: None,
    }
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
    const fn of_int(self, primitives: &PrimitiveStore) -> Type {
        match self {
            Self::Bits32 => primitives.int32,
            Self::Bits64 => primitives.int64,
        }
    }
}

struct BuiltinBuilder<'a> {
    unifier: &'a mut Unifier,
    primitives: &'a PrimitiveStore,

    is_some_ty: Type,
    unwrap_ty: Type,
    option_tvar: TypeVar,

    list_tvar: TypeVar,

    ndarray_dtype_tvar: TypeVar,
    ndarray_ndims_tvar: TypeVar,
    ndarray_copy_ty: Type,
    ndarray_fill_ty: Type,

    list_int32: Type,

    num_ty: TypeVar,
    num_var_map: VarMap,

    ndarray_float: Type,
    ndarray_float_2d: Type,

    float_or_ndarray_ty: TypeVar,
    float_or_ndarray_var_map: VarMap,

    num_or_ndarray_ty: TypeVar,
    num_or_ndarray_var_map: VarMap,

    /// See [`BuiltinBuilder::build_ndarray_from_shape_factory_function`]
    ndarray_factory_fn_shape_arg_tvar: TypeVar,
}

impl<'a> BuiltinBuilder<'a> {
    fn new(unifier: &'a mut Unifier, primitives: &'a PrimitiveStore) -> Self {
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
        let ((is_some_ty, _), (unwrap_ty, _), option_tvar) =
            if let TypeEnum::TObj { fields, params, .. } = unifier.get_ty(option).as_ref() {
                (
                    *fields.get(&PrimDef::FunOptionIsSome.simple_name().into()).unwrap(),
                    *fields.get(&PrimDef::FunOptionUnwrap.simple_name().into()).unwrap(),
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
        let (ndarray_copy_ty, _) =
            *ndarray_fields.get(&PrimDef::FunNDArrayCopy.simple_name().into()).unwrap();
        let (ndarray_fill_ty, _) =
            *ndarray_fields.get(&PrimDef::FunNDArrayFill.simple_name().into()).unwrap();

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

        let list_tvar = if let TypeEnum::TObj { obj_id, params, .. } =
            &*unifier.get_ty_immutable(primitives.list)
        {
            assert_eq!(*obj_id, PrimDef::List.id());
            iter_type_vars(params).nth(0).unwrap()
        } else {
            unreachable!()
        };
        let list_int32 = unifier
            .subst(primitives.list, &into_var_map([TypeVar { id: list_tvar.id, ty: int32 }]))
            .unwrap();

        let ndarray_factory_fn_shape_arg_tvar = unifier.get_fresh_var(Some("Shape".into()), None);

        BuiltinBuilder {
            unifier,
            primitives,

            is_some_ty,
            unwrap_ty,
            option_tvar,

            list_tvar,

            ndarray_dtype_tvar,
            ndarray_ndims_tvar,
            ndarray_copy_ty,
            ndarray_fill_ty,

            list_int32,

            num_ty,
            num_var_map,

            ndarray_float,
            ndarray_float_2d,

            float_or_ndarray_ty,
            float_or_ndarray_var_map,

            num_or_ndarray_ty,
            num_or_ndarray_var_map,

            ndarray_factory_fn_shape_arg_tvar,
        }
    }

    /// Construct every function from every [`PrimDef`], in the order of [`PrimDef`]'s definition.
    fn build_all_builtins(&mut self) -> Vec<TopLevelDef> {
        PrimDef::iter().map(|prim| self.build_builtin_of_prim(prim)).collect_vec()
    }

    /// Build the [`TopLevelDef`] associated of a [`PrimDef`].
    fn build_builtin_of_prim(&mut self, prim: PrimDef) -> TopLevelDef {
        let tld = match prim {
            PrimDef::Float
            | PrimDef::Bool
            | PrimDef::Str
            | PrimDef::Tuple
            | PrimDef::StaticMethod
            | PrimDef::Auto
            | PrimDef::Kernel
            | PrimDef::KernelInvariant
            | PrimDef::ConstGeneric
            | PrimDef::None
            | PrimDef::Virtual
            | PrimDef::Compile
            | PrimDef::ExternFn
            | PrimDef::KernelDecorator
            | PrimDef::Portable
            | PrimDef::Rpc
            | PrimDef::Generic
            | PrimDef::Literal
            | PrimDef::Int32
            | PrimDef::Int64
            | PrimDef::UInt32
            | PrimDef::UInt64 => Self::build_simple_primitive_class(prim),

            PrimDef::Range | PrimDef::FunRangeInit => self.build_range_class_related(prim),
            PrimDef::Enumerate | PrimDef::FunEnumerateInit => {
                self.build_enumerate_class_related(prim)
            }

            PrimDef::Exception => self.build_exception_class_related(prim),

            PrimDef::Option
            | PrimDef::FunOptionIsSome
            | PrimDef::FunOptionIsNone
            | PrimDef::FunOptionUnwrap
            | PrimDef::FunSome => self.build_option_class_related(prim),

            PrimDef::List => self.build_list_class_related(prim),

            PrimDef::NDArray | PrimDef::FunNDArrayCopy | PrimDef::FunNDArrayFill => {
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

            PrimDef::FunNpSize | PrimDef::FunNpShape | PrimDef::FunNpStrides => {
                self.build_ndarray_property_getter_function(prim)
            }

            PrimDef::FunNpBroadcastTo | PrimDef::FunNpTranspose | PrimDef::FunNpReshape => {
                self.build_ndarray_view_function(prim)
            }

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

            PrimDef::FunNpArgmin | PrimDef::FunNpArgmax | PrimDef::FunNpMin | PrimDef::FunNpMax => {
                self.build_np_max_min_function(prim)
            }

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

            PrimDef::FunNpDot
            | PrimDef::FunNpLinalgCholesky
            | PrimDef::FunNpLinalgQr
            | PrimDef::FunNpLinalgSvd
            | PrimDef::FunNpLinalgInv
            | PrimDef::FunNpLinalgPinv
            | PrimDef::FunNpLinalgMatrixPower
            | PrimDef::FunNpLinalgDet
            | PrimDef::FunSpLinalgLu
            | PrimDef::FunSpLinalgSchur
            | PrimDef::FunSpLinalgHessenberg => self.build_linalg_methods(prim),
        };

        if cfg!(debug_assertions) {
            // Sanity checks on the constructed [`TopLevelDef`]

            match (&tld, prim.details()) {
                (
                    TopLevelDef::Class { name, object_id, .. },
                    PrimDefDetails::PrimClass { name: exp_name, .. },
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
                    panic!(
                        "Class/function variant of the constructed TopLevelDef of PrimDef {prim:?} is different than what is defined by {prim:?}"
                    )
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
                PrimDef::Float,
                PrimDef::Bool,
                PrimDef::Str,
                PrimDef::Tuple,
                PrimDef::StaticMethod,
                PrimDef::Auto,
                PrimDef::Kernel,
                PrimDef::KernelInvariant,
                PrimDef::ConstGeneric,
                PrimDef::None,
                PrimDef::Virtual,
                PrimDef::Compile,
                PrimDef::ExternFn,
                PrimDef::KernelDecorator,
                PrimDef::Portable,
                PrimDef::Rpc,
                PrimDef::Generic,
                PrimDef::Literal,
                PrimDef::Int32,
                PrimDef::Int64,
                PrimDef::UInt32,
                PrimDef::UInt64,
            ],
        );

        TopLevelComposer::make_top_level_class_def(prim.id(), None, prim.name().into(), None, None)
    }

    fn build_range_class_related(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(prim, &[PrimDef::Range, PrimDef::FunRangeInit]);

        let PrimitiveStore { int32, range, .. } = *self.primitives;

        let make_ctor_signature = |unifier: &mut Unifier| {
            unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![
                    FuncArg {
                        name: "start".into(),
                        ty: int32,
                        default_value: None,
                        is_vararg: false,
                    },
                    FuncArg {
                        name: "stop".into(),
                        ty: int32,
                        // placeholder
                        default_value: Some(SymbolValue::I32(0)),
                        is_vararg: false,
                    },
                    FuncArg {
                        name: "step".into(),
                        ty: int32,
                        default_value: Some(SymbolValue::I32(1)),
                        is_vararg: false,
                    },
                ],
                ret: range,
                vars: VarMap::default(),
            }))
        };

        match prim {
            PrimDef::Range => {
                let fields = vec![
                    ("start".into(), int32, true),
                    ("stop".into(), int32, true),
                    ("step".into(), int32, true),
                ];
                let ctor_signature = make_ctor_signature(self.unifier);

                TopLevelDef::Class {
                    name: prim.name().into(),
                    simple_name: prim
                        .name()
                        .rsplit_once('.')
                        .map_or_else(|| prim.name(), |(_, nme)| nme)
                        .to_string(),
                    object_id: prim.id(),
                    type_vars: Vec::default(),
                    fields,
                    attributes: Vec::default(),
                    methods: vec![("__init__".into(), ctor_signature, PrimDef::FunRangeInit.id())],
                    ancestors: Vec::default(),
                    constructor: Some(ctor_signature),
                    resolver: None,
                    loc: None,
                }
            }

            PrimDef::FunRangeInit => TopLevelDef::Function {
                name: prim.name().into(),
                simple_name: prim.simple_name().into(),
                signature: make_ctor_signature(self.unifier),
                var_id: Vec::default(),
                attributes: Vec::default(),
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, obj, _, args| {
                    let (zelf_ty, zelf) = obj.unwrap();
                    let zelf = zelf.to_basic_value_enum(ctx, zelf_ty)?.into_pointer_value();
                    let zelf = RangeType::new(ctx).map_value(zelf, Some("range"));

                    let mut start = None;
                    let mut stop = None;
                    let mut step = None;
                    let int32 = ctx.i32;
                    let ty_i32 = ctx.primitives.int32;
                    for (i, arg) in args.iter().enumerate() {
                        if arg.0 == Some("start".into()) {
                            start = Some(
                                arg.1.clone().to_basic_value_enum(ctx, ty_i32)?.into_int_value(),
                            );
                        } else if arg.0 == Some("stop".into()) {
                            stop = Some(
                                arg.1.clone().to_basic_value_enum(ctx, ty_i32)?.into_int_value(),
                            );
                        } else if arg.0 == Some("step".into()) {
                            step = Some(
                                arg.1.clone().to_basic_value_enum(ctx, ty_i32)?.into_int_value(),
                            );
                        } else if i == 0 {
                            start = Some(
                                arg.1.clone().to_basic_value_enum(ctx, ty_i32)?.into_int_value(),
                            );
                        } else if i == 1 {
                            stop = Some(
                                arg.1.clone().to_basic_value_enum(ctx, ty_i32)?.into_int_value(),
                            );
                        } else if i == 2 {
                            step = Some(
                                arg.1.clone().to_basic_value_enum(ctx, ty_i32)?.into_int_value(),
                            );
                        }
                    }
                    let step = match step {
                        Some(step) => {
                            // assert step != 0, throw exception if not
                            let not_zero = ctx.builder.build_int_compare(
                                IntPredicate::NE,
                                step,
                                step.get_type().const_zero(),
                                "range_step_ne",
                            )?;
                            ctx.make_assert(
                                not_zero,
                                "0:ValueError",
                                "range() step must not be zero",
                                [None, None, None],
                                ctx.current_loc,
                            )?;
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

                    zelf.store_field(ctx, RangeField::Start, start)?;
                    zelf.store_field(ctx, RangeField::End, stop)?;
                    zelf.store_field(ctx, RangeField::Step, step)?;

                    Ok(Some(zelf.value.into()))
                })))),
                loc: None,
            },

            _ => unreachable!(),
        }
    }

    fn build_enumerate_class_related(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(prim, &[PrimDef::Enumerate, PrimDef::FunEnumerateInit]);

        let PrimitiveStore { int32, enumerate, .. } = *self.primitives;

        let arg_tvar = self.unifier.get_dummy_var();

        let ctor_signature = self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
            args: vec![
                FuncArg {
                    name: "iterable".into(),
                    ty: arg_tvar.ty,
                    default_value: None,
                    is_vararg: false,
                },
                FuncArg {
                    name: "start".into(),
                    ty: int32,
                    default_value: Some(SymbolValue::I32(0)),
                    is_vararg: false,
                },
            ],
            ret: enumerate,
            vars: VarMap::default(),
        }));
        match prim {
            PrimDef::Enumerate => {
                let fields =
                    vec![("iterable".into(), arg_tvar.ty, true), ("start".into(), int32, true)];
                TopLevelDef::Class {
                    name: prim.name().into(),
                    simple_name: prim
                        .name()
                        .rsplit_once('.')
                        .map_or_else(|| prim.name(), |(_, nme)| nme)
                        .to_string(),
                    object_id: prim.id(),
                    type_vars: Vec::default(),
                    fields,
                    attributes: Vec::default(),
                    methods: vec![(
                        "__init__".into(),
                        ctor_signature,
                        PrimDef::FunEnumerateInit.id(),
                    )],
                    ancestors: Vec::default(),
                    constructor: Some(ctor_signature),
                    resolver: None,
                    loc: None,
                }
            }
            PrimDef::FunEnumerateInit => TopLevelDef::Function {
                name: prim.name().into(),
                simple_name: prim.simple_name().into(),
                signature: ctor_signature,
                var_id: Vec::default(),
                attributes: Vec::default(),
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, obj, _, args| {
                    let (zelf_ty, zelf) = obj.unwrap();
                    let zelf = zelf.to_basic_value_enum(ctx, zelf_ty)?.into_pointer_value();
                    let zelf = EnumerateType::new(ctx.inner).map_value(zelf, Some("enumerate"));

                    let ty_i32 = ctx.primitives.int32;
                    let int32 = ctx.i32;

                    let mut start = None;

                    for (i, arg) in args.iter().enumerate() {
                        if arg.0 == Some("start".into()) || i == 1 {
                            start = Some(
                                arg.1.clone().to_basic_value_enum(ctx, ty_i32)?.into_int_value(),
                            );
                        }
                    }

                    let start = start.unwrap_or_else(|| int32.const_zero());

                    zelf.store(ctx, field!(start), start)?;

                    Ok(Some(zelf.value.into()))
                })))),
                loc: None,
            },

            _ => unreachable!(),
        }
    }

    /// Build the class `Exception` and its associated methods.
    fn build_exception_class_related(&self, prim: PrimDef) -> TopLevelDef {
        // NOTE: currently only contains the class `Exception`
        debug_assert_prim_is_allowed(prim, &[PrimDef::Exception]);

        let PrimitiveStore { int32, int64, str, .. } = *self.primitives;

        match prim {
            PrimDef::Exception => TopLevelDef::Class {
                name: prim.name().into(),
                simple_name: prim
                    .name()
                    .rsplit_once('.')
                    .map_or_else(|| prim.name(), |(_, nme)| nme)
                    .to_string(),
                object_id: prim.id(),
                type_vars: Vec::default(),
                fields: make_exception_fields(int32, int64, str),
                attributes: Vec::default(),
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
                PrimDef::FunOptionIsSome,
                PrimDef::FunOptionIsNone,
                PrimDef::FunOptionUnwrap,
                PrimDef::FunSome,
            ],
        );

        match prim {
            PrimDef::Option => TopLevelDef::Class {
                name: prim.name().into(),
                simple_name: prim
                    .name()
                    .rsplit_once('.')
                    .map_or_else(|| prim.name(), |(_, nme)| nme)
                    .to_string(),
                object_id: prim.id(),
                type_vars: vec![self.option_tvar.ty],
                fields: Vec::default(),
                attributes: Vec::default(),
                methods: vec![
                    Self::create_method(PrimDef::FunOptionIsSome, self.is_some_ty),
                    Self::create_method(PrimDef::FunOptionIsNone, self.is_some_ty),
                    Self::create_method(PrimDef::FunOptionUnwrap, self.unwrap_ty),
                ],
                ancestors: vec![TypeAnnotation::CustomClass {
                    id: prim.id(),
                    params: Vec::default(),
                }],
                constructor: None,
                resolver: None,
                loc: None,
            },

            PrimDef::FunOptionUnwrap => TopLevelDef::Function {
                name: prim.name().into(),
                simple_name: prim.simple_name().into(),
                signature: self.unwrap_ty,
                var_id: vec![self.option_tvar.id],
                attributes: Vec::default(),
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::create_dummy(String::from(
                    "handled in gen_expr",
                )))),
                loc: None,
            },

            PrimDef::FunOptionIsNone | PrimDef::FunOptionIsSome => TopLevelDef::Function {
                name: prim.name().to_string(),
                simple_name: prim.simple_name().into(),
                signature: self.is_some_ty,
                var_id: vec![self.option_tvar.id],
                attributes: Vec::default(),
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(move |ctx, obj, _, _| {
                    let expect_ty = obj.clone().unwrap().0;
                    let obj_val = obj.unwrap().1.clone().to_basic_value_enum(ctx, expect_ty)?;
                    let BasicValueEnum::PointerValue(ptr) = obj_val else {
                        unreachable!("option must be ptr")
                    };

                    let returned_int = match prim {
                        PrimDef::FunOptionIsNone => {
                            ctx.builder.build_is_null(ptr, prim.simple_name())
                        }
                        PrimDef::FunOptionIsSome => {
                            ctx.builder.build_is_not_null(ptr, prim.simple_name())
                        }
                        _ => unreachable!(),
                    };
                    Ok(Some(returned_int?.into()))
                })))),
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
                        is_vararg: false,
                    }],
                    ret: self.primitives.option,
                    vars: into_var_map([self.option_tvar]),
                })),
                var_id: vec![self.option_tvar.id],
                attributes: Vec::default(),
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, _, fun, args| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg_val = args[0].1.clone().to_basic_value_enum(ctx, arg_ty)?;
                    let option_ty = OptionType::from_unifier_type(ctx, fun.0.ret);
                    let option = option_ty.construct(ctx, Some(arg_val), Some("alloca_some"))?;
                    Ok(Some(option.value.into()))
                })))),
                loc: None,
            },

            _ => {
                unreachable!()
            }
        }
    }

    fn build_list_class_related(&self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(prim, &[PrimDef::List]);

        match prim {
            PrimDef::List => TopLevelDef::Class {
                name: prim.name().into(),
                simple_name: prim
                    .name()
                    .rsplit_once('.')
                    .map_or_else(|| prim.name(), |(_, nme)| nme)
                    .to_string(),
                object_id: prim.id(),
                type_vars: vec![self.list_tvar.ty],
                fields: Vec::default(),
                attributes: Vec::default(),
                methods: Vec::default(),
                ancestors: Vec::default(),
                constructor: None,
                resolver: None,
                loc: None,
            },

            _ => unreachable!(),
        }
    }

    /// Build the class `ndarray` and its associated methods.
    fn build_ndarray_class_related(&self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(
            prim,
            &[PrimDef::NDArray, PrimDef::FunNDArrayCopy, PrimDef::FunNDArrayFill],
        );

        match prim {
            PrimDef::NDArray => TopLevelDef::Class {
                name: prim.name().into(),
                simple_name: prim
                    .name()
                    .rsplit_once('.')
                    .map_or_else(|| prim.name(), |(_, nme)| nme)
                    .to_string(),
                object_id: prim.id(),
                type_vars: vec![self.ndarray_dtype_tvar.ty, self.ndarray_ndims_tvar.ty],
                fields: Vec::default(),
                attributes: Vec::default(),
                methods: vec![
                    Self::create_method(PrimDef::FunNDArrayCopy, self.ndarray_copy_ty),
                    Self::create_method(PrimDef::FunNDArrayFill, self.ndarray_fill_ty),
                ],
                ancestors: Vec::default(),
                constructor: None,
                resolver: None,
                loc: None,
            },

            PrimDef::FunNDArrayCopy => TopLevelDef::Function {
                name: prim.name().into(),
                simple_name: prim.simple_name().into(),
                signature: self.ndarray_copy_ty,
                var_id: vec![self.ndarray_dtype_tvar.id, self.ndarray_ndims_tvar.id],
                attributes: Vec::default(),
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, obj, fun, args| {
                    gen_ndarray_copy(ctx, &obj, fun, &args)
                        .map(|val| Some(val.as_basic_value_enum()))
                })))),
                loc: None,
            },

            PrimDef::FunNDArrayFill => TopLevelDef::Function {
                name: prim.name().into(),
                simple_name: prim.simple_name().into(),
                signature: self.ndarray_fill_ty,
                var_id: vec![self.ndarray_dtype_tvar.id, self.ndarray_ndims_tvar.id],
                attributes: Vec::default(),
                instance_to_symbol: HashMap::default(),
                instance_to_stmt: HashMap::default(),
                resolver: None,
                codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, obj, fun, args| {
                    gen_ndarray_fill(ctx, &obj, fun, &args)?;
                    Ok(None)
                })))),
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
                    is_vararg: false,
                }],
                ret: self.num_or_ndarray_ty.ty,
                vars: self.num_or_ndarray_var_map.clone(),
            })),
            var_id: Vec::default(),
            attributes: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(move |ctx, _, fun, args| {
                let arg_ty = fun.0.args[0].ty;
                let arg = args[0].1.clone().to_basic_value_enum(ctx, arg_ty)?;

                let func = match prim {
                    PrimDef::FunInt32 => builtin_fns::call_int32,
                    PrimDef::FunInt64 => builtin_fns::call_int64,
                    PrimDef::FunUInt32 => builtin_fns::call_uint32,
                    PrimDef::FunUInt64 => builtin_fns::call_uint64,
                    PrimDef::FunFloat => builtin_fns::call_float,
                    PrimDef::FunBool => builtin_fns::call_bool,
                    _ => unreachable!(),
                };
                Ok(Some(func(ctx, (arg_ty, arg))?))
            })))),
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
            Box::new(move |ctx, _, fun, args| {
                let arg_ty = fun.0.args[0].ty;
                let arg = args[0].1.clone().to_basic_value_enum(ctx, arg_ty)?;

                let ret_elem_ty = size_variant.of_int(&ctx.primitives);
                Ok(Some(builtin_fns::call_round(ctx, (arg_ty, arg), ret_elem_ty)?))
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
            Box::new(move |ctx, _, fun, args| {
                let arg_ty = fun.0.args[0].ty;
                let arg = args[0].1.clone().to_basic_value_enum(ctx, arg_ty)?;

                let ret_elem_ty = size_variant.of_int(&ctx.primitives);
                let func = match kind {
                    Kind::Ceil => builtin_fns::call_ceil,
                    Kind::Floor => builtin_fns::call_floor,
                };
                Ok(Some(func(ctx, (arg_ty, arg), ret_elem_ty)?))
            }),
        )
    }

    /// Build ndarray factory functions that only take in an argument `shape`.
    ///
    /// `shape` can be a tuple of int32s, a list of int32s, or a scalar int32.
    fn build_ndarray_from_shape_factory_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(
            prim,
            &[PrimDef::FunNpNDArray, PrimDef::FunNpEmpty, PrimDef::FunNpZeros, PrimDef::FunNpOnes],
        );

        // NOTE: on `ndarray_factory_fn_shape_arg_tvar` and
        // the `param_ty` for `create_fn_by_codegen`.
        //
        // Ideally, we should have created a [`TypeVar`] to define all possible input
        // types for the parameter "shape" like so:
        // ```rust
        // self.unifier.get_fresh_var_with_range(
        //   &[int32, list_int32, /* and more... */],
        //   Some("T".into()), None)
        // )
        // ```
        //
        // However, there is (currently) no way to type a tuple of arbitrary length in `nac3core`.
        //
        // And this is the best we could do:
        // ```rust
        // &[ int32, list_int32, tuple_1_int32, tuple_2_int32, tuple_3_int32, ... ],
        // ```
        //
        // But this is not ideal.
        //
        // Instead, we delegate the responsibility of typechecking
        // to [`typecheck::type_inferencer::Inferencer::fold_numpy_function_call_shape_argument`],
        // and use a dummy [`TypeVar`] `ndarray_factory_fn_shape_arg_tvar` as a placeholder for `param_ty`.

        create_fn_by_codegen(
            self.unifier,
            &VarMap::new(),
            prim.name(),
            self.ndarray_float,
            &[(self.ndarray_factory_fn_shape_arg_tvar.ty, "shape")],
            Box::new(move |ctx, obj, fun, args| {
                let func = match prim {
                    PrimDef::FunNpNDArray | PrimDef::FunNpEmpty => gen_ndarray_empty,
                    PrimDef::FunNpZeros => gen_ndarray_zeros,
                    PrimDef::FunNpOnes => gen_ndarray_ones,
                    _ => unreachable!(),
                };
                func(ctx, &obj, fun, &args).map(|val| Some(val.as_basic_value_enum()))
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
                            FuncArg {
                                name: "object".into(),
                                ty: tv.ty,
                                default_value: None,
                                is_vararg: false,
                            },
                            FuncArg {
                                name: "copy".into(),
                                ty: bool,
                                default_value: Some(SymbolValue::Bool(true)),
                                is_vararg: false,
                            },
                            FuncArg {
                                name: "ndmin".into(),
                                ty: int32,
                                default_value: Some(SymbolValue::U32(0)),
                                is_vararg: false,
                            },
                        ],
                        ret: ndarray,
                        vars: into_var_map([tv]),
                    })),
                    var_id: vec![tv.id],
                    attributes: Vec::default(),
                    instance_to_symbol: HashMap::default(),
                    instance_to_stmt: HashMap::default(),
                    resolver: None,
                    codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                        |ctx, obj, fun, args| {
                            gen_ndarray_array(ctx, &obj, fun, &args)
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
                    Box::new(move |ctx, obj, fun, args| {
                        gen_ndarray_full(ctx, &obj, fun, &args)
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
                            FuncArg {
                                name: "N".into(),
                                ty: int32,
                                default_value: None,
                                is_vararg: false,
                            },
                            // TODO(Derppening): Default values current do not work?
                            FuncArg {
                                name: "M".into(),
                                ty: int32,
                                default_value: Some(SymbolValue::OptionNone),
                                is_vararg: false,
                            },
                            FuncArg {
                                name: "k".into(),
                                ty: int32,
                                default_value: Some(SymbolValue::I32(0)),
                                is_vararg: false,
                            },
                        ],
                        ret: self.ndarray_float_2d,
                        vars: VarMap::default(),
                    })),
                    var_id: Vec::default(),
                    attributes: Vec::default(),
                    instance_to_symbol: HashMap::default(),
                    instance_to_stmt: HashMap::default(),
                    resolver: None,
                    codegen_callback: Some(Arc::new(GenCall::new(Box::new(
                        |ctx, obj, fun, args| {
                            gen_ndarray_eye(ctx, &obj, fun, &args)
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
                Box::new(|ctx, obj, fun, args| {
                    gen_ndarray_identity(ctx, &obj, fun, &args)
                        .map(|val| Some(val.as_basic_value_enum()))
                }),
            ),
            _ => unreachable!(),
        }
    }

    fn build_ndarray_property_getter_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(
            prim,
            &[PrimDef::FunNpSize, PrimDef::FunNpShape, PrimDef::FunNpStrides],
        );

        let in_ndarray_ty = self.unifier.get_fresh_var_with_range(
            &[self.primitives.ndarray],
            Some("T".into()),
            None,
        );

        match prim {
            PrimDef::FunNpSize => create_fn_by_codegen(
                self.unifier,
                &into_var_map([in_ndarray_ty]),
                prim.name(),
                self.primitives.int32,
                &[(in_ndarray_ty.ty, "a")],
                Box::new(|ctx, obj, fun, args| {
                    assert!(obj.is_none());
                    assert_eq!(args.len(), 1);

                    let ndarray_ty = fun.0.args[0].ty;
                    let ndarray = args[0].1.clone().to_basic_value_enum(ctx, ndarray_ty)?;
                    let ndarray = RawNDArrayType::from_unifier_type(ctx, ndarray_ty)
                        .map_value(ndarray.into_pointer_value(), None);

                    let size = ndarray.size(ctx)?;
                    let size = ctx.builder.build_int_truncate_or_bit_cast(size, ctx.i32, "")?;
                    Ok(Some(size.into()))
                }),
            ),

            PrimDef::FunNpShape | PrimDef::FunNpStrides => {
                // The function signatures of `np_shape` an `np_size` are the same.
                // Mixed together for convenience.

                // The return type is a tuple of variable length depending on the ndims of the input ndarray.
                let ret_ty = self.unifier.get_dummy_var().ty; // Handled by special folding

                create_fn_by_codegen(
                    self.unifier,
                    &into_var_map([in_ndarray_ty]),
                    prim.name(),
                    ret_ty,
                    &[(in_ndarray_ty.ty, "a")],
                    Box::new(move |ctx, obj, fun, args| {
                        assert!(obj.is_none());
                        assert_eq!(args.len(), 1);

                        let ndarray_ty = fun.0.args[0].ty;
                        let ndarray = args[0].1.clone().to_basic_value_enum(ctx, ndarray_ty)?;

                        let ndarray = RawNDArrayType::from_unifier_type(ctx, ndarray_ty)
                            .map_value(ndarray.into_pointer_value(), None);

                        let result_tuple = match prim {
                            PrimDef::FunNpShape => ndarray.make_shape_tuple(ctx)?,
                            PrimDef::FunNpStrides => ndarray.make_strides_tuple(ctx)?,
                            _ => unreachable!(),
                        };

                        Ok(Some(result_tuple.value.into()))
                    }),
                )
            }
            _ => unreachable!(),
        }
    }

    /// Build np/sp functions that take as input `NDArray` only
    fn build_ndarray_view_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(
            prim,
            &[PrimDef::FunNpBroadcastTo, PrimDef::FunNpTranspose, PrimDef::FunNpReshape],
        );

        let in_ndarray_ty = self.unifier.get_fresh_var_with_range(
            &[self.primitives.ndarray],
            Some("T".into()),
            None,
        );

        match prim {
            PrimDef::FunNpTranspose => create_fn_by_codegen(
                self.unifier,
                &into_var_map([in_ndarray_ty]),
                prim.name(),
                in_ndarray_ty.ty,
                &[(in_ndarray_ty.ty, "x")],
                Box::new(move |ctx, _, fun, args| {
                    let arg_ty = fun.0.args[0].ty;
                    let arg_val = args[0].1.clone().to_basic_value_enum(ctx, arg_ty)?;

                    let ndarray = NDArrayType::from_unifier_type(ctx, arg_ty)
                        .map_value(arg_val.into_pointer_value(), None);

                    let ndarray = ndarray.transpose(ctx, None)?; // TODO: Add axes argument
                    Ok(Some(ndarray.value.into()))
                }),
            ),

            // NOTE: on `ndarray_factory_fn_shape_arg_tvar` and
            // the `param_ty` for `create_fn_by_codegen`.
            //
            // Similar to `build_ndarray_from_shape_factory_function` we delegate the responsibility of typechecking
            // to [`typecheck::type_inferencer::Inferencer::fold_numpy_function_call_shape_argument`],
            // and use a dummy [`TypeVar`] `ndarray_factory_fn_shape_arg_tvar` as a placeholder for `param_ty`.
            PrimDef::FunNpBroadcastTo | PrimDef::FunNpReshape => {
                // These two functions have the same function signature.
                // Mixed together for convenience.

                let ret_ty = self.unifier.get_dummy_var().ty; // Handled by special holding

                create_fn_by_codegen(
                    self.unifier,
                    &VarMap::new(),
                    prim.name(),
                    ret_ty,
                    &[
                        (in_ndarray_ty.ty, "x"),
                        (self.ndarray_factory_fn_shape_arg_tvar.ty, "shape"), // Handled by special folding
                    ],
                    Box::new(move |ctx, _, fun, args| {
                        let ndarray_ty = fun.0.args[0].ty;
                        let ndarray_val = args[0].1.clone().to_basic_value_enum(ctx, ndarray_ty)?;

                        let shape_ty = fun.0.args[1].ty;
                        let shape_val = args[1].1.clone().to_basic_value_enum(ctx, shape_ty)?;

                        let ndarray = NDArrayType::from_unifier_type(ctx, ndarray_ty)
                            .map_value(ndarray_val.into_pointer_value(), None);

                        let shape = parse_numpy_int_sequence(ctx, (shape_ty, shape_val))?;

                        // The ndims after reshaping is gotten from the return type of the call.
                        let (_, ndims) = unpack_ndarray_var_tys(&mut ctx.unifier, fun.0.ret);
                        let ndims = extract_ndims(&ctx.unifier, ndims);

                        let new_ndarray = match prim {
                            PrimDef::FunNpBroadcastTo => ndarray.broadcast_to(ctx, ndims, shape)?,
                            PrimDef::FunNpReshape => {
                                ndarray.reshape_or_copy(ctx, ndims, shape.inner_value(ctx)?)?
                            }
                            _ => unreachable!(),
                        };
                        Ok(Some(new_ndarray.value.as_basic_value_enum()))
                    }),
                )
            }

            _ => unreachable!(),
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
                args: vec![FuncArg {
                    name: "s".into(),
                    ty: str,
                    default_value: None,
                    is_vararg: false,
                }],
                ret: str,
                vars: VarMap::default(),
            })),
            var_id: Vec::default(),
            attributes: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, _, fun, args| {
                let arg_ty = fun.0.args[0].ty;
                Ok(Some(args[0].1.clone().to_basic_value_enum(ctx, arg_ty)?))
            })))),
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
            Box::new(move |ctx, _, fun, args| {
                let arg_ty = fun.0.args[0].ty;
                let arg = args[0].1.clone().to_basic_value_enum(ctx, arg_ty)?;

                let func = match prim {
                    PrimDef::FunNpCeil => builtin_fns::call_ceil,
                    PrimDef::FunNpFloor => builtin_fns::call_floor,
                    _ => unreachable!(),
                };
                Ok(Some(func(ctx, (arg_ty, arg), ctx.primitives.float)?))
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
            Box::new(|ctx, _, fun, args| {
                let arg_ty = fun.0.args[0].ty;
                let arg = args[0].1.clone().to_basic_value_enum(ctx, arg_ty)?;
                Ok(Some(builtin_fns::call_numpy_round(ctx, (arg_ty, arg))?))
            }),
        )
    }

    /// Build the `len()` function.
    fn build_len_function(&mut self) -> TopLevelDef {
        let prim = PrimDef::FunLen;

        // Type handled in [`Inferencer::try_fold_special_call`]
        let arg_tvar = self.unifier.get_dummy_var();

        TopLevelDef::Function {
            name: prim.name().into(),
            simple_name: prim.simple_name().into(),
            signature: self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg {
                    name: "obj".into(),
                    ty: arg_tvar.ty,
                    default_value: None,
                    is_vararg: false,
                }],
                ret: self.primitives.int32,
                vars: into_var_map([arg_tvar]),
            })),
            var_id: Vec::default(),
            attributes: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(move |ctx, _, fun, args| {
                let arg_ty = fun.0.args[0].ty;
                let arg = args[0].1.clone().to_basic_value_enum(ctx, arg_ty)?;

                builtin_fns::call_len(ctx, (arg_ty, arg)).map(|ret| Some(ret.into()))
            })))),
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
                    FuncArg {
                        name: "m".into(),
                        ty: self.num_ty.ty,
                        default_value: None,
                        is_vararg: false,
                    },
                    FuncArg {
                        name: "n".into(),
                        ty: self.num_ty.ty,
                        default_value: None,
                        is_vararg: false,
                    },
                ],
                ret: self.num_ty.ty,
                vars: self.num_var_map.clone(),
            })),
            var_id: Vec::default(),
            attributes: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(move |ctx, _, fun, args| {
                let m_ty = fun.0.args[0].ty;
                let n_ty = fun.0.args[1].ty;
                let m_val = args[0].1.clone().to_basic_value_enum(ctx, m_ty)?;
                let n_val = args[1].1.clone().to_basic_value_enum(ctx, n_ty)?;

                let func = match prim {
                    PrimDef::FunMin => builtin_fns::call_min,
                    PrimDef::FunMax => builtin_fns::call_max,
                    _ => unreachable!(),
                };
                Ok(Some(func(ctx, (m_ty, m_val), (n_ty, n_val))?))
            })))),
            loc: None,
        }
    }

    /// Build the functions `np_max()`, `np_min()`, `np_argmax()` and `np_argmin()`
    /// Calls `call_numpy_max_min` with the function name
    fn build_np_max_min_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(
            prim,
            &[PrimDef::FunNpArgmin, PrimDef::FunNpArgmax, PrimDef::FunNpMin, PrimDef::FunNpMax],
        );

        let (var_map, ret_ty) = match prim {
            PrimDef::FunNpArgmax | PrimDef::FunNpArgmin => {
                (self.num_or_ndarray_var_map.clone(), self.primitives.int64)
            }
            PrimDef::FunNpMax | PrimDef::FunNpMin => {
                let ret_ty = self.unifier.get_fresh_var(Some("R".into()), None);
                let var_map = self
                    .num_or_ndarray_var_map
                    .clone()
                    .into_iter()
                    .chain(once((ret_ty.id, ret_ty.ty)))
                    .collect::<IndexMap<_, _>>();
                (var_map, ret_ty.ty)
            }
            _ => unreachable!(),
        };

        create_fn_by_codegen(
            self.unifier,
            &var_map,
            prim.name(),
            ret_ty,
            &[(self.num_or_ndarray_ty.ty, "a")],
            Box::new(move |ctx, _, fun, args| {
                let a_ty = fun.0.args[0].ty;
                let a = args[0].1.clone().to_basic_value_enum(ctx, a_ty)?;

                Ok(Some(builtin_fns::call_numpy_max_min(ctx, (a_ty, a), prim.name())?))
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
                    .map(|p| FuncArg {
                        name: p.1.into(),
                        ty: p.0,
                        default_value: None,
                        is_vararg: false,
                    })
                    .collect(),
                ret: ret_ty.ty,
                vars: into_var_map([x1_ty, x2_ty, ret_ty]),
            })),
            var_id: vec![x1_ty.id, x2_ty.id],
            attributes: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(move |ctx, _, fun, args| {
                let x1_ty = fun.0.args[0].ty;
                let x2_ty = fun.0.args[1].ty;
                let x1_val = args[0].1.clone().to_basic_value_enum(ctx, x1_ty)?;
                let x2_val = args[1].1.clone().to_basic_value_enum(ctx, x2_ty)?;

                let func = match prim {
                    PrimDef::FunNpMinimum => builtin_fns::call_numpy_minimum,
                    PrimDef::FunNpMaximum => builtin_fns::call_numpy_maximum,
                    _ => unreachable!(),
                };

                Ok(Some(func(ctx, (x1_ty, x1_val), (x2_ty, x2_val))?))
            })))),
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
                    is_vararg: false,
                }],
                ret: self.num_or_ndarray_ty.ty,
                vars: self.num_or_ndarray_var_map.clone(),
            })),
            var_id: Vec::default(),
            attributes: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(|ctx, _, fun, args| {
                let n_ty = fun.0.args[0].ty;
                let n_val = args[0].1.clone().to_basic_value_enum(ctx, n_ty)?;

                Ok(Some(builtin_fns::call_abs(ctx, (n_ty, n_val))?))
            })))),
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
            Box::new(move |ctx, _, fun, args| {
                let x_ty = fun.0.args[0].ty;
                let x_val = args[0].1.clone().to_basic_value_enum(ctx, x_ty)?;

                let func = match prim {
                    PrimDef::FunNpIsInf => builtin_fns::call_numpy_isinf,
                    PrimDef::FunNpIsNan => builtin_fns::call_numpy_isnan,
                    _ => unreachable!(),
                };

                Ok(Some(func(ctx, (x_ty, x_val))?))
            }),
        )
    }

    fn build_np_any_all_function(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(prim, &[PrimDef::FunNpAny, PrimDef::FunNpAll]);

        let param_ty = &[(self.num_or_ndarray_ty.ty, "a")];
        let ret_ty = self.primitives.bool;
        let var_map = &self.num_or_ndarray_var_map;
        let codegen_callback: Box<GenCallCallback> = Box::new(move |ctx, _, fun, args| {
            let llvm_i1 = ctx.i1;
            let llvm_i1_k0 = llvm_i1.const_zero();
            let llvm_i1_k1 = llvm_i1.const_all_ones();

            let a_ty = fun.0.args[0].ty;
            let a_val = args[0].1.clone().to_basic_value_enum(ctx, a_ty)?;
            let a = ScalarOrNDArray::from_value(ctx, (a_ty, a_val));
            let a_elem_ty = arraylike_flatten_element_type(&mut ctx.unifier, a_ty);

            let (init, sc_val) = match prim {
                PrimDef::FunNpAny => (llvm_i1_k0, llvm_i1_k1),
                PrimDef::FunNpAll => (llvm_i1_k1, llvm_i1_k0),
                _ => unreachable!(),
            };

            let acc = a.fold(ctx, init, |ctx, hooks, acc, elem| {
                gen_if_callback(
                    &mut (),
                    ctx,
                    |(), ctx| {
                        Ok(ctx.builder.build_int_compare(IntPredicate::EQ, acc, sc_val, "")?)
                    },
                    |(), ctx| {
                        if let Some(hooks) = hooks {
                            hooks.build_break_branch(ctx.builder)?;
                        }
                        Ok(())
                    },
                    |(), _| Ok(()),
                )?;

                let is_truthy = builtin_fns::call_bool(ctx, (a_elem_ty, elem))?.into_int_value();

                Ok(match prim {
                    PrimDef::FunNpAny => ctx.builder.build_or(acc, is_truthy, "")?,
                    PrimDef::FunNpAll => ctx.builder.build_and(acc, is_truthy, "")?,
                    _ => unreachable!(),
                })
            })?;

            Ok(Some(acc.as_basic_value_enum()))
        });

        create_fn_by_codegen(self.unifier, var_map, prim.name(), ret_ty, param_ty, codegen_callback)
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
            Box::new(move |ctx, _, fun, args| {
                let arg_ty = fun.0.args[0].ty;
                let arg_val = args[0].1.clone().to_basic_value_enum(ctx, arg_ty)?;

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
                Ok(Some(func(ctx, (arg_ty, arg_val))?))
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
                    .map(|p| FuncArg {
                        name: p.1.into(),
                        ty: p.0,
                        default_value: None,
                        is_vararg: false,
                    })
                    .collect(),
                ret: ret_ty.ty,
                vars: into_var_map([x1_ty, x2_ty, ret_ty]),
            })),
            var_id: vec![ret_ty.id],
            attributes: Vec::default(),
            instance_to_symbol: HashMap::default(),
            instance_to_stmt: HashMap::default(),
            resolver: None,
            codegen_callback: Some(Arc::new(GenCall::new(Box::new(move |ctx, _, fun, args| {
                let x1_ty = fun.0.args[0].ty;
                let x1_val = args[0].1.clone().to_basic_value_enum(ctx, x1_ty)?;
                let x2_ty = fun.0.args[1].ty;
                let x2_val = args[1].1.clone().to_basic_value_enum(ctx, x2_ty)?;

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

                Ok(Some(func(ctx, (x1_ty, x1_val), (x2_ty, x2_val))?))
            })))),
            loc: None,
        }
    }

    /// Build `np_linalg` and `sp_linalg` functions
    ///
    /// The input to these functions must be floating point `NDArray`
    fn build_linalg_methods(&mut self, prim: PrimDef) -> TopLevelDef {
        debug_assert_prim_is_allowed(
            prim,
            &[
                PrimDef::FunNpDot,
                PrimDef::FunNpLinalgCholesky,
                PrimDef::FunNpLinalgQr,
                PrimDef::FunNpLinalgSvd,
                PrimDef::FunNpLinalgInv,
                PrimDef::FunNpLinalgPinv,
                PrimDef::FunNpLinalgMatrixPower,
                PrimDef::FunNpLinalgDet,
                PrimDef::FunSpLinalgLu,
                PrimDef::FunSpLinalgSchur,
                PrimDef::FunSpLinalgHessenberg,
            ],
        );

        match prim {
            PrimDef::FunNpDot => create_fn_by_codegen(
                self.unifier,
                &self.num_or_ndarray_var_map,
                prim.name(),
                self.num_ty.ty,
                &[(self.num_or_ndarray_ty.ty, "x1"), (self.num_or_ndarray_ty.ty, "x2")],
                Box::new(move |ctx, _, fun, args| {
                    let x1_ty = fun.0.args[0].ty;
                    let x1_val = args[0].1.clone().to_basic_value_enum(ctx, x1_ty)?;

                    let x2_ty = fun.0.args[1].ty;
                    let x2_val = args[1].1.clone().to_basic_value_enum(ctx, x2_ty)?;

                    let result = ndarray_dot(ctx, (x1_ty, x1_val), (x2_ty, x2_val))?;
                    Ok(Some(result))
                }),
            ),

            PrimDef::FunNpLinalgCholesky | PrimDef::FunNpLinalgInv | PrimDef::FunNpLinalgPinv => {
                create_fn_by_codegen(
                    self.unifier,
                    &VarMap::new(),
                    prim.name(),
                    self.ndarray_float_2d,
                    &[(self.ndarray_float_2d, "x1")],
                    Box::new(move |ctx, _, fun, args| {
                        let x1_ty = fun.0.args[0].ty;
                        let x1_val = args[0].1.clone().to_basic_value_enum(ctx, x1_ty)?;

                        let func = match prim {
                            PrimDef::FunNpLinalgCholesky => builtin_fns::call_np_linalg_cholesky,
                            PrimDef::FunNpLinalgInv => builtin_fns::call_np_linalg_inv,
                            PrimDef::FunNpLinalgPinv => builtin_fns::call_np_linalg_pinv,
                            _ => unreachable!(),
                        };
                        Ok(Some(func(ctx, (x1_ty, x1_val))?))
                    }),
                )
            }

            PrimDef::FunNpLinalgQr
            | PrimDef::FunSpLinalgLu
            | PrimDef::FunSpLinalgSchur
            | PrimDef::FunSpLinalgHessenberg => {
                let ret_ty = self.unifier.add_ty(TypeEnum::TTuple {
                    ty: vec![self.ndarray_float_2d, self.ndarray_float_2d],
                    is_vararg_ctx: false,
                });
                create_fn_by_codegen(
                    self.unifier,
                    &VarMap::new(),
                    prim.name(),
                    ret_ty,
                    &[(self.ndarray_float_2d, "x1")],
                    Box::new(move |ctx, _, fun, args| {
                        let x1_ty = fun.0.args[0].ty;
                        let x1_val = args[0].1.clone().to_basic_value_enum(ctx, x1_ty)?;

                        let func = match prim {
                            PrimDef::FunNpLinalgQr => builtin_fns::call_np_linalg_qr,
                            PrimDef::FunSpLinalgLu => builtin_fns::call_sp_linalg_lu,
                            PrimDef::FunSpLinalgSchur => builtin_fns::call_sp_linalg_schur,
                            PrimDef::FunSpLinalgHessenberg => {
                                builtin_fns::call_sp_linalg_hessenberg
                            }
                            _ => unreachable!(),
                        };
                        Ok(Some(func(ctx, (x1_ty, x1_val))?))
                    }),
                )
            }

            PrimDef::FunNpLinalgSvd => {
                let ret_ty = self.unifier.add_ty(TypeEnum::TTuple {
                    ty: vec![self.ndarray_float_2d, self.ndarray_float, self.ndarray_float_2d],
                    is_vararg_ctx: false,
                });
                create_fn_by_codegen(
                    self.unifier,
                    &VarMap::new(),
                    prim.name(),
                    ret_ty,
                    &[(self.ndarray_float_2d, "x1")],
                    Box::new(move |ctx, _, fun, args| {
                        let x1_ty = fun.0.args[0].ty;
                        let x1_val = args[0].1.clone().to_basic_value_enum(ctx, x1_ty)?;

                        Ok(Some(builtin_fns::call_np_linalg_svd(ctx, (x1_ty, x1_val))?))
                    }),
                )
            }
            PrimDef::FunNpLinalgMatrixPower => create_fn_by_codegen(
                self.unifier,
                &VarMap::new(),
                prim.name(),
                self.ndarray_float_2d,
                &[(self.ndarray_float_2d, "x1"), (self.primitives.int32, "power")],
                Box::new(move |ctx, _, fun, args| {
                    let x1_ty = fun.0.args[0].ty;
                    let x1_val = args[0].1.clone().to_basic_value_enum(ctx, x1_ty)?;
                    let x2_ty = fun.0.args[1].ty;
                    let x2_val = args[1].1.clone().to_basic_value_enum(ctx, x2_ty)?;

                    Ok(Some(builtin_fns::call_np_linalg_matrix_power(
                        ctx,
                        (x1_ty, x1_val),
                        (x2_ty, x2_val),
                    )?))
                }),
            ),
            PrimDef::FunNpLinalgDet => create_fn_by_codegen(
                self.unifier,
                &VarMap::new(),
                prim.name(),
                self.primitives.float,
                &[(self.ndarray_float_2d, "x1")],
                Box::new(move |ctx, _, fun, args| {
                    let x1_ty = fun.0.args[0].ty;
                    let x1_val = args[0].1.clone().to_basic_value_enum(ctx, x1_ty)?;
                    Ok(Some(builtin_fns::call_np_linalg_det(ctx, (x1_ty, x1_val))?))
                }),
            ),
            _ => unreachable!(),
        }
    }

    fn create_method(prim: PrimDef, method_ty: Type) -> (StrRef, Type, DefinitionId) {
        (prim.simple_name().into(), method_ty, prim.id())
    }

    fn new_type_or_ndarray_ty(&mut self, scalar_ty: Type) -> TypeVar {
        let ndarray = make_ndarray_ty(self.unifier, self.primitives, Some(scalar_ty), None);

        self.unifier.get_fresh_var_with_range(&[scalar_ty, ndarray], Some("T".into()), None)
    }
}
