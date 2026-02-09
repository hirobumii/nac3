use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use itertools::Itertools;
use parking_lot::RwLock;
use strum::IntoEnumIterator;

use nac3parser::ast::{self, Constant, Expr, Location, StrRef};

use super::{
    DefinitionId, TopLevelDef,
    helper::{PrimDef, PrimDefDetails},
};
use crate::{
    symbol_resolver::{SymbolResolver, SymbolValue},
    toplevel::composer::{BuiltinRegistry, erase_expr_type},
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{AttrKind, Type, TypeEnum, Unifier, VarMap},
    },
};

#[derive(Clone, Debug)]
pub enum TypeAnnotation {
    Primitive(Type),
    // we use type vars kind at params to represent self type
    CustomClass {
        id: DefinitionId,
        // params can also be type var
        params: Vec<Self>,
    },
    // can only be CustomClassKind
    Virtual(Box<Self>),
    TypeVar(Type),
    /// A `Literal` allowing a subset of literals.
    Literal(Vec<Constant>),
    Tuple(Vec<Self>),
}

impl TypeAnnotation {
    pub fn stringify(&self, unifier: &mut Unifier) -> String {
        match self {
            Self::Primitive(ty) | Self::TypeVar(ty) => unifier.stringify(*ty),
            Self::CustomClass { id, params } => {
                let class_name = unifier.top_level.as_ref().map_or_else(
                    || format!("class_def_{}", id.0),
                    |top| {
                        if let TopLevelDef::Class { name, .. } =
                            &*top.definitions.read()[id.0].read()
                        {
                            (*name).into()
                        } else {
                            unreachable!()
                        }
                    },
                );
                format!("{}{}", class_name, {
                    let param_list =
                        params.iter().map(|p| p.stringify(unifier)).collect_vec().join(", ");
                    if param_list.is_empty() { String::new() } else { format!("[{param_list}]") }
                })
            }
            Self::Literal(values) => {
                format!("Literal({})", values.iter().map(|v| format!("{v:?}")).join(", "))
            }
            Self::Virtual(ty) => format!("virtual[{}]", ty.stringify(unifier)),
            Self::Tuple(types) => {
                format!(
                    "tuple[{}]",
                    types.iter().map(|p| p.stringify(unifier)).collect_vec().join(", ")
                )
            }
        }
    }
}

