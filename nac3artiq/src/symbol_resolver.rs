use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering::Relaxed},
        Arc,
    },
};

use itertools::Itertools;
use parking_lot::RwLock;
use pyo3::{
    types::{PyDict, PyTuple},
    PyAny, PyErr, PyObject, PyResult, Python,
};

use super::PrimitivePythonId;
use nac3core::{
    codegen::{
        types::{ndarray::NDArrayType, ProxyType},
        values::ndarray::make_contiguous_strides,
        CodeGenContext, CodeGenerator,
    },
    inkwell::{
        module::Linkage,
        types::{BasicType, BasicTypeEnum},
        values::BasicValueEnum,
        AddressSpace,
    },
    nac3parser::ast::{self, StrRef},
    symbol_resolver::{StaticValue, SymbolResolver, SymbolValue, ValueEnum},
    toplevel::{
        helper::PrimDef,
        numpy::{make_ndarray_ty, unpack_ndarray_var_tys},
        DefinitionId, TopLevelDef,
    },
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{into_var_map, iter_type_vars, Type, TypeEnum, TypeVar, Unifier, VarMap},
    },
};

pub enum PrimitiveValue {
    I32(i32),
    I64(i64),
    U32(u32),
    U64(u64),
    F64(f64),
    Bool(bool),
    Str(String),
}

/// An entry in the [`DeferredEvaluationStore`], containing the deferred types, a [`PyObject`]
/// representing the `__constraints__` of the type variables, and the name of the type to be
/// instantiated.
type DeferredEvaluationEntry = (Vec<Type>, PyObject, String);

#[derive(Clone)]
pub struct DeferredEvaluationStore {
    needs_defer: Arc<AtomicBool>,
    store: Arc<RwLock<Vec<DeferredEvaluationEntry>>>,
}

impl DeferredEvaluationStore {
    pub fn new() -> Self {
        DeferredEvaluationStore {
            needs_defer: Arc::new(AtomicBool::new(true)),
            store: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

/// A class field as stored in the [`InnerResolver`], represented by the ID and name of the
/// associated [`PythonValue`].
type ResolverField = (u64, StrRef);
/// A class field as stored in Python, represented by the `id()` and [`PyObject`] of the field.
type PyFieldHandle = (u64, PyObject);

pub struct InnerResolver {
    pub id_to_type: RwLock<HashMap<StrRef, Type>>,
    pub id_to_def: RwLock<HashMap<StrRef, DefinitionId>>,
    pub id_to_pyval: RwLock<HashMap<StrRef, (u64, PyObject)>>,
    pub id_to_primitive: RwLock<HashMap<u64, PrimitiveValue>>,
    pub field_to_val: RwLock<HashMap<ResolverField, Option<PyFieldHandle>>>,
    pub global_value_ids: Arc<RwLock<HashMap<u64, PyObject>>>,
    pub pyid_to_def: Arc<RwLock<HashMap<u64, DefinitionId>>>,
    pub pyid_to_type: Arc<RwLock<HashMap<u64, Type>>>,
    pub primitive_ids: PrimitivePythonId,
    pub helper: PythonHelper,
    pub string_store: Arc<RwLock<HashMap<String, i32>>>,
    pub exception_ids: Arc<RwLock<HashMap<usize, usize>>>,
    pub deferred_eval_store: DeferredEvaluationStore,
    // module specific
    pub name_to_pyid: HashMap<StrRef, u64>,
    pub module: PyObject,
}

pub struct Resolver(pub Arc<InnerResolver>);

#[derive(Clone)]
pub struct PythonHelper {
    pub type_fn: PyObject,
    pub len_fn: PyObject,
    pub id_fn: PyObject,
    pub origin_ty_fn: PyObject,
    pub args_ty_fn: PyObject,
    pub store_obj: PyObject,
    pub store_str: PyObject,
}

struct PythonValue {
    id: u64,
    value: PyObject,
    store_obj: PyObject,
    resolver: Arc<InnerResolver>,
}

impl StaticValue for PythonValue {
    fn get_unique_identifier(&self) -> u64 {
        self.id
    }

    fn get_const_obj<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        _: &mut dyn CodeGenerator,
    ) -> BasicValueEnum<'ctx> {
        ctx.module.get_global(format!("{}_const", self.id).as_str()).map_or_else(
            || {
                Python::with_gil(|py| -> PyResult<BasicValueEnum<'ctx>> {
                    let id: u32 = self.store_obj.call1(py, (self.value.clone(),))?.extract(py)?;
                    let struct_type = ctx.ctx.struct_type(&[ctx.ctx.i32_type().into()], false);
                    let global = ctx.module.add_global(
                        struct_type,
                        None,
                        format!("{}_const", self.id).as_str(),
                    );
                    global.set_constant(true);
                    // Set linkage of global to private to avoid name collisions
                    global.set_linkage(Linkage::Private);
                    global.set_initializer(&ctx.ctx.const_struct(
                        &[ctx.ctx.i32_type().const_int(u64::from(id), false).into()],
                        false,
                    ));
                    Ok(global.as_pointer_value().into())
                })
                .unwrap()
            },
            |val| val.as_pointer_value().into(),
        )
    }

    fn to_basic_value_enum<'ctx, 'a>(
        &self,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        generator: &mut dyn CodeGenerator,
        expected_ty: Type,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        if let Some(val) = self.resolver.id_to_primitive.read().get(&self.id) {
            return Ok(match val {
                PrimitiveValue::I32(val) => ctx.ctx.i32_type().const_int(*val as u64, false).into(),
                PrimitiveValue::I64(val) => ctx.ctx.i64_type().const_int(*val as u64, false).into(),
                PrimitiveValue::U32(val) => {
                    ctx.ctx.i32_type().const_int(u64::from(*val), false).into()
                }
                PrimitiveValue::U64(val) => ctx.ctx.i64_type().const_int(*val, false).into(),
                PrimitiveValue::F64(val) => ctx.ctx.f64_type().const_float(*val).into(),
                PrimitiveValue::Bool(val) => {
                    ctx.ctx.i8_type().const_int(u64::from(*val), false).into()
                }
                PrimitiveValue::Str(val) => ctx.gen_string(generator, val).into(),
            });
        }
        if let Some(global) = ctx.module.get_global(&self.id.to_string()) {
            return Ok(global.as_pointer_value().into());
        }

        Python::with_gil(|py| -> PyResult<BasicValueEnum<'ctx>> {
            self.resolver
                .get_obj_value(py, self.value.as_ref(py), ctx, generator, expected_ty)
                .map(Option::unwrap)
        })
        .map_err(|e| e.to_string())
    }

    fn get_field<'ctx>(
        &self,
        name: StrRef,
        ctx: &mut CodeGenContext<'ctx, '_>,
    ) -> Option<ValueEnum<'ctx>> {
        {
            let field_to_val = self.resolver.field_to_val.read();
            field_to_val.get(&(self.id, name)).cloned()
        }
        .unwrap_or_else(|| {
            Python::with_gil(|py| -> PyResult<Option<(u64, PyObject)>> {
                let helper = &self.resolver.helper;
                let ty = helper.type_fn.call1(py, (&self.value,))?;
                let ty_id: u64 = helper.id_fn.call1(py, (ty,))?.extract(py)?;
                // for optimizing unwrap KernelInvariant
                if ty_id == self.resolver.primitive_ids.option && name == "_nac3_option".into() {
                    let obj = self.value.getattr(py, name.to_string().as_str())?;
                    let id = self.resolver.helper.id_fn.call1(py, (&obj,))?.extract(py)?;
                    return if self.id == self.resolver.primitive_ids.none {
                        Ok(None)
                    } else {
                        Ok(Some((id, obj)))
                    };
                }
                let def_id = { *self.resolver.pyid_to_def.read().get(&ty_id).unwrap() };
                let mut mutable = true;
                let defs = ctx.top_level.definitions.read();
                if let TopLevelDef::Class { fields, .. } = &*defs[def_id.0].read() {
                    for (field_name, _, is_mutable) in fields {
                        if field_name == &name {
                            mutable = *is_mutable;
                            break;
                        }
                    }
                }
                let result = if mutable {
                    None
                } else {
                    let obj = self.value.getattr(py, name.to_string().as_str())?;
                    let id = self.resolver.helper.id_fn.call1(py, (&obj,))?.extract(py)?;
                    Some((id, obj))
                };
                self.resolver.field_to_val.write().insert((self.id, name), result.clone());
                Ok(result)
            })
            .unwrap()
        })
        .map(|(id, obj)| {
            ValueEnum::Static(Arc::new(PythonValue {
                id,
                value: obj,
                store_obj: self.store_obj.clone(),
                resolver: self.resolver.clone(),
            }))
        })
    }

    fn get_tuple_element<'ctx>(&self, index: u32) -> Option<ValueEnum<'ctx>> {
        Python::with_gil(|py| -> PyResult<Option<(u64, PyObject)>> {
            let helper = &self.resolver.helper;
            let ty = helper.type_fn.call1(py, (&self.value,))?;
            let ty_id: u64 = helper.id_fn.call1(py, (ty,))?.extract(py)?;
            assert_eq!(ty_id, self.resolver.primitive_ids.tuple);
            let tup: &PyTuple = self.value.extract(py)?;
            let elem = tup.get_item(index as usize)?;
            let id = self.resolver.helper.id_fn.call1(py, (elem,))?.extract(py)?;
            Ok(Some((id, elem.into())))
        })
        .unwrap()
        .map(|(id, obj)| {
            ValueEnum::Static(Arc::new(PythonValue {
                id,
                value: obj,
                store_obj: self.store_obj.clone(),
                resolver: self.resolver.clone(),
            }))
        })
    }
}

