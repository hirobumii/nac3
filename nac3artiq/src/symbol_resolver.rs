use inkwell::{types::BasicType, values::BasicValueEnum, AddressSpace};
use nac3core::{
    codegen::CodeGenContext,
    location::Location,
    symbol_resolver::SymbolResolver,
    toplevel::{DefinitionId, TopLevelDef},
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{Type, TypeEnum, Unifier},
    },
};
use parking_lot::{Mutex, RwLock};
use pyo3::{
    types::{PyList, PyModule, PyTuple},
    PyAny, PyObject, PyResult, Python,
};
use nac3parser::ast::StrRef;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::PrimitivePythonId;

pub struct Resolver {
    pub id_to_type: Mutex<HashMap<StrRef, Type>>,
    pub id_to_def: Mutex<HashMap<StrRef, DefinitionId>>,
    pub global_value_ids: Arc<Mutex<HashSet<u64>>>,
    pub class_names: Mutex<HashMap<StrRef, Type>>,
    pub pyid_to_def: Arc<RwLock<HashMap<u64, DefinitionId>>>,
    pub pyid_to_type: Arc<RwLock<HashMap<u64, Type>>>,
    pub primitive_ids: PrimitivePythonId,
    // module specific
    pub name_to_pyid: HashMap<StrRef, u64>,
    pub module: PyObject,
}

struct PythonHelper<'a> {
    type_fn: &'a PyAny,
    len_fn: &'a PyAny,
    id_fn: &'a PyAny,
    eval_type_fn: &'a PyAny,
    origin_ty_fn: &'a PyAny,
    args_ty_fn: &'a PyAny,
    globals_dict: &'a PyAny,
    print_fn: &'a PyAny,
}