/// Converts a [`DefinitionId`] representing a [`TopLevelDef::Class`] and its type arguments into a
/// [`TypeAnnotation`].
#[allow(clippy::too_many_arguments)]
fn class_def_id_to_type_annotation<T, S: std::hash::BuildHasher + Clone>(
    resolver: &(dyn SymbolResolver + Send + Sync),
    top_level_defs: &[Arc<RwLock<TopLevelDef>>],
    builtin_registry: &Arc<dyn BuiltinRegistry>,
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
    mut locked: HashMap<DefinitionId, Vec<Type>, S>,
    id: StrRef,
    (obj_id, type_args): (DefinitionId, Option<&Expr<T>>),
    location: &Location,
) -> Result<TypeAnnotation, HashSet<String>> {
    let Some(top_level_def) = top_level_defs.get(obj_id.0) else {
        return Err(HashSet::from([format!(
            "NameError: name '{id}' is not defined (at {location})",
        )]));
    };

    // We need to use `try_read` here, since the composer may be processing our class right now,
    // which requires exclusive access to modify the class internals.
    //
    // `locked` is guaranteed to hold a k-v pair of the composer-processing class, so fallback
    // to it if the `top_level_def` is already locked for mutation.
    let type_vars = if let Some(def_read) = top_level_def.try_read() {
        if let TopLevelDef::Class { type_vars, .. } = &*def_read {
            type_vars.clone()
        } else {
            return Err(HashSet::from([format!(
                "function cannot be used as a type (at {location})",
            )]));
        }
    } else {
        locked.get(&obj_id).unwrap().clone()
    };

    let param_type_infos = if let Some(slice) = type_args {
        // we do not check whether the application of type variables are compatible here
        let params_ast = if let ast::ExprKind::Tuple { elts, .. } = &slice.node {
            elts.iter().collect_vec()
        } else {
            vec![slice]
        };

        if type_vars.len() != params_ast.len() {
            return Err(HashSet::from([format!(
                "expect {} type parameters but got {} (at {})",
                type_vars.len(),
                params_ast.len(),
                params_ast[0].location,
            )]));
        }

        let result = params_ast
            .iter()
            .map(|x| {
                parse_ast_to_type_annotation_kinds(
                    resolver,
                    top_level_defs,
                    builtin_registry,
                    unifier,
                    primitives,
                    x,
                    {
                        locked.insert(obj_id, type_vars.clone());
                        locked.clone()
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        // make sure any type vars in the result are in scope (from locked class context)
        let type_vars_in_result: Vec<_> =
            result.iter().flat_map(get_type_var_contained_in_type_annotation).collect();

        let all_type_vars_in_scope = type_vars_in_result.iter().all(|tv| {
            if let TypeAnnotation::TypeVar(t) = tv {
                // Auto-generated type variables will be inferred from runtime values.
                if let TypeEnum::TVar { range, .. } = &*unifier.get_ty(*t)
                    && range.is_empty()
                {
                    return true;
                }
                locked.values().flatten().any(|class_tv| unifier.unioned(*class_tv, *t))
            } else {
                false
            }
        });

        if type_vars_in_result.is_empty() || all_type_vars_in_scope {
            result
        } else {
            return Err(HashSet::from([format!(
                "application of type vars to generic class is not currently supported (at {})",
                params_ast[0].location
            )]));
        }
    } else {
        // check param number here
        if !type_vars.is_empty() {
            return Err(HashSet::from([format!(
                "expect {} type variable parameter but got 0 (at {location})",
                type_vars.len(),
            )]));
        }

        Vec::new()
    };

    Ok(TypeAnnotation::CustomClass { id: obj_id, params: param_type_infos })
}

/// Parses the `id` of a [`ast::ExprKind::Name`] expression as a [`TypeAnnotation`].
fn parse_name_as_type_annotation<T, S: std::hash::BuildHasher + Clone>(
    resolver: &(dyn SymbolResolver + Send + Sync),
    top_level_defs: &[Arc<RwLock<TopLevelDef>>],
    builtin_registry: &Arc<dyn BuiltinRegistry>,
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
    locked: HashMap<DefinitionId, Vec<Type>, S>,
    expr: &ast::Expr<T>,
) -> Result<TypeAnnotation, HashSet<String>> {
    let location = &expr.location;
    let ast::ExprKind::Name { id, .. } = &expr.node else { unreachable!("must be name expr here") };
    if let Some(builtin) = builtin_registry.match_builtin(&erase_expr_type(expr)) {
        if builtin == PrimDef::Auto {
            let auto_var = unifier.get_fresh_var(Some(*id), Some(*location)).ty;
            return Ok(TypeAnnotation::TypeVar(auto_var));
        }
        if let PrimDefDetails::PrimClass { get_ty_fn, .. } = builtin.details() {
            match builtin {
                PrimDef::Exception => {
                    return Ok(TypeAnnotation::CustomClass {
                        id: PrimDef::Exception.id(),
                        params: Vec::default(),
                    });
                }
                _ => {
                    return Ok(TypeAnnotation::Primitive(get_ty_fn(primitives)));
                }
            }
        }
    }
    if let Ok(obj_id) = resolver.get_identifier_def(*id) {
        class_def_id_to_type_annotation(
            resolver,
            top_level_defs,
            builtin_registry,
            unifier,
            primitives,
            locked,
            *id,
            (obj_id, None as Option<&Expr<T>>),
            location,
        )
    } else if let Ok(ty) = resolver.get_symbol_type(unifier, top_level_defs, primitives, *id) {
        if let TypeEnum::TVar { .. } = unifier.get_ty(ty).as_ref() {
            let var = unifier.get_fresh_var(Some(*id), Some(*location)).ty;
            unifier.unify(var, ty).unwrap();
            Ok(TypeAnnotation::TypeVar(ty))
        } else {
            Err(HashSet::from([format!("`{id}` is not a valid type annotation (at {location})",)]))
        }
    } else {
        Err(HashSet::from([format!("`{id}` is not a valid type annotation (at {location})",)]))
    }
}

/// Parses the `id` and generic arguments of a class as a [`TypeAnnotation`].
#[allow(clippy::too_many_arguments)]
fn parse_class_id_as_type_annotation<T, S: std::hash::BuildHasher + Clone>(
    resolver: &(dyn SymbolResolver + Send + Sync),
    top_level_defs: &[Arc<RwLock<TopLevelDef>>],
    builtin_registry: &Arc<dyn BuiltinRegistry>,
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
    locked: HashMap<DefinitionId, Vec<Type>, S>,
    value: &Expr<T>,
    id: StrRef,
    slice: &Expr<T>,
    location: &Location,
) -> Result<TypeAnnotation, HashSet<String>> {
    let obj_id = if let Some(builtin) = builtin_registry.match_builtin(&erase_expr_type(value)) {
        match builtin {
            PrimDef::Auto
            | PrimDef::Kernel
            | PrimDef::KernelInvariant
            | PrimDef::ConstGeneric
            | PrimDef::None
            | PrimDef::Virtual
            | PrimDef::Option => {
                return Err(HashSet::from([format!(
                    "keywords cannot be class name (at {location})"
                )]));
            }
            _ => builtin.id(),
        }
    } else {
        resolver.get_identifier_def(id)?
    };

    class_def_id_to_type_annotation(
        resolver,
        top_level_defs,
        builtin_registry,
        unifier,
        primitives,
        locked,
        id,
        (obj_id, Some(slice)),
        location,
    )
}

/// Resolves an AST of a possibly-qualified class name into a [`DefinitionId`].
fn resolve_class_to_id<T>(
    resolver: &(dyn SymbolResolver + Send + Sync),
    top_level_defs: &[Arc<RwLock<TopLevelDef>>],
    expr: &Expr<T>,
) -> Result<DefinitionId, HashSet<String>> {
    /// Variant of `resolve_class_to_id` that accepts a tuple of the parent qualified expression
    /// `value` and the attribute `attr`.
    fn resolve_id<T>(
        resolver: &(dyn SymbolResolver + Send + Sync),
        top_level_defs: &[Arc<RwLock<TopLevelDef>>],
        (parent, attr): (Option<&Expr<T>>, &StrRef),
        location: &Location,
    ) -> Result<DefinitionId, HashSet<String>> {
        // If the path is unqualified, directly resolve using the provided `resolver`
        let Some(parent) = parent else {
            return resolver.get_identifier_def(*attr);
        };

        // Unpack the parent expression into its qualified parent and attribute
        let (parent_value, parent_attr) = match &parent.node {
            ast::ExprKind::Attribute { value, attr, .. } => (Some(value.as_ref()), attr),
            ast::ExprKind::Name { id, .. } => (None, id),
            _ => {
                return Err(HashSet::from([format!(
                    "unsupported expression type for class name (at {location})",
                )]));
            }
        };

        // Resolve the direct parent module our attribute belongs to
        let parent_id =
            resolve_id(resolver, top_level_defs, (parent_value, parent_attr), &parent.location)?;
        let Some(mod_tld) = top_level_defs.get(parent_id.0) else {
            return Err(HashSet::from([format!(
                "NameError: name '{parent_attr}' is not defined (at {})",
                parent.location
            )]));
        };
        let TopLevelDef::Module { simple_name: mod_name, resolver: mod_resolver, .. } =
            &*mod_tld.read()
        else {
            unreachable!("must be module here")
        };

        // Resolve the attribute using the resolver of our direct parent
        resolve_id::<T>(
            mod_resolver.as_ref().map_or(resolver, Arc::as_ref),
            top_level_defs,
            (None, attr),
            location,
        )
        .map_err(|_| {
            HashSet::from([format!(
                "AttributeError: module '{mod_name}' has no attribute '{attr}' (at {location})",
            )])
        })
    }

    match &expr.node {
        ast::ExprKind::Name { id, .. } => {
            resolve_id::<T>(resolver, top_level_defs, (None, id), &expr.location)
        }

        ast::ExprKind::Attribute { value, attr, .. } => {
            resolve_id(resolver, top_level_defs, (Some(value), attr), &expr.location)
        }

        _ => Err(HashSet::from([format!(
            "unsupported expression type for class name (at {})",
            expr.location
        )])),
    }
}

/// Parses an AST expression `expr` into a [`TypeAnnotation`].
///
/// * `locked` - A [`HashMap`] containing the IDs of known definitions, mapped to a [`Vec`] of all
///   generic variables associated with the definition.
/// * `type_var` - The type variable associated with the type argument currently being parsed. Pass
///   [`None`] when this function is invoked externally.
pub fn parse_ast_to_type_annotation_kinds<T, S: std::hash::BuildHasher + Clone>(
    resolver: &(dyn SymbolResolver + Send + Sync),
    top_level_defs: &[Arc<RwLock<TopLevelDef>>],
    builtin_registry: &Arc<dyn BuiltinRegistry>,
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
    expr: &ast::Expr<T>,
    // the key stores the type_var of this topleveldef::class, we only need this field here
    locked: HashMap<DefinitionId, Vec<Type>, S>,
) -> Result<TypeAnnotation, HashSet<String>> {
    match &expr.node {
        ast::ExprKind::Name { .. } => parse_name_as_type_annotation::<T, S>(
            resolver,
            top_level_defs,
            builtin_registry,
            unifier,
            primitives,
            locked,
            expr,
        ),

        ast::ExprKind::Subscript { value, slice, .. } => {
            let builtin = builtin_registry.match_builtin(&erase_expr_type(value));
            match builtin {
                // virtual
                Some(PrimDef::Virtual) => {
                    let def = parse_ast_to_type_annotation_kinds(
                        resolver,
                        top_level_defs,
                        builtin_registry,
                        unifier,
                        primitives,
                        slice.as_ref(),
                        locked,
                    )?;
                    if !matches!(def, TypeAnnotation::CustomClass { .. }) {
                        unreachable!("must be concretized custom class kind in the virtual")
                    }
                    Ok(TypeAnnotation::Virtual(def.into()))
                }

                // option
                Some(PrimDef::Option) => {
                    let def_ann = parse_ast_to_type_annotation_kinds(
                        resolver,
                        top_level_defs,
                        builtin_registry,
                        unifier,
                        primitives,
                        slice.as_ref(),
                        locked,
                    )?;
                    let id = if let TypeEnum::TObj { obj_id, .. } =
                        unifier.get_ty(primitives.option).as_ref()
                    {
                        *obj_id
                    } else {
                        unreachable!()
                    };
                    Ok(TypeAnnotation::CustomClass { id, params: vec![def_ann] })
                }

                // tuple
                Some(PrimDef::Tuple) => {
                    let tup_elts = match &slice.node {
                        ast::ExprKind::Tuple { elts, .. } => elts.as_slice(),
                        _ => std::slice::from_ref(slice.as_ref()),
                    };
                    let type_annotations = tup_elts
                        .iter()
                        .map(|e| {
                            parse_ast_to_type_annotation_kinds(
                                resolver,
                                top_level_defs,
                                builtin_registry,
                                unifier,
                                primitives,
                                e,
                                locked.clone(),
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(TypeAnnotation::Tuple(type_annotations))
                }

                // Literal
                Some(PrimDef::Literal) => {
                    let tup_elts = match &slice.node {
                        ast::ExprKind::Tuple { elts, .. } => elts.as_slice(),
                        _ => std::slice::from_ref(slice.as_ref()),
                    };
                    let type_annotations = tup_elts
                        .iter()
                        .map(|e| match &e.node {
                            ast::ExprKind::Constant { value, .. } => {
                                Ok(TypeAnnotation::Literal(vec![value.clone()]))
                            }
                            _ => parse_ast_to_type_annotation_kinds(
                                resolver,
                                top_level_defs,
                                builtin_registry,
                                unifier,
                                primitives,
                                e,
                                locked.clone(),
                            ),
                        })
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .flat_map(|type_ann| match type_ann {
                            TypeAnnotation::Literal(values) => values,
                            _ => unreachable!(),
                        })
                        .collect_vec();

                    if type_annotations.len() == 1 {
                        Ok(TypeAnnotation::Literal(type_annotations))
                    } else {
                        Err(HashSet::from([format!(
                            "multiple literal bounds are currently unsupported (at {})",
                            value.location
                        )]))
                    }
                }

                // custom class
                _ => match &value.node {
                    ast::ExprKind::Name { id, .. } => parse_class_id_as_type_annotation(
                        resolver,
                        top_level_defs,
                        builtin_registry,
                        unifier,
                        primitives,
                        locked,
                        value,
                        *id,
                        slice,
                        &expr.location,
                    ),

                    ast::ExprKind::Attribute { attr, .. } => {
                        let def_id = match builtin {
                            Some(builtin) => builtin.id(),
                            None => resolve_class_to_id(resolver, top_level_defs, value)?,
                        };
                        class_def_id_to_type_annotation(
                            resolver,
                            top_level_defs,
                            builtin_registry,
                            unifier,
                            primitives,
                            locked,
                            *attr,
                            (def_id, Some(slice)),
                            &expr.location,
                        )
                    }

                    _ => Err(HashSet::from([format!(
                        "unsupported expression type for class name (at {})",
                        value.location
                    )])),
                },
            }
        }

        ast::ExprKind::Constant { value, .. } => Ok(TypeAnnotation::Literal(vec![value.clone()])),

        ast::ExprKind::Attribute { attr, .. } => {
            let builtin = builtin_registry.match_builtin(&erase_expr_type(expr));
            let def_id = match builtin {
                Some(builtin) => builtin.id(),
                None => resolve_class_to_id(resolver, top_level_defs, expr)?,
            };

            class_def_id_to_type_annotation::<T, S>(
                resolver,
                top_level_defs,
                builtin_registry,
                unifier,
                primitives,
                locked,
                *attr,
                (def_id, None),
                &expr.location,
            )
        }

        _ => Err(HashSet::from([format!(
            "unsupported expression for type annotation (at {})",
            expr.location
        )])),
    }
}

// no need to have the `locked` parameter, unlike the `parse_ast_to_type_annotation_kinds`, since
// when calling this function, there should be no topleveldef::class being write, and this function
// also only read the toplevedefs
pub fn get_type_from_type_annotation_kinds(
    top_level_defs: &[Arc<RwLock<TopLevelDef>>],
    unifier: &mut Unifier,
    primitives: &PrimitiveStore,
    ann: &TypeAnnotation,
    subst_list: &mut Option<Vec<Type>>,
) -> Result<Type, HashSet<String>> {
    match ann {
        TypeAnnotation::CustomClass { id: obj_id, params } => {
            // let def_read = top_level_defs[obj_id.0].read();
            let class_def: &TopLevelDef = &top_level_defs[obj_id.0].read();
            let TopLevelDef::Class { fields, methods, type_vars, .. } = class_def else {
                unreachable!("should be class def here")
            };

            if type_vars.len() != params.len() {
                return Err(HashSet::from([format!(
                    "unexpected number of type parameters: expected {} but got {}",
                    type_vars.len(),
                    params.len()
                )]));
            }

            let param_ty = params
                .iter()
                .map(|x| {
                    get_type_from_type_annotation_kinds(
                        top_level_defs,
                        unifier,
                        primitives,
                        x,
                        subst_list,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;

            let ty = if let Some(prim_def) = PrimDef::iter().find(|prim| prim.id() == *obj_id) {
                // Primitive TopLevelDefs do not contain all fields that are present in their Type
                // counterparts, so directly perform subst on the Type instead.

                let PrimDefDetails::PrimClass { get_ty_fn, .. } = prim_def.details() else {
                    unreachable!()
                };

                let base_ty = get_ty_fn(primitives);
                let params =
                    if let TypeEnum::TObj { params, .. } = &*unifier.get_ty_immutable(base_ty) {
                        params.clone()
                    } else {
                        unreachable!()
                    };

                unifier
                    .subst(
                        get_ty_fn(primitives),
                        &params
                            .iter()
                            .zip(param_ty)
                            .map(|(obj_tv, param)| (*obj_tv.0, param))
                            .collect(),
                    )
                    .unwrap_or(base_ty)
            } else {
                let subst = {
                    // check for compatible range
                    // TODO: if allow type var to be applied(now this disallowed in the parse_to_type_annotation), need more check
                    let mut result = VarMap::new();
                    for (tvar, p) in type_vars.iter().zip(param_ty) {
                        match unifier.get_ty(*tvar).as_ref() {
                            TypeEnum::TVar {
                                id,
                                range,
                                fields: None,
                                name,
                                loc,
                                is_const_generic: false,
                            } => {
                                let ok: bool = {
                                    // create a temp type var and unify to check compatibility
                                    p == *tvar || {
                                        let temp = unifier.get_fresh_var_with_range(
                                            range.as_slice(),
                                            *name,
                                            *loc,
                                        );
                                        unifier.unify(temp.ty, p).is_ok()
                                    }
                                };
                                if ok {
                                    result.insert(*id, p);
                                } else {
                                    return Err(HashSet::from([format!(
                                        "cannot apply type {} to type variable with id {:?}",
                                        unifier.internal_stringify(
                                            p,
                                            &mut |id| format!("class{id}"),
                                            &mut |id| format!("typevar{id}"),
                                            &mut None
                                        ),
                                        *id
                                    )]));
                                }
                            }

                            TypeEnum::TVar {
                                id, range, name, loc, is_const_generic: true, ..
                            } => {
                                let ty = range[0];
                                let ok: bool = {
                                    // create a temp type var and unify to check compatibility
                                    p == *tvar || {
                                        let temp =
                                            unifier.get_fresh_const_generic_var(ty, *name, *loc);
                                        unifier.unify(temp.ty, p).is_ok()
                                    }
                                };
                                if ok {
                                    result.insert(*id, p);
                                } else {
                                    return Err(HashSet::from([format!(
                                        "cannot apply type {} to type variable {}",
                                        unifier.stringify(p),
                                        name.unwrap_or_else(|| format!("typevar{id}").into()),
                                    )]));
                                }
                            }

                            _ => unreachable!("must be generic type var"),
                        }
                    }
                    result
                };
                // Class Attributes keep a copy with Class Definition and are not added to objects
                let mut tobj_fields = methods
                    .iter()
                    .map(|(name, ty, _)| {
                        let subst_ty = unifier.subst(*ty, &subst).unwrap_or(*ty);
                        // methods are immutable
                        (*name, (subst_ty, AttrKind::Method))
                    })
                    .collect::<HashMap<_, _>>();
                tobj_fields.extend(fields.iter().map(|(name, ty, mutability)| {
                    let subst_ty = unifier.subst(*ty, &subst).unwrap_or(*ty);
                    (*name, (subst_ty, AttrKind::Field { mutable: *mutability }))
                }));
                let need_subst = !subst.is_empty();
                let ty = unifier.add_ty(TypeEnum::TObj {
                    obj_id: *obj_id,
                    fields: tobj_fields,
                    params: subst,
                });

                if need_subst && let Some(wl) = subst_list.as_mut() {
                    wl.push(ty);
                }

                ty
            };

            Ok(ty)
        }
        TypeAnnotation::Primitive(ty) | TypeAnnotation::TypeVar(ty) => Ok(*ty),
        TypeAnnotation::Literal(values) => {
            let values = values
                .iter()
                .map(SymbolValue::from_constant_inferred)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| HashSet::from([err]))?;

            let var = unifier.get_fresh_literal(values, None);
            Ok(var)
        }
        TypeAnnotation::Virtual(ty) => {
            let ty = get_type_from_type_annotation_kinds(
                top_level_defs,
                unifier,
                primitives,
                ty.as_ref(),
                subst_list,
            )?;
            Ok(unifier.add_ty(TypeEnum::TVirtual { ty }))
        }
        TypeAnnotation::Tuple(tys) => {
            let tys = tys
                .iter()
                .map(|x| {
                    get_type_from_type_annotation_kinds(
                        top_level_defs,
                        unifier,
                        primitives,
                        x,
                        subst_list,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(unifier.add_ty(TypeEnum::TTuple { ty: tys, is_vararg_ctx: false }))
        }
    }
}

/// given an def id, return a type annotation of self \
/// ```python
/// class A(Generic[T, V]):
///     def fun(self):
/// ```
/// the type of `self` should be similar to `A[T, V]`, where `T`, `V`
/// considered to be type variables associated with the class \
/// \
/// But note that here we do not make a duplication of `T`, `V`, we directly
/// use them as they are in the [`TopLevelDef::Class`] since those in the
/// `TopLevelDef::Class.type_vars` will be substitute later when seeing applications/instantiations
/// the Type of their fields and methods will also be subst when application/instantiation
#[must_use]
pub fn make_self_type_annotation(type_vars: &[Type], object_id: DefinitionId) -> TypeAnnotation {
    TypeAnnotation::CustomClass {
        id: object_id,
        params: type_vars.iter().map(|ty| TypeAnnotation::TypeVar(*ty)).collect_vec(),
    }
}

/// get all the occurences of type vars contained in a type annotation
/// e.g. `A[int, B[T], V, virtual[C[G]]]` => [T, V, G]
/// this function will not make a duplicate of type var
#[must_use]
pub fn get_type_var_contained_in_type_annotation(ann: &TypeAnnotation) -> Vec<TypeAnnotation> {
    let mut result: Vec<TypeAnnotation> = Vec::new();
    match ann {
        TypeAnnotation::TypeVar(..) => result.push(ann.clone()),
        TypeAnnotation::Virtual(ann) => {
            result.extend(get_type_var_contained_in_type_annotation(ann.as_ref()));
        }
        TypeAnnotation::CustomClass { params, .. } => {
            for p in params {
                result.extend(get_type_var_contained_in_type_annotation(p));
            }
        }
        TypeAnnotation::Tuple(anns) => {
            for a in anns {
                result.extend(get_type_var_contained_in_type_annotation(a));
            }
        }
        TypeAnnotation::Primitive(..) | TypeAnnotation::Literal { .. } => {}
    }
    result
}

/// check the type compatibility for overload
pub fn check_overload_type_annotation_compatible(
    this: &TypeAnnotation,
    other: &TypeAnnotation,
    unifier: &mut Unifier,
) -> bool {
    match (this, other) {
        (TypeAnnotation::Primitive(a), TypeAnnotation::Primitive(b)) => a == b,
        (TypeAnnotation::TypeVar(a), TypeAnnotation::TypeVar(b)) => {
            let a = unifier.get_ty(*a);
            let a = &*a;
            let b = unifier.get_ty(*b);
            let b = &*b;
            let (
                TypeEnum::TVar { id: a, fields: None, .. },
                TypeEnum::TVar { id: b, fields: None, .. },
            ) = (a, b)
            else {
                unreachable!("must be type var")
            };

            a == b
        }
        (TypeAnnotation::Virtual(a), TypeAnnotation::Virtual(b)) => {
            check_overload_type_annotation_compatible(a.as_ref(), b.as_ref(), unifier)
        }

        (TypeAnnotation::Tuple(a), TypeAnnotation::Tuple(b)) => {
            a.len() == b.len() && {
                a.iter()
                    .zip(b)
                    .all(|(a, b)| check_overload_type_annotation_compatible(a, b, unifier))
            }
        }

        (
            TypeAnnotation::CustomClass { id: a, params: a_p },
            TypeAnnotation::CustomClass { id: b, params: b_p },
        ) => {
            a.0 == b.0 && {
                a_p.len() == b_p.len() && {
                    a_p.iter()
                        .zip(b_p)
                        .all(|(a, b)| check_overload_type_annotation_compatible(a, b, unifier))
                }
            }
        }

        _ => false,
    }
}