impl InnerResolver {
    fn get_list_elem_type(
        &self,
        py: Python,
        list: &PyAny,
        len: usize,
        unifier: &mut Unifier,
        defs: &[Arc<RwLock<TopLevelDef>>],
        primitives: &PrimitiveStore,
    ) -> PyResult<Result<Type, String>> {
        let mut ty = match self.get_obj_type(py, list.get_item(0)?, unifier, defs, primitives)? {
            Ok(t) => t,
            Err(e) => return Ok(Err(format!("type error ({e}) at element #0 of the list"))),
        };
        for i in 1..len {
            let b = match list
                .get_item(i)
                .map(|elem| self.get_obj_type(py, elem, unifier, defs, primitives))??
            {
                Ok(t) => t,
                Err(e) => return Ok(Err(format!("type error ({e}) at element #{i} of the list"))),
            };
            ty = match unifier.unify(ty, b) {
                Ok(()) => ty,
                Err(e) => {
                    return Ok(Err(format!(
                        "inhomogeneous type ({}) at element #{i} of the list",
                        e.to_display(unifier)
                    )))
                }
            };
        }
        Ok(Ok(ty))
    }

    /// Handles python objects that represent types themselves,
    ///
    /// Primitives and class types should be themselves, use `ty_id` to check;
    /// `TypeVars` and `GenericAlias`(`A[int, bool]`) should use `ty_ty_id` to check.
    ///
    /// The `bool` value returned indicates whether they are instantiated or not
    fn get_pyty_obj_type(
        &self,
        py: Python,
        pyty: &PyAny,
        unifier: &mut Unifier,
        defs: &[Arc<RwLock<TopLevelDef>>],
        primitives: &PrimitiveStore,
    ) -> PyResult<Result<(Type, bool), String>> {
        let ty_id: u64 = self.helper.id_fn.call1(py, (pyty,))?.extract(py)?;
        let ty_ty_id: u64 =
            self.helper.id_fn.call1(py, (self.helper.type_fn.call1(py, (pyty,))?,))?.extract(py)?;

        if ty_id == self.primitive_ids.int || ty_id == self.primitive_ids.int32 {
            Ok(Ok((primitives.int32, true)))
        } else if ty_id == self.primitive_ids.int64 {
            Ok(Ok((primitives.int64, true)))
        } else if ty_id == self.primitive_ids.uint32 {
            Ok(Ok((primitives.uint32, true)))
        } else if ty_id == self.primitive_ids.uint64 {
            Ok(Ok((primitives.uint64, true)))
        } else if ty_id == self.primitive_ids.bool || ty_id == self.primitive_ids.np_bool_ {
            Ok(Ok((primitives.bool, true)))
        } else if ty_id == self.primitive_ids.string || ty_id == self.primitive_ids.np_str_ {
            Ok(Ok((primitives.str, true)))
        } else if ty_id == self.primitive_ids.float || ty_id == self.primitive_ids.float64 {
            Ok(Ok((primitives.float, true)))
        } else if ty_id == self.primitive_ids.exception {
            Ok(Ok((primitives.exception, true)))
        } else if ty_id == self.primitive_ids.list {
            // do not handle type var param and concrete check here
            let list_tvar = if let TypeEnum::TObj { obj_id, params, .. } =
                &*unifier.get_ty_immutable(primitives.list)
            {
                assert_eq!(*obj_id, PrimDef::List.id());
                iter_type_vars(params).nth(0).unwrap()
            } else {
                unreachable!()
            };

            let var = unifier.get_dummy_var().ty;
            let list = unifier
                .subst(primitives.list, &into_var_map([TypeVar { id: list_tvar.id, ty: var }]))
                .unwrap();
            Ok(Ok((list, false)))
        } else if ty_id == self.primitive_ids.ndarray {
            // do not handle type var param and concrete check here
            let var = unifier.get_dummy_var().ty;
            let ndims = unifier.get_fresh_const_generic_var(primitives.usize(), None, None).ty;
            let ndarray = make_ndarray_ty(unifier, primitives, Some(var), Some(ndims));
            Ok(Ok((ndarray, false)))
        } else if ty_id == self.primitive_ids.tuple {
            // do not handle type var param and concrete check here
            Ok(Ok((unifier.add_ty(TypeEnum::TTuple { ty: vec![], is_vararg_ctx: false }), false)))
        } else if ty_id == self.primitive_ids.option {
            Ok(Ok((primitives.option, false)))
        } else if ty_id == self.primitive_ids.none {
            unreachable!("none cannot be typeid")
        } else if let Some(def_id) = self.pyid_to_def.read().get(&ty_id).copied() {
            let def = defs[def_id.0].read();
            let TopLevelDef::Class { object_id, type_vars, fields, methods, .. } = &*def else {
                // only object is supported, functions are not supported
                unreachable!("function type is not supported, should not be queried")
            };

            // do not handle type var param and concrete check here, and no subst
            Ok(Ok({
                let ty = TypeEnum::TObj {
                    obj_id: *object_id,
                    params: type_vars
                        .iter()
                        .map(|x| {
                            let TypeEnum::TVar { id, .. } = &*unifier.get_ty(*x) else {
                                unreachable!()
                            };

                            (*id, *x)
                        })
                        .collect(),
                    fields: {
                        let mut res = methods
                            .iter()
                            .map(|(iden, ty, _)| (*iden, (*ty, false)))
                            .collect::<HashMap<_, _>>();
                        res.extend(fields.clone().into_iter().map(|x| (x.0, (x.1, x.2))));
                        res
                    },
                };
                // here also false, later instantiation use python object to check compatible
                (unifier.add_ty(ty), false)
            }))
        } else if ty_ty_id == self.primitive_ids.typevar {
            let name: &str = pyty.getattr("__name__").unwrap().extract().unwrap();
            let (constraint_types, is_const_generic) = {
                let constraints = pyty.getattr("__constraints__").unwrap();
                let mut result: Vec<Type> = vec![];
                let needs_defer = self.deferred_eval_store.needs_defer.load(Relaxed);

                let mut is_const_generic = false;
                for i in 0usize.. {
                    if let Ok(constr) = constraints.get_item(i) {
                        let constr_id: u64 = self.helper.id_fn.call1(py, (constr,))?.extract(py)?;
                        if constr_id == self.primitive_ids.const_generic_marker {
                            is_const_generic = true;
                            continue;
                        }

                        if !is_const_generic && needs_defer {
                            result.push(unifier.get_dummy_var().ty);
                        } else {
                            result.push({
                                match self.get_pyty_obj_type(py, constr, unifier, defs, primitives)? {
                                    Ok((ty, _)) => {
                                        if unifier.is_concrete(ty, &[]) {
                                            ty
                                        } else {
                                            return Ok(Err(format!(
                                                "the {}th constraint of TypeVar `{}` is not concrete",
                                                i + 1,
                                                pyty.getattr("__name__")?.extract::<String>()?
                                            )));
                                        }
                                    }
                                    Err(err) => return Ok(Err(err)),
                                }
                            });
                        }
                    } else {
                        break;
                    }
                }

                if !is_const_generic && needs_defer {
                    self.deferred_eval_store.store.write().push((
                        result.clone(),
                        constraints.extract()?,
                        pyty.getattr("__name__")?.extract::<String>()?,
                    ));
                }

                (result, is_const_generic)
            };

            let res = if is_const_generic {
                if constraint_types.len() != 1 {
                    return Ok(Err(format!(
                        "ConstGeneric expects 1 argument, got {}",
                        constraint_types.len()
                    )));
                }

                unifier.get_fresh_const_generic_var(constraint_types[0], Some(name.into()), None).ty
            } else {
                unifier.get_fresh_var_with_range(&constraint_types, Some(name.into()), None).ty
            };

            Ok(Ok((res, true)))
        } else if ty_ty_id == self.primitive_ids.generic_alias.0
            || ty_ty_id == self.primitive_ids.generic_alias.1
        {
            let origin = self.helper.origin_ty_fn.call1(py, (pyty,))?;
            let args = self.helper.args_ty_fn.call1(py, (pyty,))?;
            let args: &PyTuple = args.downcast(py)?;
            let origin_ty =
                match self.get_pyty_obj_type(py, origin.as_ref(py), unifier, defs, primitives)? {
                    Ok((ty, false)) => ty,
                    Ok((_, true)) => {
                        return Ok(Err("instantiated type does not take type parameters".into()))
                    }
                    Err(err) => return Ok(Err(err)),
                };

            match &*unifier.get_ty(origin_ty) {
                TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::List.id() => {
                    if args.len() == 1 {
                        let ty = match self.get_pyty_obj_type(
                            py,
                            args.get_item(0)?,
                            unifier,
                            defs,
                            primitives,
                        )? {
                            Ok(ty) => ty,
                            Err(err) => return Ok(Err(err)),
                        };
                        if !unifier.is_concrete(ty.0, &[]) && !ty.1 {
                            return Ok(Err(
                                "type list should take concrete parameters in typevar range".into(),
                            ));
                        }
                        let list_tvar = if let TypeEnum::TObj { obj_id, params, .. } =
                            &*unifier.get_ty_immutable(primitives.list)
                        {
                            assert_eq!(*obj_id, PrimDef::List.id());
                            iter_type_vars(params).nth(0).unwrap()
                        } else {
                            unreachable!()
                        };
                        let list = unifier
                            .subst(
                                primitives.list,
                                &into_var_map([TypeVar { id: list_tvar.id, ty: ty.0 }]),
                            )
                            .unwrap();
                        Ok(Ok((list, true)))
                    } else {
                        return Ok(Err(format!(
                            "type list needs exactly 1 type parameters, found {}",
                            args.len()
                        )));
                    }
                }
                TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
                    if args.len() != 2 {
                        return Ok(Err(format!(
                            "type list needs exactly 2 type parameters, found {}",
                            args.len()
                        )));
                    }

                    // npt.NDArray[T] == np.ndarray[Any, np.dtype[T]]
                    let ndarray_dtype_pyty =
                        self.helper.args_ty_fn.call1(py, (args.get_item(1)?,))?;
                    let dtype = ndarray_dtype_pyty.downcast::<PyTuple>(py)?.get_item(0)?;

                    let ty = match self.get_pyty_obj_type(py, dtype, unifier, defs, primitives)? {
                        Ok(ty) => ty,
                        Err(err) => return Ok(Err(err)),
                    };

                    if !unifier.is_concrete(ty.0, &[]) && !ty.1 {
                        return Ok(Err(
                            "type `ndarray` should take concrete parameters for dtype".into()
                        ));
                    }

                    Ok(Ok((make_ndarray_ty(unifier, primitives, Some(ty.0), None), true)))
                }
                TypeEnum::TTuple { .. } => {
                    let args = match args
                        .iter()
                        .map(|x| self.get_pyty_obj_type(py, x, unifier, defs, primitives))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .collect::<Result<Vec<_>, _>>() {
                            Ok(args) if !args.is_empty() => args
                                .into_iter()
                                .map(|(x, check)| if !unifier.is_concrete(x, &[]) && !check {
                                        panic!("type tuple should take concrete parameters in type var ranges")
                                    } else {
                                        x
                                    }
                                )
                                .collect::<Vec<_>>(),
                            Err(err) => return Ok(Err(err)),
                            _ => return Ok(Err("tuple type needs at least 1 type parameters".to_string()))
                        };
                    Ok(Ok((
                        unifier.add_ty(TypeEnum::TTuple { ty: args, is_vararg_ctx: false }),
                        true,
                    )))
                }
                TypeEnum::TObj { params, obj_id, .. } => {
                    let subst = {
                        if params.len() != args.len() {
                            return Ok(Err(format!(
                                "for class #{}, expect {} type parameters, got {}.",
                                obj_id.0,
                                params.len(),
                                args.len(),
                            )));
                        }
                        let args = match args
                            .iter()
                            .map(|x| self.get_pyty_obj_type(py, x, unifier, defs, primitives))
                            .collect::<Result<Vec<_>, _>>()?
                            .into_iter()
                            .collect::<Result<Vec<_>, _>>() {
                                Ok(args) => args
                                .into_iter()
                                .map(|(x, check)| if !unifier.is_concrete(x, &[]) && !check {
                                        panic!("type class should take concrete parameters in type var ranges")
                                    } else {
                                        x
                                    }
                                )
                                .collect::<Vec<_>>(),
                                Err(err) => return Ok(Err(err)),
                            };
                        params
                            .iter()
                            .zip(args.iter())
                            .map(|((id, _), ty)| (*id, *ty))
                            .collect::<VarMap>()
                    };
                    Ok(Ok((unifier.subst(origin_ty, &subst).unwrap_or(origin_ty), true)))
                }
                TypeEnum::TVirtual { .. } => {
                    if args.len() == 1 {
                        let ty = match self.get_pyty_obj_type(
                            py,
                            args.get_item(0)?,
                            unifier,
                            defs,
                            primitives,
                        )? {
                            Ok(ty) => ty,
                            Err(err) => return Ok(Err(err)),
                        };
                        assert!(
                            unifier.is_concrete(ty.0, &[]) || ty.1,
                            "virtual class should take concrete parameters in type var ranges"
                        );
                        Ok(Ok((unifier.add_ty(TypeEnum::TVirtual { ty: ty.0 }), true)))
                    } else {
                        return Ok(Err(format!(
                            "virtual class needs exactly 1 type parameters, found {}",
                            args.len()
                        )));
                    }
                }
                _ => unimplemented!(),
            }
        } else if ty_id == self.primitive_ids.virtual_id {
            Ok(Ok((
                {
                    let ty = TypeEnum::TVirtual { ty: unifier.get_dummy_var().ty };
                    unifier.add_ty(ty)
                },
                false,
            )))
        } else {
            let str_fn =
                pyo3::types::PyModule::import(py, "builtins").unwrap().getattr("repr").unwrap();
            let str_repr: String = str_fn.call1((pyty,)).unwrap().extract().unwrap();
            Ok(Err(format!("{str_repr} is not registered with NAC3 (@nac3 decorator missing?)")))
        }
    }

    pub fn get_obj_type(
        &self,
        py: Python,
        obj: &PyAny,
        unifier: &mut Unifier,
        defs: &[Arc<RwLock<TopLevelDef>>],
        primitives: &PrimitiveStore,
    ) -> PyResult<Result<Type, String>> {
        let ty = self.helper.type_fn.call1(py, (obj,)).unwrap();
        let py_obj_id: u64 = self.helper.id_fn.call1(py, (obj,))?.extract(py)?;
        if let Some(ty) = self.pyid_to_type.read().get(&py_obj_id) {
            return Ok(Ok(*ty));
        }

        // check if constructor function exists in the methods list
        let pyid_to_def = self.pyid_to_def.read();
        let constructor_ty = pyid_to_def.get(&py_obj_id).and_then(|def_id| {
            defs.iter().find_map(|def| {
                if let Some(rear_guard) = def.try_read() {
                    if let TopLevelDef::Class { object_id, methods, constructor, .. } = &*rear_guard
                    {
                        if object_id == def_id
                            && constructor.is_some()
                            && methods.iter().any(|(s, _, _)| s == &"__init__".into())
                        {
                            return *constructor;
                        }
                    }
                }
                None
            })
        });

        if let Some(ty) = constructor_ty {
            self.pyid_to_type.write().insert(py_obj_id, ty);
            return Ok(Ok(ty));
        }
        let (extracted_ty, inst_check) = match self.get_pyty_obj_type(
            py,
            {
                if [
                    self.primitive_ids.typevar,
                    self.primitive_ids.generic_alias.0,
                    self.primitive_ids.generic_alias.1,
                    self.primitive_ids.string,
                ]
                .contains(&self.helper.id_fn.call1(py, (ty.clone(),))?.extract::<u64>(py)?)
                {
                    obj
                } else {
                    ty.as_ref(py)
                }
            },
            unifier,
            defs,
            primitives,
        )? {
            Ok(s) => s,
            Err(e) => {
                // Allow access to Class Attributes of Classes without having to initialize Objects
                if self.pyid_to_def.read().contains_key(&py_obj_id) {
                    if let Some(def_id) = self.pyid_to_def.read().get(&py_obj_id).copied() {
                        let def = defs[def_id.0].read();
                        let TopLevelDef::Class { object_id, .. } = &*def else {
                            // only object is supported, functions are not supported
                            unreachable!("function type is not supported, should not be queried")
                        };

                        let ty = TypeEnum::TObj {
                            obj_id: *object_id,
                            params: VarMap::new(),
                            fields: HashMap::new(),
                        };
                        (unifier.add_ty(ty), true)
                    } else {
                        return Ok(Err(e));
                    }
                } else {
                    return Ok(Err(e));
                }
            }
        };
        match (&*unifier.get_ty(extracted_ty), inst_check) {
            // do the instantiation for these four types
            (TypeEnum::TObj { obj_id, params, .. }, false) if *obj_id == PrimDef::List.id() => {
                let ty = iter_type_vars(params).nth(0).unwrap().ty;
                let len: usize = self.helper.len_fn.call1(py, (obj,))?.extract(py)?;
                if len == 0 {
                    assert!(matches!(
                        &*unifier.get_ty(ty),
                        TypeEnum::TVar { fields: None, range, .. }
                            if range.is_empty()
                    ));
                    Ok(Ok(extracted_ty))
                } else {
                    let actual_ty =
                        self.get_list_elem_type(py, obj, len, unifier, defs, primitives)?;
                    match actual_ty {
                        Ok(t) => match unifier.unify(ty, t) {
                            Ok(()) => {
                                let list_tvar = if let TypeEnum::TObj { obj_id, params, .. } =
                                    &*unifier.get_ty_immutable(primitives.list)
                                {
                                    assert_eq!(*obj_id, PrimDef::List.id());
                                    iter_type_vars(params).nth(0).unwrap()
                                } else {
                                    unreachable!()
                                };
                                let list = unifier
                                    .subst(
                                        primitives.list,
                                        &into_var_map([TypeVar { id: list_tvar.id, ty }]),
                                    )
                                    .unwrap();
                                Ok(Ok(list))
                            }

                            Err(e) => Ok(Err(format!(
                                "type error ({}) for the list",
                                e.to_display(unifier)
                            ))),
                        },
                        Err(e) => Ok(Err(e)),
                    }
                }
            }
            (TypeEnum::TObj { obj_id, .. }, false) if *obj_id == PrimDef::NDArray.id() => {
                let (ty, ndims) = unpack_ndarray_var_tys(unifier, extracted_ty);
                let len: usize = obj.getattr("ndim")?.extract()?;
                if len == 0 {
                    assert!(matches!(
                        &*unifier.get_ty(ty),
                        TypeEnum::TVar { fields: None, range, .. }
                            if range.is_empty()
                    ));
                    Ok(Ok(extracted_ty))
                } else {
                    let dtype = obj.getattr("dtype")?.getattr("type")?;
                    let dtype_ty = self.get_pyty_obj_type(py, dtype, unifier, defs, primitives)?;
                    match dtype_ty {
                        Ok((t, _)) => match unifier.unify(ty, t) {
                            Ok(()) => {
                                let ndarray_ty =
                                    make_ndarray_ty(unifier, primitives, Some(ty), Some(ndims));

                                Ok(Ok(ndarray_ty))
                            }
                            Err(e) => Ok(Err(format!(
                                "type error ({}) for the ndarray",
                                e.to_display(unifier),
                            ))),
                        },
                        Err(e) => Ok(Err(e)),
                    }
                }
            }
            (TypeEnum::TTuple { .. }, false) => {
                let elements: &PyTuple = obj.downcast()?;
                let types: Result<Result<Vec<_>, _>, _> = elements
                    .iter()
                    .map(|elem| self.get_obj_type(py, elem, unifier, defs, primitives))
                    .collect();
                let types = types?;
                Ok(types.map(|types| {
                    unifier.add_ty(TypeEnum::TTuple { ty: types, is_vararg_ctx: false })
                }))
            }
            // special handling for option type since its class member layout in python side
            // is special and cannot be mapped directly to a nac3 type as below
            (TypeEnum::TObj { obj_id, params, .. }, false)
                if *obj_id == primitives.option.obj_id(unifier).unwrap() =>
            {
                let Ok(field_data) = obj.getattr("_nac3_option") else {
                    unreachable!("cannot be None")
                };
                // if is `none`
                let zelf_id: u64 = self.helper.id_fn.call1(py, (obj,))?.extract(py)?;
                if zelf_id == self.primitive_ids.none {
                    let ty_enum = unifier.get_ty_immutable(primitives.option);
                    let TypeEnum::TObj { params, .. } = ty_enum.as_ref() else {
                        unreachable!("must be tobj")
                    };

                    let var_map = into_var_map(iter_type_vars(params).map(|tvar| {
                        let TypeEnum::TVar { id, range, name, loc, .. } = &*unifier.get_ty(tvar.ty)
                        else {
                            unreachable!()
                        };

                        assert_eq!(*id, tvar.id);
                        let ty = unifier.get_fresh_var_with_range(range, *name, *loc).ty;
                        TypeVar { id: *id, ty }
                    }));
                    return Ok(Ok(unifier.subst(primitives.option, &var_map).unwrap()));
                }

                let ty = match self.get_obj_type(py, field_data, unifier, defs, primitives)? {
                    Ok(t) => t,
                    Err(e) => {
                        return Ok(Err(format!(
                            "error when getting type of the option object ({e})"
                        )))
                    }
                };
                let new_var_map: VarMap = params.iter().map(|(id, _)| (*id, ty)).collect();
                let res = unifier.subst(extracted_ty, &new_var_map).unwrap_or(extracted_ty);
                Ok(Ok(res))
            }
            (TypeEnum::TObj { params, fields, .. }, false) => {
                self.pyid_to_type.write().insert(py_obj_id, extracted_ty);
                let var_map = into_var_map(iter_type_vars(params).map(|tvar| {
                    let TypeEnum::TVar { id, range, name, loc, .. } = &*unifier.get_ty(tvar.ty)
                    else {
                        unreachable!()
                    };

                    assert_eq!(*id, tvar.id);
                    let ty = unifier.get_fresh_var_with_range(range, *name, *loc).ty;
                    TypeVar { id: *id, ty }
                }));
                let mut instantiate_obj = || {
                    // loop through non-function fields of the class to get the instantiated value
                    for field in fields {
                        let name: String = (*field.0).into();
                        if let TypeEnum::TFunc(..) = &*unifier.get_ty(field.1 .0) {
                            continue;
                        }
                        let field_data = match obj.getattr(name.as_str()) {
                            Ok(d) => d,
                            Err(e) => return Ok(Err(format!("{e}"))),
                        };
                        let ty =
                            match self.get_obj_type(py, field_data, unifier, defs, primitives)? {
                                Ok(t) => t,
                                Err(e) => {
                                    return Ok(Err(format!(
                                        "error when getting type of field `{name}` ({e})"
                                    )))
                                }
                            };
                        let field_ty = unifier.subst(field.1 .0, &var_map).unwrap_or(field.1 .0);
                        if let Err(e) = unifier.unify(ty, field_ty) {
                            // field type mismatch
                            return Ok(Err(format!(
                                "error when getting type of field `{name}` ({})",
                                e.to_display(unifier)
                            )));
                        }
                    }
                    for ty in var_map.values() {
                        // must be concrete type
                        if !unifier.is_concrete(*ty, &[]) {
                            return Ok(Err("object is not of concrete type".into()));
                        }
                    }
                    let extracted_ty =
                        unifier.subst(extracted_ty, &var_map).unwrap_or(extracted_ty);
                    Ok(Ok(extracted_ty))
                };
                let result = instantiate_obj();
                // update/remove the cache according to the result
                match result {
                    Ok(Ok(ty)) => self.pyid_to_type.write().insert(py_obj_id, ty),
                    _ => self.pyid_to_type.write().remove(&py_obj_id),
                };
                result
            }
            _ => {
                // check integer bounds
                if unifier.unioned(extracted_ty, primitives.int32) {
                    obj.extract::<i32>().map_or_else(
                        |_| Ok(Err(format!("{obj} is not in the range of int32"))),
                        |_| Ok(Ok(extracted_ty)),
                    )
                } else if unifier.unioned(extracted_ty, primitives.int64) {
                    obj.extract::<i64>().map_or_else(
                        |_| Ok(Err(format!("{obj} is not in the range of int64"))),
                        |_| Ok(Ok(extracted_ty)),
                    )
                } else if unifier.unioned(extracted_ty, primitives.uint32) {
                    obj.extract::<u32>().map_or_else(
                        |_| Ok(Err(format!("{obj} is not in the range of uint32"))),
                        |_| Ok(Ok(extracted_ty)),
                    )
                } else if unifier.unioned(extracted_ty, primitives.uint64) {
                    obj.extract::<u64>().map_or_else(
                        |_| Ok(Err(format!("{obj} is not in the range of uint64"))),
                        |_| Ok(Ok(extracted_ty)),
                    )
                } else if unifier.unioned(extracted_ty, primitives.bool) {
                    if obj.extract::<bool>().is_ok()
                        || obj.call_method("__bool__", (), None)?.extract::<bool>().is_ok()
                    {
                        Ok(Ok(extracted_ty))
                    } else {
                        Ok(Err(format!("{obj} is not in the range of bool")))
                    }
                } else if unifier.unioned(extracted_ty, primitives.float) {
                    obj.extract::<f64>().map_or_else(
                        |_| Ok(Err(format!("{obj} is not in the range of float64"))),
                        |_| Ok(Ok(extracted_ty)),
                    )
                } else if unifier.unioned(extracted_ty, primitives.str) {
                    obj.extract::<String>().map_or_else(
                        |_| Ok(Err(format!("{obj} is not a valid string"))),
                        |_| Ok(Ok(primitives.str)),
                    )
                } else {
                    Ok(Ok(extracted_ty))
                }
            }
        }
    }

    pub fn get_obj_value<'ctx>(
        &self,
        py: Python,
        obj: &PyAny,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
        expected_ty: Type,
    ) -> PyResult<Option<BasicValueEnum<'ctx>>> {
        let ty_id: u64 =
            self.helper.id_fn.call1(py, (self.helper.type_fn.call1(py, (obj,))?,))?.extract(py)?;
        let id: u64 = self.helper.id_fn.call1(py, (obj,))?.extract(py)?;
        if ty_id == self.primitive_ids.int || ty_id == self.primitive_ids.int32 {
            let val: i32 = obj.extract().unwrap();
            self.id_to_primitive.write().insert(id, PrimitiveValue::I32(val));
            Ok(Some(ctx.ctx.i32_type().const_int(val as u64, false).into()))
        } else if ty_id == self.primitive_ids.int64 {
            let val: i64 = obj.extract().unwrap();
            self.id_to_primitive.write().insert(id, PrimitiveValue::I64(val));
            Ok(Some(ctx.ctx.i64_type().const_int(val as u64, false).into()))
        } else if ty_id == self.primitive_ids.uint32 {
            let val: u32 = obj.extract().unwrap();
            self.id_to_primitive.write().insert(id, PrimitiveValue::U32(val));
            Ok(Some(ctx.ctx.i32_type().const_int(u64::from(val), false).into()))
        } else if ty_id == self.primitive_ids.uint64 {
            let val: u64 = obj.extract().unwrap();
            self.id_to_primitive.write().insert(id, PrimitiveValue::U64(val));
            Ok(Some(ctx.ctx.i64_type().const_int(val, false).into()))
        } else if ty_id == self.primitive_ids.bool {
            let val: bool = obj.extract().unwrap();
            self.id_to_primitive.write().insert(id, PrimitiveValue::Bool(val));
            Ok(Some(ctx.ctx.i8_type().const_int(u64::from(val), false).into()))
        } else if ty_id == self.primitive_ids.np_bool_ {
            let val: bool = obj.call_method("__bool__", (), None)?.extract().unwrap();
            self.id_to_primitive.write().insert(id, PrimitiveValue::Bool(val));
            Ok(Some(ctx.ctx.i8_type().const_int(u64::from(val), false).into()))
        } else if ty_id == self.primitive_ids.string || ty_id == self.primitive_ids.np_str_ {
            let val: String = obj.extract().unwrap();
            self.id_to_primitive.write().insert(id, PrimitiveValue::Str(val.clone()));
            return Ok(Some(ctx.gen_string(generator, val).into()));
        } else if ty_id == self.primitive_ids.float || ty_id == self.primitive_ids.float64 {
            let val: f64 = obj.extract().unwrap();
            self.id_to_primitive.write().insert(id, PrimitiveValue::F64(val));
            Ok(Some(ctx.ctx.f64_type().const_float(val).into()))
        } else if ty_id == self.primitive_ids.list {
            let id_str = id.to_string();

            if let Some(global) = ctx.module.get_global(&id_str) {
                return Ok(Some(global.as_pointer_value().into()));
            }

            let len: usize = self.helper.len_fn.call1(py, (obj,))?.extract(py)?;
            let elem_ty = match ctx.unifier.get_ty_immutable(expected_ty).as_ref() {
                TypeEnum::TObj { obj_id, params, .. } if *obj_id == PrimDef::List.id() => {
                    iter_type_vars(params).nth(0).unwrap().ty
                }
                _ => unreachable!("must be list"),
            };
            let size_t = generator.get_size_type(ctx.ctx);
            let ty = if len == 0
                && matches!(&*ctx.unifier.get_ty_immutable(elem_ty), TypeEnum::TVar { .. })
            {
                // The default type for zero-length lists of unknown element type is size_t
                size_t.into()
            } else {
                ctx.get_llvm_type(generator, elem_ty)
            };
            let arr_ty = ctx
                .ctx
                .struct_type(&[ty.ptr_type(AddressSpace::default()).into(), size_t.into()], false);

            {
                if self.global_value_ids.read().contains_key(&id) {
                    let global = ctx.module.get_global(&id_str).unwrap_or_else(|| {
                        ctx.module.add_global(arr_ty, Some(AddressSpace::default()), &id_str)
                    });
                    return Ok(Some(global.as_pointer_value().into()));
                }
                self.global_value_ids.write().insert(id, obj.into());
            }

            let arr: Result<Option<Vec<_>>, _> = (0..len)
                .map(|i| {
                    obj.get_item(i).and_then(|elem| {
                        self.get_obj_value(py, elem, ctx, generator, elem_ty).map_err(|e| {
                            super::CompileError::new_err(format!("Error getting element {i}: {e}"))
                        })
                    })
                })
                .collect();
            let arr = arr?.unwrap();

            let arr_global = ctx.module.add_global(
                ty.array_type(len as u32),
                Some(AddressSpace::default()),
                &(id_str.clone() + "_"),
            );
            let arr: BasicValueEnum = if ty.is_int_type() {
                let arr: Vec<_> = arr.into_iter().map(BasicValueEnum::into_int_value).collect();
                ty.into_int_type().const_array(&arr)
            } else if ty.is_float_type() {
                let arr: Vec<_> = arr.into_iter().map(BasicValueEnum::into_float_value).collect();
                ty.into_float_type().const_array(&arr)
            } else if ty.is_array_type() {
                let arr: Vec<_> = arr.into_iter().map(BasicValueEnum::into_array_value).collect();
                ty.into_array_type().const_array(&arr)
            } else if ty.is_struct_type() {
                let arr: Vec<_> = arr.into_iter().map(BasicValueEnum::into_struct_value).collect();
                ty.into_struct_type().const_array(&arr)
            } else if ty.is_pointer_type() {
                let arr: Vec<_> = arr.into_iter().map(BasicValueEnum::into_pointer_value).collect();
                ty.into_pointer_type().const_array(&arr)
            } else {
                unreachable!()
            }
            .into();
            arr_global.set_initializer(&arr);

            let val = arr_ty.const_named_struct(&[
                arr_global
                    .as_pointer_value()
                    .const_cast(ty.ptr_type(AddressSpace::default()))
                    .into(),
                size_t.const_int(len as u64, false).into(),
            ]);

            let global = ctx.module.add_global(arr_ty, Some(AddressSpace::default()), &id_str);
            global.set_initializer(&val);

            Ok(Some(global.as_pointer_value().into()))
        } else if ty_id == self.primitive_ids.ndarray {
            let id_str = id.to_string();

            if let Some(global) = ctx.module.get_global(&id_str) {
                return Ok(Some(global.as_pointer_value().into()));
            }

            let ndarray_ty = if matches!(&*ctx.unifier.get_ty_immutable(expected_ty), TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id())
            {
                expected_ty
            } else {
                unreachable!("must be ndarray")
            };
            let (ndarray_dtype, _) = unpack_ndarray_var_tys(&mut ctx.unifier, ndarray_ty);

            let llvm_i8 = ctx.ctx.i8_type();
            let llvm_pi8 = llvm_i8.ptr_type(AddressSpace::default());
            let llvm_usize = generator.get_size_type(ctx.ctx);
            let llvm_ndarray = NDArrayType::from_unifier_type(generator, ctx, ndarray_ty);
            let dtype = llvm_ndarray.element_type();

            {
                if self.global_value_ids.read().contains_key(&id) {
                    let global = ctx.module.get_global(&id_str).unwrap_or_else(|| {
                        ctx.module.add_global(
                            llvm_ndarray.as_base_type().get_element_type().into_struct_type(),
                            Some(AddressSpace::default()),
                            &id_str,
                        )
                    });
                    return Ok(Some(global.as_pointer_value().into()));
                }
                self.global_value_ids.write().insert(id, obj.into());
            }

            let ndims = llvm_ndarray.ndims();

            // Obtain the shape of the ndarray
            let shape_tuple: &PyTuple = obj.getattr("shape")?.downcast()?;
            assert_eq!(shape_tuple.len(), ndims as usize);

            // The Rust type inferencer cannot figure this out
            let shape_values = shape_tuple
                .iter()
                .enumerate()
                .map(|(i, elem)| {
                    let value = self
                        .get_obj_value(py, elem, ctx, generator, ctx.primitives.usize())
                        .map_err(|e| {
                            super::CompileError::new_err(format!("Error getting element {i}: {e}"))
                        })?
                        .unwrap();
                    let value = ctx
                        .builder
                        .build_int_z_extend(value.into_int_value(), llvm_usize, "")
                        .unwrap();
                    Ok(value)
                })
                .collect::<Result<Vec<_>, PyErr>>()?;

            // Also use this opportunity to get the constant values of `shape_values` for calculating strides.
            let shape_u64s = shape_values
                .iter()
                .map(|dim| {
                    assert!(dim.is_const());
                    dim.get_zero_extended_constant().unwrap()
                })
                .collect_vec();
            let shape_values = llvm_usize.const_array(&shape_values);

            // create a global for ndarray.shape and initialize it using the shape
            let shape_global = ctx.module.add_global(
                llvm_usize.array_type(ndims as u32),
                Some(AddressSpace::default()),
                &(id_str.clone() + ".shape"),
            );
            shape_global.set_initializer(&shape_values);

            // Obtain the (flattened) elements of the ndarray
            let sz: usize = obj.getattr("size")?.extract()?;
            let data: Vec<_> = (0..sz)
                .map(|i| {
                    obj.getattr("flat")?.get_item(i).and_then(|elem| {
                        let value = self
                            .get_obj_value(py, elem, ctx, generator, ndarray_dtype)
                            .map_err(|e| {
                                super::CompileError::new_err(format!(
                                    "Error getting element {i}: {e}"
                                ))
                            })?
                            .unwrap();

                        assert_eq!(value.get_type(), dtype);
                        Ok(value)
                    })
                })
                .try_collect()?;
            let data = data.into_iter();
            let data = match dtype {
                BasicTypeEnum::ArrayType(ty) => {
                    ty.const_array(&data.map(BasicValueEnum::into_array_value).collect_vec())
                }

                BasicTypeEnum::FloatType(ty) => {
                    ty.const_array(&data.map(BasicValueEnum::into_float_value).collect_vec())
                }

                BasicTypeEnum::IntType(ty) => {
                    ty.const_array(&data.map(BasicValueEnum::into_int_value).collect_vec())
                }

                BasicTypeEnum::PointerType(ty) => {
                    ty.const_array(&data.map(BasicValueEnum::into_pointer_value).collect_vec())
                }

                BasicTypeEnum::StructType(ty) => {
                    ty.const_array(&data.map(BasicValueEnum::into_struct_value).collect_vec())
                }

                BasicTypeEnum::VectorType(_) => unreachable!(),
            };

            // create a global for ndarray.data and initialize it using the elements
            //
            // NOTE: NDArray's `data` is `u8*`. Here, `data_global` is an array of `dtype`.
            // We will have to cast it to an `u8*` later.
            let data_global = ctx.module.add_global(
                dtype.array_type(sz as u32),
                Some(AddressSpace::default()),
                &(id_str.clone() + ".data"),
            );
            data_global.set_initializer(&data);

            // Get the constant itemsize.
            //
            // NOTE: dtype.size_of() may return a non-constant, where `TargetData::get_store_size`
            // will always return a constant size.
            let itemsize = ctx
                .registry
                .llvm_options
                .create_target_machine()
                .map(|tm| tm.get_target_data().get_store_size(&dtype))
                .unwrap();
            assert_ne!(itemsize, 0);

            // Create the strides needed for ndarray.strides
            let strides = make_contiguous_strides(itemsize, ndims, &shape_u64s);
            let strides =
                strides.into_iter().map(|stride| llvm_usize.const_int(stride, false)).collect_vec();
            let strides = llvm_usize.const_array(&strides);

            // create a global for ndarray.strides and initialize it
            let strides_global = ctx.module.add_global(
                llvm_usize.array_type(ndims as u32),
                Some(AddressSpace::default()),
                &format!("${id_str}.strides"),
            );
            strides_global.set_initializer(&strides);

            // create a global for the ndarray object and initialize it

            // NOTE: data_global is an array of dtype, we want a `u8*`.
            let ndarray_data = data_global.as_pointer_value();
            let ndarray_data = ctx.builder.build_pointer_cast(ndarray_data, llvm_pi8, "").unwrap();

            let ndarray_itemsize = llvm_usize.const_int(itemsize, false);

            let ndarray_ndims = llvm_usize.const_int(ndims, false);

            // calling as_pointer_value on shape and strides returns [i64 x ndims]*
            // convert into i64* to conform with expected layout of ndarray

            let ndarray_shape = shape_global.as_pointer_value();
            let ndarray_shape = unsafe {
                ctx.builder
                    .build_in_bounds_gep(
                        ndarray_shape,
                        &[llvm_usize.const_zero(), llvm_usize.const_zero()],
                        "",
                    )
                    .unwrap()
            };

            let ndarray_strides = strides_global.as_pointer_value();
            let ndarray_strides = unsafe {
                ctx.builder
                    .build_in_bounds_gep(
                        ndarray_strides,
                        &[llvm_usize.const_zero(), llvm_usize.const_zero()],
                        "",
                    )
                    .unwrap()
            };

            let ndarray = llvm_ndarray
                .as_base_type()
                .get_element_type()
                .into_struct_type()
                .const_named_struct(&[
                    ndarray_itemsize.into(),
                    ndarray_ndims.into(),
                    ndarray_shape.into(),
                    ndarray_strides.into(),
                    ndarray_data.into(),
                ]);

            let ndarray_global = ctx.module.add_global(
                llvm_ndarray.as_base_type().get_element_type().into_struct_type(),
                Some(AddressSpace::default()),
                &id_str,
            );
            ndarray_global.set_initializer(&ndarray);

            Ok(Some(ndarray_global.as_pointer_value().into()))
        } else if ty_id == self.primitive_ids.tuple {
            let expected_ty_enum = ctx.unifier.get_ty_immutable(expected_ty);
            let TypeEnum::TTuple { ty, is_vararg_ctx: false } = expected_ty_enum.as_ref() else {
                unreachable!()
            };

            let tup_tys = ty.iter();
            let elements: &PyTuple = obj.downcast()?;
            assert_eq!(elements.len(), tup_tys.len());
            let val: Result<Option<Vec<_>>, _> = elements
                .iter()
                .enumerate()
                .zip(tup_tys)
                .map(|((i, elem), ty)| {
                    self.get_obj_value(py, elem, ctx, generator, *ty).map_err(|e| {
                        super::CompileError::new_err(format!("Error getting element {i}: {e}"))
                    })
                })
                .collect();
            let val = val?.unwrap();
            let val = ctx.ctx.const_struct(&val, false);
            Ok(Some(val.into()))
        } else if ty_id == self.primitive_ids.option {
            let option_val_ty = match ctx.unifier.get_ty_immutable(expected_ty).as_ref() {
                TypeEnum::TObj { obj_id, params, .. }
                    if *obj_id == ctx.primitives.option.obj_id(&ctx.unifier).unwrap() =>
                {
                    *params.iter().next().unwrap().1
                }
                _ => unreachable!("must be option type"),
            };
            if id == self.primitive_ids.none {
                // for option type, just a null ptr
                Ok(Some(
                    ctx.get_llvm_type(generator, option_val_ty)
                        .ptr_type(AddressSpace::default())
                        .const_null()
                        .into(),
                ))
            } else {
                match self
                    .get_obj_value(
                        py,
                        obj.getattr("_nac3_option").unwrap(),
                        ctx,
                        generator,
                        option_val_ty,
                    )
                    .map_err(|e| {
                        super::CompileError::new_err(format!(
                            "Error getting value of Option object: {e}"
                        ))
                    })? {
                    Some(v) => {
                        let global_str = format!("{id}_option");
                        {
                            if self.global_value_ids.read().contains_key(&id) {
                                let global =
                                    ctx.module.get_global(&global_str).unwrap_or_else(|| {
                                        ctx.module.add_global(
                                            v.get_type(),
                                            Some(AddressSpace::default()),
                                            &global_str,
                                        )
                                    });
                                return Ok(Some(global.as_pointer_value().into()));
                            }
                            self.global_value_ids.write().insert(id, obj.into());
                        }
                        let global = ctx.module.add_global(
                            v.get_type(),
                            Some(AddressSpace::default()),
                            &global_str,
                        );
                        global.set_initializer(&v);
                        Ok(Some(global.as_pointer_value().into()))
                    }
                    None => Ok(None),
                }
            }
        } else {
            let id_str = id.to_string();

            if let Some(global) = ctx.module.get_global(&id_str) {
                return Ok(Some(global.as_pointer_value().into()));
            }

            let top_level_defs = ctx.top_level.definitions.read();
            let ty = self
                .get_obj_type(py, obj, &mut ctx.unifier, &top_level_defs, &ctx.primitives)?
                .unwrap();
            let ty = ctx
                .get_llvm_type(generator, ty)
                .into_pointer_type()
                .get_element_type()
                .into_struct_type();
            {
                if self.global_value_ids.read().contains_key(&id) {
                    let global = ctx.module.get_global(&id_str).unwrap_or_else(|| {
                        ctx.module.add_global(ty, Some(AddressSpace::default()), &id_str)
                    });
                    return Ok(Some(global.as_pointer_value().into()));
                }
                self.global_value_ids.write().insert(id, obj.into());
            }
            // should be classes
            let definition =
                top_level_defs.get(self.pyid_to_def.read().get(&ty_id).unwrap().0).unwrap().read();
            let TopLevelDef::Class { fields, .. } = &*definition else { unreachable!() };

            let values: Result<Option<Vec<_>>, _> = fields
                .iter()
                .map(|(name, ty, _)| {
                    self.get_obj_value(
                        py,
                        obj.getattr(name.to_string().as_str())?,
                        ctx,
                        generator,
                        *ty,
                    )
                    .map_err(|e| {
                        super::CompileError::new_err(format!("Error getting field {name}: {e}"))
                    })
                })
                .collect();
            let values = values?;
            if let Some(values) = values {
                let val = ty.const_named_struct(&values);
                let global = ctx.module.get_global(&id_str).unwrap_or_else(|| {
                    ctx.module.add_global(ty, Some(AddressSpace::default()), &id_str)
                });
                global.set_initializer(&val);
                Ok(Some(global.as_pointer_value().into()))
            } else {
                Ok(None)
            }
        }
    }

    fn get_default_param_obj_value(
        &self,
        py: Python,
        obj: &PyAny,
    ) -> PyResult<Result<SymbolValue, String>> {
        let id: u64 = self.helper.id_fn.call1(py, (obj,))?.extract(py)?;
        let ty_id: u64 =
            self.helper.id_fn.call1(py, (self.helper.type_fn.call1(py, (obj,))?,))?.extract(py)?;
        Ok(if ty_id == self.primitive_ids.int || ty_id == self.primitive_ids.int32 {
            let val: i32 = obj.extract()?;
            Ok(SymbolValue::I32(val))
        } else if ty_id == self.primitive_ids.int64 {
            let val: i64 = obj.extract()?;
            Ok(SymbolValue::I64(val))
        } else if ty_id == self.primitive_ids.uint32 {
            let val: u32 = obj.extract()?;
            Ok(SymbolValue::U32(val))
        } else if ty_id == self.primitive_ids.uint64 {
            let val: u64 = obj.extract()?;
            Ok(SymbolValue::U64(val))
        } else if ty_id == self.primitive_ids.bool {
            let val: bool = obj.extract()?;
            Ok(SymbolValue::Bool(val))
        } else if ty_id == self.primitive_ids.np_bool_ {
            let val: bool = obj.call_method("__bool__", (), None)?.extract()?;
            Ok(SymbolValue::Bool(val))
        } else if ty_id == self.primitive_ids.string || ty_id == self.primitive_ids.np_str_ {
            let val: String = obj.extract()?;
            Ok(SymbolValue::Str(val))
        } else if ty_id == self.primitive_ids.float || ty_id == self.primitive_ids.float64 {
            let val: f64 = obj.extract()?;
            Ok(SymbolValue::Double(val))
        } else if ty_id == self.primitive_ids.tuple {
            let elements: &PyTuple = obj.downcast()?;
            let elements: Result<Result<Vec<_>, String>, _> =
                elements.iter().map(|elem| self.get_default_param_obj_value(py, elem)).collect();
            elements?.map(SymbolValue::Tuple)
        } else if ty_id == self.primitive_ids.option {
            if id == self.primitive_ids.none {
                Ok(SymbolValue::OptionNone)
            } else {
                self.get_default_param_obj_value(py, obj.getattr("_nac3_option").unwrap())?
                    .map(|v| SymbolValue::OptionSome(Box::new(v)))
            }
        } else {
            Err("only primitives values, option and tuple can be default parameter value".into())
        })
    }
}