impl Resolver {
    fn get_list_elem_type(
        &self,
        list: &PyAny,
        len: usize,
        helper: &PythonHelper,
        unifier: &mut Unifier,
        defs: &[Arc<RwLock<TopLevelDef>>],
        primitives: &PrimitiveStore,
    ) -> PyResult<Option<Type>> {
        let first = self.get_obj_type(list.get_item(0)?, helper, unifier, defs, primitives)?;
        Ok((1..len).fold(first, |a, i| {
            let b = list
                .get_item(i)
                .map(|elem| self.get_obj_type(elem, helper, unifier, defs, primitives));
            a.and_then(|a| {
                if let Ok(Ok(Some(ty))) = b {
                    if unifier.unify(a, ty).is_ok() {
                        Some(a)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
        }))
    }

    // handle python objects that represent types themselves
    // primitives and class types should be themselves, use `ty_id` to check,
    // TypeVars and GenericAlias(`A[int, bool]`) should use `ty_ty_id` to check
    // the `bool` value returned indicates whether they are instantiated or not
    fn get_pyty_obj_type(
        &self,
        pyty: &PyAny,
        helper: &PythonHelper,
        unifier: &mut Unifier,
        defs: &[Arc<RwLock<TopLevelDef>>],
        primitives: &PrimitiveStore,
    ) -> PyResult<Result<(Type, bool), String>> {
        // eval_type use only globals_dict should be fine
        let evaluated_ty = helper
            .eval_type_fn
            .call1((pyty, helper.globals_dict, helper.globals_dict)).unwrap();
        let ty_id: u64 = helper
            .id_fn
            .call1((evaluated_ty,))?
            .extract()?;
        let ty_ty_id: u64 = helper
            .id_fn
            .call1((helper.type_fn.call1((evaluated_ty,))?,))?
            .extract()?;
        
        if ty_id == self.primitive_ids.int || ty_id == self.primitive_ids.int32 {
            Ok(Ok((primitives.int32, true)))
        } else if ty_id == self.primitive_ids.int64 {
            Ok(Ok((primitives.int64, true)))
        } else if ty_id == self.primitive_ids.bool {
            Ok(Ok((primitives.bool, true)))
        } else if ty_id == self.primitive_ids.float {
            Ok(Ok((primitives.float, true)))
        } else if ty_id == self.primitive_ids.list {
            // do not handle type var param and concrete check here
            let var = unifier.get_fresh_var().0;
            let list = unifier.add_ty(TypeEnum::TList { ty: var });
            Ok(Ok((list, false)))
        } else if ty_id == self.primitive_ids.tuple {
            // do not handle type var param and concrete check here
            Ok(Ok((unifier.add_ty(TypeEnum::TTuple { ty: vec![] }), false)))
        } else if let Some(def_id) = self.pyid_to_def.read().get(&ty_id).cloned() {
            // println!("getting def");
            let def = defs[def_id.0].read();
            // println!("got def");
            if let TopLevelDef::Class {
                object_id,
                type_vars,
                fields,
                methods,
                ..
            } = &*def
            {
                // do not handle type var param and concrete check here, and no subst
                Ok(Ok({
                    let ty = TypeEnum::TObj {
                        obj_id: *object_id,
                        params: RefCell::new({
                            type_vars
                                .iter()
                                .map(|x| {
                                    if let TypeEnum::TVar { id, .. } = &*unifier.get_ty(*x) {
                                        (*id, *x)
                                    } else { unreachable!() }
                                }).collect()
                        }),
                        fields: RefCell::new({
                            let mut res = methods
                                .iter()
                                .map(|(iden, ty, _)| (*iden, (*ty, false)))
                                .collect::<HashMap<_, _>>();
                            res.extend(fields.clone().into_iter().map(|x| (x.0, (x.1, x.2))));
                            res
                        })
                    };
                    // here also false, later insta use python object to check compatible
                    (unifier.add_ty(ty), false)
                }))
            } else {
                // only object is supported, functions are not supported
                unreachable!("function type is not supported, should not be queried")
            }
        } else if ty_ty_id == self.primitive_ids.typevar {
            let constraint_types = {
                let constraints = pyty.getattr("__constraints__").unwrap();
                let mut result: Vec<Type> = vec![];
                for i in 0.. {
                    if let Ok(constr) = constraints.get_item(i) {
                        result.push({
                            match self.get_pyty_obj_type(constr, helper, unifier, defs, primitives)? {
                                Ok((ty, _)) => {
                                    if unifier.is_concrete(ty, &[]) {
                                        ty
                                    } else {
                                        return Ok(Err(format!(
                                            "the {}th constraint of TypeVar `{}` is not concrete",
                                            i + 1,
                                            pyty.getattr("__name__")?.extract::<String>()?
                                        )))
                                    }
                                },
                                Err(err) => return Ok(Err(err))
                            }
                        })
                    } else {
                        break;
                    }
                }
                result
            };
            let res = unifier.get_fresh_var_with_range(&constraint_types).0;
            Ok(Ok((res, true)))
        } else if ty_ty_id == self.primitive_ids.generic_alias.0 || ty_ty_id == self.primitive_ids.generic_alias.1 {
            let origin = helper.origin_ty_fn.call1((evaluated_ty,))?;
            let args: &PyTuple = helper.args_ty_fn.call1((evaluated_ty,))?.cast_as()?;
            let origin_ty = match self.get_pyty_obj_type(origin, helper, unifier, defs, primitives)? {
                Ok((ty, false)) => ty,
                Ok((_, true)) => return Ok(Err("instantiated type does not take type parameters".into())),
                Err(err) => return Ok(Err(err))
            };

            match &*unifier.get_ty(origin_ty) {
                TypeEnum::TList { .. } => {
                    if args.len() == 1 {
                        let ty = match self.get_pyty_obj_type(args.get_item(0), helper, unifier, defs, primitives)? {
                            Ok(ty) => ty,
                            Err(err) => return Ok(Err(err))
                        };
                        if !unifier.is_concrete(ty.0, &[]) && !ty.1 {
                            panic!("type list should take concrete parameters in type var ranges")
                        }
                        Ok(Ok((unifier.add_ty(TypeEnum::TList { ty: ty.0 }), true)))
                    } else {
                        return Ok(Err(format!("type list needs exactly 1 type parameters, found {}", args.len())))
                    }
                 },
                TypeEnum::TTuple { .. } => {
                    let args = match args
                        .iter()
                        .map(|x| self.get_pyty_obj_type(x, helper, unifier, defs, primitives))
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
                    Ok(Ok((unifier.add_ty(TypeEnum::TTuple { ty: args }), true)))
                },
                TypeEnum::TObj { params, obj_id, .. } => {
                    let subst = {
                        let params = &*params.borrow();
                        if params.len() != args.len() {
                            return Ok(Err(format!(
                                "for class #{}, expect {} type parameters, got {}.",
                                obj_id.0,
                                params.len(),
                                args.len(),
                            )))
                        }
                        let args = match args
                            .iter()
                            .map(|x| self.get_pyty_obj_type(x, helper, unifier, defs, primitives))
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
                            .collect::<HashMap<_, _>>()
                    };
                    Ok(Ok((unifier.subst(origin_ty, &subst).unwrap_or(origin_ty), true)))
                },
                TypeEnum::TVirtual { .. } => {
                    if args.len() == 1 {
                        let ty = match self.get_pyty_obj_type(args.get_item(0), helper, unifier, defs, primitives)? {
                            Ok(ty) => ty,
                            Err(err) => return Ok(Err(err))
                        };
                        if !unifier.is_concrete(ty.0, &[]) && !ty.1 {
                            panic!("virtual class should take concrete parameters in type var ranges")
                        }
                        Ok(Ok((unifier.add_ty(TypeEnum::TVirtual { ty: ty.0 }), true)))
                    } else {
                        return Ok(Err(format!("virtual class needs exactly 1 type parameters, found {}", args.len())))
                    }
                }
                _ => unimplemented!()
            }
        } else if ty_id == self.primitive_ids.virtual_id {
            Ok(Ok(({
                let ty = TypeEnum::TVirtual { ty: unifier.get_fresh_var().0 };
                unifier.add_ty(ty)
            }, false)))
        } else {
            Ok(Err("unknown type".into()))
        }
    }

    fn get_obj_type(
        &self,
        obj: &PyAny,
        helper: &PythonHelper,
        unifier: &mut Unifier,
        defs: &[Arc<RwLock<TopLevelDef>>],
        primitives: &PrimitiveStore,
    ) -> PyResult<Option<Type>> {
        let (extracted_ty, inst_check) = match self.get_pyty_obj_type(
            {
                let ty = helper.type_fn.call1((obj,)).unwrap();
                if [self.primitive_ids.typevar,
                    self.primitive_ids.generic_alias.0,
                    self.primitive_ids.generic_alias.1
                ].contains(&helper.id_fn.call1((ty,))?.extract::<u64>()?) {
                    obj
                } else {
                    ty
                }
            },
            helper,
            unifier,
            defs,
            primitives
        )? {
            Ok(s) => s,
            Err(_) => return Ok(None)
        };
        return match (&*unifier.get_ty(extracted_ty), inst_check) {
            // do the instantiation for these three types
            (TypeEnum::TList { ty }, false) => {
                let len: usize = helper.len_fn.call1((obj,))?.extract()?;
                if len == 0 {
                    assert!(matches!(
                        &*unifier.get_ty(extracted_ty),
                        TypeEnum::TVar { meta: nac3core::typecheck::typedef::TypeVarMeta::Generic, range, .. }
                            if range.borrow().is_empty()
                    ));
                    Ok(Some(extracted_ty))
                } else {
                    let actual_ty = self
                        .get_list_elem_type(obj, len, helper, unifier, defs, primitives)?;
                    if let Some(actual_ty) = actual_ty {
                        unifier.unify(*ty, actual_ty).unwrap();
                        Ok(Some(extracted_ty))
                    } else {
                        Ok(None)
                    }
                }
            }
            (TypeEnum::TTuple { .. }, false) => {
                let elements: &PyTuple = obj.cast_as()?;
                let types: Result<Option<Vec<_>>, _> = elements
                    .iter()
                    .map(|elem| self.get_obj_type(elem, helper, unifier, defs, primitives))
                    .collect();
                let types = types?;
                Ok(types.map(|types| unifier.add_ty(TypeEnum::TTuple { ty: types })))
            }
            (TypeEnum::TObj { params, fields, .. }, false) => {
                let var_map = params
                    .borrow()
                    .iter()
                    .map(|(id_var, ty)| {
                        if let TypeEnum::TVar { id, range, .. } = &*unifier.get_ty(*ty) {
                            assert_eq!(*id, *id_var);
                            (*id, unifier.get_fresh_var_with_range(&range.borrow()).0)
                        } else {
                            unreachable!()
                        }
                    })
                    .collect::<HashMap<_, _>>();
                // loop through non-function fields of the class to get the instantiated value
                for field in fields.borrow().iter() {
                    let name: String = (*field.0).into();
                    if let TypeEnum::TFunc( .. ) = &*unifier.get_ty(field.1.0) {
                        continue;
                    } else {
                        let field_data = obj.getattr(&name)?;
                        let ty = self
                            .get_obj_type(field_data, helper, unifier, defs, primitives)?
                            .unwrap_or(primitives.none);
                        let field_ty = unifier.subst(field.1.0, &var_map).unwrap_or(field.1.0);
                        if unifier.unify(ty, field_ty).is_err() {
                            // field type mismatch
                            return Ok(None);
                        }
                    }
                }
                for (_, ty) in var_map.iter() {
                    // must be concrete type
                    if !unifier.is_concrete(*ty, &[]) {
                        return Ok(None)
                    }
                }
                return Ok(Some(unifier.subst(extracted_ty, &var_map).unwrap_or(extracted_ty)));
            }
            _ => Ok(Some(extracted_ty))
        };
    }

    fn get_obj_value<'ctx, 'a>(
        &self,
        obj: &PyAny,
        helper: &PythonHelper,
        ctx: &mut CodeGenContext<'ctx, 'a>,
    ) -> PyResult<Option<BasicValueEnum<'ctx>>> {
        let ty_id: u64 = helper
            .id_fn
            .call1((helper.type_fn.call1((obj,))?,))?
            .extract()?;
        if ty_id == self.primitive_ids.int || ty_id == self.primitive_ids.int32 {
            let val: i32 = obj.extract()?;
            Ok(Some(ctx.ctx.i32_type().const_int(val as u64, false).into()))
        } else if ty_id == self.primitive_ids.int64 {
            let val: i64 = obj.extract()?;
            Ok(Some(ctx.ctx.i64_type().const_int(val as u64, false).into()))
        } else if ty_id == self.primitive_ids.bool {
            let val: bool = obj.extract()?;
            Ok(Some(
                ctx.ctx.bool_type().const_int(val as u64, false).into(),
            ))
        } else if ty_id == self.primitive_ids.float {
            let val: f64 = obj.extract()?;
            Ok(Some(ctx.ctx.f64_type().const_float(val).into()))
        } else if ty_id == self.primitive_ids.list {
            let id: u64 = helper.id_fn.call1((obj,))?.extract()?;
            let id_str = id.to_string();
            let len: usize = helper.len_fn.call1((obj,))?.extract()?;
            let ty = if len == 0 {
                ctx.primitives.int32
            } else {
                self.get_list_elem_type(
                    obj,
                    len,
                    helper,
                    &mut ctx.unifier,
                    &ctx.top_level.definitions.read(),
                    &ctx.primitives,
                )?
                .unwrap()
            };
            let ty = ctx.get_llvm_type(ty);
            let arr_ty = ctx.ctx.struct_type(
                &[
                    ctx.ctx.i32_type().into(),
                    ty.ptr_type(AddressSpace::Generic).into(),
                ],
                false,
            );

            {
                let mut global_value_ids = self.global_value_ids.lock();
                if global_value_ids.contains(&id) {
                    let global = ctx.module.get_global(&id_str).unwrap_or_else(|| {
                        ctx.module
                            .add_global(arr_ty, Some(AddressSpace::Generic), &id_str)
                    });
                    return Ok(Some(global.as_pointer_value().into()));
                } else {
                    global_value_ids.insert(id);
                }
            }

            let arr: Result<Option<Vec<_>>, _> = (0..len)
                .map(|i| {
                    obj.get_item(i)
                        .and_then(|elem| self.get_obj_value(elem, helper, ctx))
                })
                .collect();
            let arr = arr?.unwrap();

            let arr_global = ctx.module.add_global(
                ty.array_type(len as u32),
                Some(AddressSpace::Generic),
                &(id_str.clone() + "_"),
            );
            let arr: BasicValueEnum = if ty.is_int_type() {
                let arr: Vec<_> = arr
                    .into_iter()
                    .map(BasicValueEnum::into_int_value)
                    .collect();
                ty.into_int_type().const_array(&arr)
            } else if ty.is_float_type() {
                let arr: Vec<_> = arr
                    .into_iter()
                    .map(BasicValueEnum::into_float_value)
                    .collect();
                ty.into_float_type().const_array(&arr)
            } else if ty.is_array_type() {
                let arr: Vec<_> = arr
                    .into_iter()
                    .map(BasicValueEnum::into_array_value)
                    .collect();
                ty.into_array_type().const_array(&arr)
            } else if ty.is_struct_type() {
                let arr: Vec<_> = arr
                    .into_iter()
                    .map(BasicValueEnum::into_struct_value)
                    .collect();
                ty.into_struct_type().const_array(&arr)
            } else if ty.is_pointer_type() {
                let arr: Vec<_> = arr
                    .into_iter()
                    .map(BasicValueEnum::into_pointer_value)
                    .collect();
                ty.into_pointer_type().const_array(&arr)
            } else {
                unreachable!()
            }
            .into();
            arr_global.set_initializer(&arr);

            let val = arr_ty.const_named_struct(&[
                ctx.ctx.i32_type().const_int(len as u64, false).into(),
                arr_global
                    .as_pointer_value()
                    .const_cast(ty.ptr_type(AddressSpace::Generic))
                    .into(),
            ]);

            let global = ctx
                .module
                .add_global(arr_ty, Some(AddressSpace::Generic), &id_str);
            global.set_initializer(&val);

            Ok(Some(global.as_pointer_value().into()))
        } else if ty_id == self.primitive_ids.tuple {
            let id: u64 = helper.id_fn.call1((obj,))?.extract()?;
            let id_str = id.to_string();
            let elements: &PyTuple = obj.cast_as()?;
            let types: Result<Option<Vec<_>>, _> = elements
                .iter()
                .map(|elem| {
                    self.get_obj_type(
                        elem,
                        helper,
                        &mut ctx.unifier,
                        &ctx.top_level.definitions.read(),
                        &ctx.primitives,
                    )
                    .map(|ty| ty.map(|ty| ctx.get_llvm_type(ty)))
                })
                .collect();
            let types = types?.unwrap();
            let ty = ctx.ctx.struct_type(&types, false);

            {
                let mut global_value_ids = self.global_value_ids.lock();
                if global_value_ids.contains(&id) {
                    let global = ctx.module.get_global(&id_str).unwrap_or_else(|| {
                        ctx.module
                            .add_global(ty, Some(AddressSpace::Generic), &id_str)
                    });
                    return Ok(Some(global.as_pointer_value().into()));
                } else {
                    global_value_ids.insert(id);
                }
            }

            let val: Result<Option<Vec<_>>, _> = elements
                .iter()
                .map(|elem| self.get_obj_value(elem, helper, ctx))
                .collect();
            let val = val?.unwrap();
            let val = ctx.ctx.const_struct(&val, false);
            let global = ctx
                .module
                .add_global(ty, Some(AddressSpace::Generic), &id_str);
            global.set_initializer(&val);
            Ok(Some(global.as_pointer_value().into()))
        } else {
            let id: u64 = helper.id_fn.call1((obj,))?.extract()?;
            let id_str = id.to_string();
            let top_level_defs = ctx.top_level.definitions.read();
            let ty = self
                .get_obj_type(
                    obj,
                    helper,
                    &mut ctx.unifier,
                    &top_level_defs,
                    &ctx.primitives,
                )?
                .unwrap();
            let ty = ctx
                .get_llvm_type(ty)
                .into_pointer_type()
                .get_element_type()
                .into_struct_type()
                .as_basic_type_enum();
            {
                let mut global_value_ids = self.global_value_ids.lock();
                if global_value_ids.contains(&id) {
                    let global = ctx.module.get_global(&id_str).unwrap_or_else(|| {
                        ctx.module
                            .add_global(ty, Some(AddressSpace::Generic), &id_str)
                    });
                    return Ok(Some(global.as_pointer_value().into()));
                } else {
                    global_value_ids.insert(id);
                }
            }
            // should be classes
            let definition = top_level_defs
                .get(self.pyid_to_def.read().get(&ty_id).unwrap().0)
                .unwrap()
                .read();
            if let TopLevelDef::Class { fields, .. } = &*definition {
                let values: Result<Option<Vec<_>>, _> = fields
                    .iter()
                    .map(|(name, _, _)| {
                        self.get_obj_value(obj.getattr(&name.to_string())?, helper, ctx)
                    })
                    .collect();
                let values = values?;
                if let Some(values) = values {
                    let val = ctx.ctx.const_struct(&values, false);
                    let global = ctx
                        .module
                        .add_global(ty, Some(AddressSpace::Generic), &id_str);
                    global.set_initializer(&val);
                    Ok(Some(global.as_pointer_value().into()))
                } else {
                    Ok(None)
                }
            } else {
                unreachable!()
            }
        }
    }
}

impl SymbolResolver for Resolver {
    fn get_symbol_type(
        &self,
        unifier: &mut Unifier,
        defs: &[Arc<RwLock<TopLevelDef>>],
        primitives: &PrimitiveStore,
        str: StrRef,
    ) -> Option<Type> {
        let mut id_to_type = self.id_to_type.lock();
        id_to_type.get(&str).cloned().or_else(|| {
            let py_id = self.name_to_pyid.get(&str);
            let result = py_id.and_then(|id| {
                self.pyid_to_type.read().get(id).copied().or_else(|| {
                    Python::with_gil(|py| -> PyResult<Option<Type>> {
                        let obj: &PyAny = self.module.extract(py)?;
                        let members: &PyList = PyModule::import(py, "inspect")?
                            .getattr("getmembers")?
                            .call1((obj,))?
                            .cast_as()?;
                        let mut sym_ty = None;
                        for member in members.iter() {
                            let key: &str = member.get_item(0)?.extract()?;
                            if key == str.to_string() {
                                let builtins = PyModule::import(py, "builtins")?;
                                let typings = PyModule::import(py, "typing")?;
                                let helper = PythonHelper {
                                    id_fn: builtins.getattr("id").unwrap(),
                                    len_fn: builtins.getattr("len").unwrap(),
                                    type_fn: builtins.getattr("type").unwrap(),
                                    origin_ty_fn: typings.getattr("get_origin").unwrap(),
                                    args_ty_fn: typings.getattr("get_args").unwrap(),
                                    globals_dict: obj.getattr("__dict__").unwrap(),
                                    eval_type_fn: typings.getattr("_eval_type").unwrap(),
                                    print_fn: builtins.getattr("print").unwrap(),
                                };
                                sym_ty = self.get_obj_type(
                                    member.get_item(1)?,
                                    &helper,
                                    unifier,
                                    defs,
                                    primitives,
                                )?;
                                break;
                            }
                        }
                        Ok(sym_ty)
                    })
                    .unwrap()
                })
            });
            if let Some(result) = &result {
                id_to_type.insert(str, *result);
            }
            result
        })
    }

    fn get_symbol_value<'ctx, 'a>(
        &self,
        id: StrRef,
        ctx: &mut CodeGenContext<'ctx, 'a>,
    ) -> Option<BasicValueEnum<'ctx>> {
        Python::with_gil(|py| -> PyResult<Option<BasicValueEnum<'ctx>>> {
            let obj: &PyAny = self.module.extract(py)?;
            let members: &PyList = PyModule::import(py, "inspect")?
                .getattr("getmembers")?
                .call1((obj,))?
                .cast_as()?;
            let mut sym_value = None;
            for member in members.iter() {
                let key: &str = member.get_item(0)?.extract()?;
                let val = member.get_item(1)?;
                if key == id.to_string() {
                    let builtins = PyModule::import(py, "builtins")?;
                    let typings = PyModule::import(py, "typing")?;
                    let helper = PythonHelper {
                        id_fn: builtins.getattr("id").unwrap(),
                        len_fn: builtins.getattr("len").unwrap(),
                        type_fn: builtins.getattr("type").unwrap(),
                        origin_ty_fn: typings.getattr("get_origin").unwrap(),
                        args_ty_fn: typings.getattr("get_args").unwrap(),
                        globals_dict: obj.getattr("__dict__").unwrap(),
                        eval_type_fn: typings.getattr("_eval_type").unwrap(),
                        print_fn: builtins.getattr("print").unwrap(),
                    };
                    sym_value = self.get_obj_value(val, &helper, ctx)?;
                    break;
                }
            }
            Ok(sym_value)
        })
        .unwrap()
    }

    fn get_symbol_location(&self, _: StrRef) -> Option<Location> {
        unimplemented!()
    }

    fn get_identifier_def(&self, id: StrRef) -> Option<DefinitionId> {
        let mut id_to_def = self.id_to_def.lock();
        id_to_def.get(&id).cloned().or_else(|| {
            let py_id = self.name_to_pyid.get(&id);
            let result = py_id.and_then(|id| self.pyid_to_def.read().get(id).copied());
            if let Some(result) = &result {
                id_to_def.insert(id, *result);
            }
            result
        })
    }
}