impl SymbolResolver for Resolver {
    fn get_default_param_value(&self, expr: &ast::Expr) -> Option<SymbolValue> {
        let ast::ExprKind::Name { id, .. } = &expr.node else {
            unreachable!("only for resolving names")
        };

        Python::with_gil(|py| -> PyResult<Option<SymbolValue>> {
            let obj: &PyAny = self.0.module.extract(py)?;
            let members: &PyDict = obj.getattr("__dict__").unwrap().downcast().unwrap();
            let mut sym_value = None;
            for (key, val) in members {
                let key: &str = key.extract()?;
                if key == id.to_string() {
                    if let Ok(Ok(v)) = self.0.get_default_param_obj_value(py, val) {
                        sym_value = Some(v);
                    }
                    break;
                }
            }
            Ok(sym_value)
        })
        .unwrap()
    }

    fn get_symbol_type(
        &self,
        unifier: &mut Unifier,
        defs: &[Arc<RwLock<TopLevelDef>>],
        primitives: &PrimitiveStore,
        str: StrRef,
    ) -> Result<Type, String> {
        if let Some(ty) = {
            let id_to_type = self.0.id_to_type.read();
            id_to_type.get(&str).copied()
        } {
            Ok(ty)
        } else {
            let Some(id) = self.0.name_to_pyid.get(&str) else {
                return Err(format!("cannot find symbol `{str}`"));
            };
            let result = if let Some(t) = {
                let pyid_to_type = self.0.pyid_to_type.read();
                pyid_to_type.get(id).copied()
            } {
                Ok(t)
            } else {
                Python::with_gil(|py| -> PyResult<Result<Type, String>> {
                    let obj: &PyAny = self.0.module.extract(py)?;
                    let mut sym_ty = Err(format!("cannot find symbol `{str}`"));
                    let members: &PyDict = obj.getattr("__dict__").unwrap().downcast().unwrap();
                    for (key, val) in members {
                        let key: &str = key.extract()?;
                        if key == str.to_string() {
                            sym_ty = self.0.get_obj_type(py, val, unifier, defs, primitives)?;
                            break;
                        }
                    }
                    if let Ok(t) = sym_ty {
                        if let TypeEnum::TVar { .. } = &*unifier.get_ty(t) {
                            self.0.pyid_to_type.write().insert(*id, t);
                        }
                    }
                    Ok(sym_ty)
                })
                .unwrap()
            };
            result
        }
    }

    fn get_symbol_value<'ctx>(
        &self,
        id: StrRef,
        _: &mut CodeGenContext<'ctx, '_>,
        _: &mut dyn CodeGenerator,
    ) -> Option<ValueEnum<'ctx>> {
        let sym_value = {
            let id_to_val = self.0.id_to_pyval.read();
            id_to_val.get(&id).cloned()
        }
        .or_else(|| {
            Python::with_gil(|py| -> PyResult<Option<(u64, PyObject)>> {
                let obj: &PyAny = self.0.module.extract(py)?;
                let mut sym_value: Option<(u64, PyObject)> = None;
                let members: &PyDict = obj.getattr("__dict__").unwrap().downcast().unwrap();
                for (key, val) in members {
                    let key: &str = key.extract()?;
                    if key == id.to_string() {
                        let id = self.0.helper.id_fn.call1(py, (val,))?.extract(py)?;
                        sym_value = Some((id, val.extract()?));
                        break;
                    }
                }
                if let Some((pyid, val)) = &sym_value {
                    self.0.id_to_pyval.write().insert(id, (*pyid, val.clone()));
                }
                Ok(sym_value)
            })
            .unwrap()
        });
        sym_value.map(|(id, v)| {
            ValueEnum::Static(Arc::new(PythonValue {
                id,
                value: v,
                store_obj: self.0.helper.store_obj.clone(),
                resolver: self.0.clone(),
            }))
        })
    }

    fn get_identifier_def(&self, id: StrRef) -> Result<DefinitionId, HashSet<String>> {
        {
            let id_to_def = self.0.id_to_def.read();
            id_to_def.get(&id).copied().ok_or_else(String::new)
        }
        .or_else(|_| {
            let py_id = self
                .0
                .name_to_pyid
                .get(&id)
                .ok_or_else(|| HashSet::from([format!("Undefined identifier `{id}`")]))?;
            let result = self.0.pyid_to_def.read().get(py_id).copied().ok_or_else(|| {
                HashSet::from([format!(
                    "`{id}` is not registered with NAC3 (@nac3 decorator missing?)"
                )])
            })?;
            self.0.id_to_def.write().insert(id, result);
            Ok(result)
        })
    }

    fn get_string_id(&self, s: &str) -> i32 {
        let mut string_store = self.0.string_store.write();
        if let Some(id) = string_store.get(s) {
            *id
        } else {
            let id = i32::try_from(string_store.len()).unwrap();
            string_store.insert(s.into(), id);
            id
        }
    }

    fn handle_deferred_eval(
        &self,
        unifier: &mut Unifier,
        defs: &[Arc<RwLock<TopLevelDef>>],
        primitives: &PrimitiveStore,
    ) -> Result<(), String> {
        // we don't need a lock because this will only be run in a single thread
        if self.0.deferred_eval_store.needs_defer.load(Relaxed) {
            self.0.deferred_eval_store.needs_defer.store(false, Relaxed);
            let store = self.0.deferred_eval_store.store.read();
            Python::with_gil(|py| -> PyResult<Result<(), String>> {
                for (variables, constraints, name) in store.iter() {
                    let constraints: &PyAny = constraints.as_ref(py);
                    for (i, var) in variables.iter().enumerate() {
                        if let Ok(constr) = constraints.get_item(i) {
                            match self.0.get_pyty_obj_type(py, constr, unifier, defs, primitives)? {
                                Ok((ty, _)) => {
                                    if !unifier.is_concrete(ty, &[]) {
                                        return Ok(Err(format!(
                                            "the {}th constraint of TypeVar `{}` is not concrete",
                                            i + 1,
                                            name,
                                        )));
                                    }
                                    unifier.unify(ty, *var).unwrap();
                                }
                                Err(err) => return Ok(Err(err)),
                            }
                        } else {
                            break;
                        }
                    }
                }
                Ok(Ok(()))
            })
            .unwrap()?;
        }
        Ok(())
    }

    fn get_exception_id(&self, tyid: usize) -> usize {
        let exn_ids = self.0.exception_ids.read();
        exn_ids.get(&tyid).copied().unwrap_or(0)
    }
}
