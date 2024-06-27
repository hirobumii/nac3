use std::collections::{HashMap, HashSet};
use std::convert::{From, TryInto};
use std::iter::once;
use std::ops::Not;
use std::{cell::RefCell, sync::Arc};

use super::typedef::{Call, FunSignature, FuncArg, RecordField, Type, TypeEnum, Unifier, VarMap};
use super::{magic_methods::*, type_error::TypeError, typedef::CallId};
use crate::toplevel::TopLevelDef;
use crate::{
    symbol_resolver::{SymbolResolver, SymbolValue},
    toplevel::{
        helper::{arraylike_flatten_element_type, arraylike_get_ndims, PrimDef},
        numpy::{make_ndarray_ty, unpack_ndarray_var_tys},
        TopLevelContext,
    },
};
use itertools::{izip, Itertools};
use nac3parser::ast::{
    self,
    fold::{self, Fold},
    Arguments, Comprehension, ExprContext, ExprKind, Located, Location, StrRef,
};

#[cfg(test)]
mod test;

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub struct CodeLocation {
    row: usize,
    col: usize,
}

impl From<Location> for CodeLocation {
    fn from(loc: Location) -> CodeLocation {
        CodeLocation { row: loc.row(), col: loc.column() }
    }
}

#[derive(Clone, Copy)]
pub struct PrimitiveStore {
    pub int32: Type,
    pub int64: Type,
    pub uint32: Type,
    pub uint64: Type,
    pub float: Type,
    pub bool: Type,
    pub none: Type,
    pub range: Type,
    pub str: Type,
    pub exception: Type,
    pub option: Type,
    pub ndarray: Type,
    pub size_t: u32,
}

impl PrimitiveStore {
    /// Returns a [`Type`] representing a signed representation of `size_t`.
    #[must_use]
    pub fn isize(&self) -> Type {
        match self.size_t {
            32 => self.int32,
            64 => self.int64,
            _ => unreachable!(),
        }
    }

    /// Returns a [Type] representing `size_t`.
    #[must_use]
    pub fn usize(&self) -> Type {
        match self.size_t {
            32 => self.uint32,
            64 => self.uint64,
            _ => unreachable!(),
        }
    }
}

pub struct FunctionData {
    pub resolver: Arc<dyn SymbolResolver + Send + Sync>,
    pub return_type: Option<Type>,
    pub bound_variables: Vec<Type>,
}

pub struct Inferencer<'a> {
    pub top_level: &'a TopLevelContext,
    pub defined_identifiers: HashSet<StrRef>,
    pub function_data: &'a mut FunctionData,
    pub unifier: &'a mut Unifier,
    pub primitives: &'a PrimitiveStore,
    pub virtual_checks: &'a mut Vec<(Type, Type, Location)>,
    pub variable_mapping: HashMap<StrRef, Type>,
    pub calls: &'a mut HashMap<CodeLocation, CallId>,
    pub in_handler: bool,
}

struct NaiveFolder();
impl Fold<()> for NaiveFolder {
    type TargetU = Option<Type>;
    type Error = HashSet<String>;
    fn map_user(&mut self, (): ()) -> Result<Self::TargetU, Self::Error> {
        Ok(None)
    }
}

fn report_error<T>(msg: &str, location: Location) -> Result<T, HashSet<String>> {
    Err(HashSet::from([format!("{msg} at {location}")]))
}

impl<'a> Fold<()> for Inferencer<'a> {
    type TargetU = Option<Type>;
    type Error = HashSet<String>;

    fn map_user(&mut self, (): ()) -> Result<Self::TargetU, Self::Error> {
        Ok(None)
    }

    fn fold_stmt(
        &mut self,
        mut node: ast::Stmt<()>,
    ) -> Result<ast::Stmt<Self::TargetU>, Self::Error> {
        let stmt = match node.node {
            // we don't want fold over type annotation
            ast::StmtKind::AnnAssign { mut target, annotation, value, simple, config_comment } => {
                self.infer_pattern(&target)?;
                // fix parser problem...
                if let ExprKind::Attribute { ctx, .. } = &mut target.node {
                    *ctx = ExprContext::Store;
                }

                let target = Box::new(self.fold_expr(*target)?);
                let value = if let Some(v) = value {
                    let ty = Box::new(self.fold_expr(*v)?);
                    self.unify(target.custom.unwrap(), ty.custom.unwrap(), &node.location)?;
                    Some(ty)
                } else {
                    return report_error(
                        "declaration without definition is not yet supported",
                        node.location,
                    );
                };
                let top_level_defs = self.top_level.definitions.read();
                let annotation_type = self.function_data.resolver.parse_type_annotation(
                    top_level_defs.as_slice(),
                    self.unifier,
                    self.primitives,
                    annotation.as_ref(),
                )?;
                self.unify(annotation_type, target.custom.unwrap(), &node.location)?;
                let annotation = Box::new(NaiveFolder().fold_expr(*annotation)?);
                Located {
                    location: node.location,
                    custom: None,
                    node: ast::StmtKind::AnnAssign {
                        target,
                        annotation,
                        value,
                        simple,
                        config_comment,
                    },
                }
            }
            ast::StmtKind::Try { body, handlers, orelse, finalbody, config_comment } => {
                let body = body
                    .into_iter()
                    .map(|stmt| self.fold_stmt(stmt))
                    .collect::<Result<Vec<_>, _>>()?;
                let outer_in_handler = self.in_handler;
                let mut exception_handlers = Vec::with_capacity(handlers.len());
                self.in_handler = true;
                {
                    let top_level_defs = self.top_level.definitions.read();
                    let mut naive_folder = NaiveFolder();
                    for handler in handlers {
                        let ast::ExcepthandlerKind::ExceptHandler { type_, name, body } =
                            handler.node;
                        let type_ = if let Some(type_) = type_ {
                            let typ = self.function_data.resolver.parse_type_annotation(
                                top_level_defs.as_slice(),
                                self.unifier,
                                self.primitives,
                                &type_,
                            )?;
                            self.virtual_checks.push((
                                typ,
                                self.primitives.exception,
                                handler.location,
                            ));
                            if let Some(name) = name {
                                if !self.defined_identifiers.contains(&name) {
                                    self.defined_identifiers.insert(name);
                                }
                                if let Some(old_typ) = self.variable_mapping.insert(name, typ) {
                                    let loc = handler.location;
                                    self.unifier.unify(old_typ, typ).map_err(|e| {
                                        HashSet::from([e
                                            .at(Some(loc))
                                            .to_display(self.unifier)
                                            .to_string()])
                                    })?;
                                }
                            }
                            let mut type_ = naive_folder.fold_expr(*type_)?;
                            type_.custom = Some(typ);
                            Some(Box::new(type_))
                        } else {
                            None
                        };
                        let body = body
                            .into_iter()
                            .map(|stmt| self.fold_stmt(stmt))
                            .collect::<Result<Vec<_>, _>>()?;
                        exception_handlers.push(Located {
                            location: handler.location,
                            node: ast::ExcepthandlerKind::ExceptHandler { type_, name, body },
                            custom: None,
                        });
                    }
                }
                self.in_handler = outer_in_handler;
                let handlers = exception_handlers;
                let orelse = orelse.into_iter().map(|stmt| self.fold_stmt(stmt)).collect::<Result<
                    Vec<_>,
                    _,
                >>(
                )?;
                let finalbody = finalbody
                    .into_iter()
                    .map(|stmt| self.fold_stmt(stmt))
                    .collect::<Result<Vec<_>, _>>()?;
                Located {
                    location: node.location,
                    node: ast::StmtKind::Try { body, handlers, orelse, finalbody, config_comment },
                    custom: None,
                }
            }
            ast::StmtKind::For { target, iter, body, orelse, config_comment, type_comment } => {
                self.infer_pattern(&target)?;
                let target = self.fold_expr(*target)?;
                let iter = self.fold_expr(*iter)?;
                if self.unifier.unioned(iter.custom.unwrap(), self.primitives.range) {
                    self.unify(self.primitives.int32, target.custom.unwrap(), &target.location)?;
                } else {
                    let list_like_ty = match &*self.unifier.get_ty(iter.custom.unwrap()) {
                        TypeEnum::TList { .. } => {
                            self.unifier.add_ty(TypeEnum::TList { ty: target.custom.unwrap() })
                        }
                        TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
                            todo!()
                        }
                        _ => unreachable!(),
                    };
                    self.unify(list_like_ty, iter.custom.unwrap(), &iter.location)?;
                }
                let body =
                    body.into_iter().map(|b| self.fold_stmt(b)).collect::<Result<Vec<_>, _>>()?;
                let orelse =
                    orelse.into_iter().map(|o| self.fold_stmt(o)).collect::<Result<Vec<_>, _>>()?;
                Located {
                    location: node.location,
                    node: ast::StmtKind::For {
                        target: Box::new(target),
                        iter: Box::new(iter),
                        body,
                        orelse,
                        config_comment,
                        type_comment,
                    },
                    custom: None,
                }
            }
            ast::StmtKind::Assign { ref mut targets, ref config_comment, .. } => {
                for target in &mut *targets {
                    if let ExprKind::Attribute { ctx, .. } = &mut target.node {
                        *ctx = ExprContext::Store;
                    }
                }
                if targets.iter().all(|t| matches!(t.node, ExprKind::Name { .. })) {
                    let ast::StmtKind::Assign { targets, value, .. } = node.node else {
                        unreachable!()
                    };

                    let value = self.fold_expr(*value)?;
                    let value_ty = value.custom.unwrap();
                    let targets: Result<Vec<_>, _> = targets
                        .into_iter()
                        .map(|target| {
                            let ExprKind::Name { id, ctx } = target.node else { unreachable!() };

                            self.defined_identifiers.insert(id);
                            let target_ty = if let Some(ty) = self.variable_mapping.get(&id) {
                                *ty
                            } else {
                                let unifier: &mut Unifier = self.unifier;
                                self.function_data
                                    .resolver
                                    .get_symbol_type(
                                        unifier,
                                        &self.top_level.definitions.read(),
                                        self.primitives,
                                        id,
                                    )
                                    .unwrap_or_else(|_| {
                                        self.variable_mapping.insert(id, value_ty);
                                        value_ty
                                    })
                            };
                            let location = target.location;
                            self.unifier.unify(value_ty, target_ty).map(|()| Located {
                                location,
                                node: ExprKind::Name { id, ctx },
                                custom: Some(target_ty),
                            })
                        })
                        .collect();
                    let loc = node.location;
                    let targets = targets.map_err(|e| {
                        HashSet::from([e.at(Some(loc)).to_display(self.unifier).to_string()])
                    })?;
                    return Ok(Located {
                        location: node.location,
                        node: ast::StmtKind::Assign {
                            targets,
                            value: Box::new(value),
                            type_comment: None,
                            config_comment: config_comment.clone(),
                        },
                        custom: None,
                    });
                }
                for target in targets {
                    self.infer_pattern(target)?;
                }
                fold::fold_stmt(self, node)?
            }
            ast::StmtKind::With { ref items, .. } => {
                for item in items {
                    if let Some(var) = &item.optional_vars {
                        self.infer_pattern(var)?;
                    }
                }
                fold::fold_stmt(self, node)?
            }
            _ => fold::fold_stmt(self, node)?,
        };
        match &stmt.node {
            ast::StmtKind::AnnAssign { .. }
            | ast::StmtKind::Break { .. }
            | ast::StmtKind::Continue { .. }
            | ast::StmtKind::Expr { .. }
            | ast::StmtKind::For { .. }
            | ast::StmtKind::Pass { .. }
            | ast::StmtKind::Try { .. } => {}
            ast::StmtKind::If { test, .. } | ast::StmtKind::While { test, .. } => {
                self.unify(test.custom.unwrap(), self.primitives.bool, &test.location)?;
            }
            ast::StmtKind::Assign { targets, value, .. } => {
                for target in targets {
                    self.unify(target.custom.unwrap(), value.custom.unwrap(), &target.location)?;
                }
            }
            ast::StmtKind::Raise { exc, cause, .. } => {
                if let Some(cause) = cause {
                    return report_error("raise ... from cause is not supported", cause.location);
                }
                if let Some(exc) = exc {
                    self.virtual_checks.push((
                        exc.custom.unwrap(),
                        self.primitives.exception,
                        exc.location,
                    ));
                } else if !self.in_handler {
                    return report_error(
                        "cannot reraise outside exception handlers",
                        stmt.location,
                    );
                }
            }
            ast::StmtKind::With { items, .. } => {
                for item in items {
                    let ty = item.context_expr.custom.unwrap();
                    // if we can simply unify without creating new types...
                    let mut fast_path = false;
                    if let TypeEnum::TObj { fields, .. } = &*self.unifier.get_ty(ty) {
                        fast_path = true;
                        if let Some(enter) = fields.get(&"__enter__".into()).copied() {
                            if let TypeEnum::TFunc(signature) = &*self.unifier.get_ty(enter.0) {
                                if !signature.args.is_empty() {
                                    return report_error(
                                        "__enter__ method should take no argument other than self",
                                        stmt.location,
                                    );
                                }
                                if let Some(var) = &item.optional_vars {
                                    if signature.vars.is_empty() {
                                        self.unify(
                                            signature.ret,
                                            var.custom.unwrap(),
                                            &stmt.location,
                                        )?;
                                    } else {
                                        fast_path = false;
                                    }
                                }
                            } else {
                                fast_path = false;
                            }
                        } else {
                            return report_error(
                                "__enter__ method is required for context manager",
                                stmt.location,
                            );
                        }
                        if let Some(exit) = fields.get(&"__exit__".into()).copied() {
                            if let TypeEnum::TFunc(signature) = &*self.unifier.get_ty(exit.0) {
                                if !signature.args.is_empty() {
                                    return report_error(
                                        "__exit__ method should take no argument other than self",
                                        stmt.location,
                                    );
                                }
                            } else {
                                fast_path = false;
                            }
                        } else {
                            return report_error(
                                "__exit__ method is required for context manager",
                                stmt.location,
                            );
                        }
                    }
                    if !fast_path {
                        let enter = TypeEnum::TFunc(FunSignature {
                            args: vec![],
                            ret: item.optional_vars.as_ref().map_or_else(
                                || self.unifier.get_dummy_var().ty,
                                |var| var.custom.unwrap(),
                            ),
                            vars: VarMap::default(),
                        });
                        let enter = self.unifier.add_ty(enter);
                        let exit = TypeEnum::TFunc(FunSignature {
                            args: vec![],
                            ret: self.unifier.get_dummy_var().ty,
                            vars: VarMap::default(),
                        });
                        let exit = self.unifier.add_ty(exit);
                        let mut fields = HashMap::new();
                        fields.insert("__enter__".into(), RecordField::new(enter, false, None));
                        fields.insert("__exit__".into(), RecordField::new(exit, false, None));
                        let record = self.unifier.add_record(fields);
                        self.unify(ty, record, &stmt.location)?;
                    }
                }
            }
            ast::StmtKind::Return { value, .. } => match (value, self.function_data.return_type) {
                (Some(v), Some(v1)) => {
                    self.unify(v.custom.unwrap(), v1, &v.location)?;
                }
                (Some(_), None) => {
                    return report_error("Unexpected return value", stmt.location);
                }
                (None, Some(_)) => {
                    return report_error("Expected return value", stmt.location);
                }
                (None, None) => {}
            },
            ast::StmtKind::AugAssign { target, op, value, .. } => {
                let res_ty = self.infer_bin_ops(stmt.location, target, *op, value, true)?;
                self.unify(res_ty, target.custom.unwrap(), &stmt.location)?;
            }
            ast::StmtKind::Assert { test, msg, .. } => {
                self.unify(test.custom.unwrap(), self.primitives.bool, &test.location)?;
                match msg {
                    Some(m) => self.unify(m.custom.unwrap(), self.primitives.str, &m.location)?,
                    None => (),
                }
            }
            _ => return report_error("Unsupported statement type", stmt.location),
        };
        Ok(stmt)
    }

    fn fold_expr(&mut self, node: ast::Expr<()>) -> Result<ast::Expr<Self::TargetU>, Self::Error> {
        let expr = match node.node {
            ExprKind::Call { func, args, keywords } => {
                return self.fold_call(node.location, *func, args, keywords);
            }
            ExprKind::Lambda { args, body } => {
                return self.fold_lambda(node.location, *args, *body);
            }
            ExprKind::ListComp { elt, generators } => {
                return self.fold_listcomp(node.location, *elt, generators);
            }
            _ => fold::fold_expr(self, node)?,
        };
        let custom = match &expr.node {
            ExprKind::Constant { value, .. } => Some(self.infer_constant(value, &expr.location)?),
            ExprKind::Name { id, .. } => {
                // the name `none` is special since it may have different types
                if id == &"none".into() {
                    if let TypeEnum::TObj { params, .. } =
                        self.unifier.get_ty_immutable(self.primitives.option).as_ref()
                    {
                        let var_map = params
                            .iter()
                            .map(|(id_var, ty)| {
                                let TypeEnum::TVar { id, range, name, loc, .. } =
                                    &*self.unifier.get_ty(*ty)
                                else {
                                    unreachable!()
                                };

                                assert_eq!(*id, *id_var);
                                (*id, self.unifier.get_fresh_var_with_range(range, *name, *loc).ty)
                            })
                            .collect::<VarMap>();
                        Some(self.unifier.subst(self.primitives.option, &var_map).unwrap())
                    } else {
                        unreachable!("must be tobj")
                    }
                } else {
                    if !self.defined_identifiers.contains(id) {
                        match self.function_data.resolver.get_symbol_type(
                            self.unifier,
                            &self.top_level.definitions.read(),
                            self.primitives,
                            *id,
                        ) {
                            Ok(_) => {
                                self.defined_identifiers.insert(*id);
                            }
                            Err(e) => {
                                return report_error(
                                    &format!("type error at identifier `{id}` ({e})"),
                                    expr.location,
                                );
                            }
                        }
                    }
                    Some(self.infer_identifier(*id)?)
                }
            }
            ExprKind::List { elts, .. } => Some(self.infer_list(elts)?),
            ExprKind::Tuple { elts, .. } => Some(self.infer_tuple(elts)?),
            ExprKind::Attribute { value, attr, ctx } => {
                Some(self.infer_attribute(value, *attr, *ctx)?)
            }
            ExprKind::BoolOp { values, .. } => Some(self.infer_bool_ops(values)?),
            ExprKind::BinOp { left, op, right } => {
                Some(self.infer_bin_ops(expr.location, left, *op, right, false)?)
            }
            ExprKind::UnaryOp { op, operand } => {
                Some(self.infer_unary_ops(expr.location, *op, operand)?)
            }
            ExprKind::Compare { left, ops, comparators } => {
                Some(self.infer_compare(expr.location, left, ops, comparators)?)
            }
            ExprKind::Subscript { value, slice, ctx, .. } => {
                Some(self.infer_subscript(value.as_ref(), slice.as_ref(), *ctx)?)
            }
            ExprKind::IfExp { test, body, orelse } => {
                Some(self.infer_if_expr(test, body.as_ref(), orelse.as_ref())?)
            }
            ExprKind::ListComp { .. } | ExprKind::Lambda { .. } | ExprKind::Call { .. } => {
                expr.custom
            } // already computed
            ExprKind::Slice { .. } => {
                // slices aren't exactly ranges, but for our purposes this should suffice
                Some(self.primitives.range)
            }
            _ => return report_error("not supported", expr.location),
        };
        Ok(ast::Expr { custom, location: expr.location, node: expr.node })
    }
}

type InferenceResult = Result<Type, HashSet<String>>;

impl<'a> Inferencer<'a> {
    /// Constrain a <: b
    /// Currently implemented as unification
    fn constrain(&mut self, a: Type, b: Type, location: &Location) -> Result<(), HashSet<String>> {
        self.unify(a, b, location)
    }

    fn unify(&mut self, a: Type, b: Type, location: &Location) -> Result<(), HashSet<String>> {
        self.unifier.unify(a, b).map_err(|e| {
            HashSet::from([e.at(Some(*location)).to_display(self.unifier).to_string()])
        })
    }

    fn infer_pattern(&mut self, pattern: &ast::Expr<()>) -> Result<(), HashSet<String>> {
        match &pattern.node {
            ExprKind::Name { id, .. } => {
                if !self.defined_identifiers.contains(id) {
                    self.defined_identifiers.insert(*id);
                }
                Ok(())
            }
            ExprKind::Tuple { elts, .. } => {
                for elt in elts {
                    self.infer_pattern(elt)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn build_method_call(
        &mut self,
        location: Location,
        method: StrRef,
        obj: Type,
        params: Vec<Type>,
        ret: Option<Type>,
    ) -> InferenceResult {
        if let TypeEnum::TObj { params: class_params, fields, .. } = &*self.unifier.get_ty(obj) {
            if class_params.is_empty() {
                if let Some(ty) = fields.get(&method) {
                    let ty = ty.0;
                    if let TypeEnum::TFunc(sign) = &*self.unifier.get_ty(ty) {
                        if sign.vars.is_empty() {
                            let call = Call {
                                posargs: params,
                                kwargs: HashMap::new(),
                                ret: sign.ret,
                                fun: RefCell::new(None),
                                loc: Some(location),
                            };
                            if let Some(ret) = ret {
                                self.unifier
                                    .unify(sign.ret, ret)
                                    .map_err(|err| {
                                        format!(
                                            "Cannot unify {} <: {} - {:?}",
                                            self.unifier.stringify(sign.ret),
                                            self.unifier.stringify(ret),
                                            TypeError::new(err.kind, Some(location))
                                        )
                                    })
                                    .unwrap();
                            }
                            self.unifier.unify_call(&call, ty, sign).map_err(|e| {
                                HashSet::from([e
                                    .at(Some(location))
                                    .to_display(self.unifier)
                                    .to_string()])
                            })?;
                            return Ok(sign.ret);
                        }
                    }
                }
            }
        }
        let ret = ret.unwrap_or_else(|| self.unifier.get_dummy_var().ty);

        let call = self.unifier.add_call(Call {
            posargs: params,
            kwargs: HashMap::new(),
            ret,
            fun: RefCell::new(None),
            loc: Some(location),
        });
        self.calls.insert(location.into(), call);
        let call = self.unifier.add_ty(TypeEnum::TCall(vec![call]));
        let fields = once((method.into(), RecordField::new(call, false, Some(location)))).collect();
        let record = self.unifier.add_record(fields);
        self.constrain(obj, record, &location)?;
        Ok(ret)
    }

    fn fold_lambda(
        &mut self,
        location: Location,
        args: Arguments,
        body: ast::Expr<()>,
    ) -> Result<ast::Expr<Option<Type>>, HashSet<String>> {
        if !args.posonlyargs.is_empty()
            || args.vararg.is_some()
            || !args.kwonlyargs.is_empty()
            || args.kwarg.is_some()
            || !args.defaults.is_empty()
        {
            // actually I'm not sure whether programs violating this is a valid python program.
            return report_error(
                "We only support positional or keyword arguments without defaults for lambdas",
                if args.args.is_empty() { body.location } else { args.args[0].location },
            );
        }

        let mut defined_identifiers = self.defined_identifiers.clone();
        for arg in &args.args {
            let name = &arg.node.arg;
            if !defined_identifiers.contains(name) {
                defined_identifiers.insert(*name);
            }
        }
        let fn_args: Vec<_> = args
            .args
            .iter()
            .map(|v| {
                (v.node.arg, self.unifier.get_fresh_var(Some(v.node.arg), Some(v.location)).ty)
            })
            .collect();
        let mut variable_mapping = self.variable_mapping.clone();
        variable_mapping.extend(fn_args.iter().copied());
        let ret = self.unifier.get_dummy_var().ty;

        let mut new_context = Inferencer {
            function_data: self.function_data,
            unifier: self.unifier,
            primitives: self.primitives,
            virtual_checks: self.virtual_checks,
            calls: self.calls,
            top_level: self.top_level,
            defined_identifiers,
            variable_mapping,
            // lambda should not be considered in exception handler
            in_handler: false,
        };
        let fun = FunSignature {
            args: fn_args
                .iter()
                .map(|(k, ty)| FuncArg { name: *k, ty: *ty, default_value: None })
                .collect(),
            ret,
            vars: VarMap::default(),
        };
        let body = new_context.fold_expr(body)?;
        new_context.unify(fun.ret, body.custom.unwrap(), &location)?;
        let mut args = new_context.fold_arguments(args)?;
        for (arg, (name, ty)) in args.args.iter_mut().zip(fn_args.iter()) {
            assert_eq!(&arg.node.arg, name);
            arg.custom = Some(*ty);
        }
        Ok(Located {
            location,
            node: ExprKind::Lambda { args: args.into(), body: body.into() },
            custom: Some(self.unifier.add_ty(TypeEnum::TFunc(fun))),
        })
    }

    fn fold_listcomp(
        &mut self,
        location: Location,
        elt: ast::Expr<()>,
        mut generators: Vec<Comprehension>,
    ) -> Result<ast::Expr<Option<Type>>, HashSet<String>> {
        if generators.len() != 1 {
            return report_error(
                "Only 1 generator statement for list comprehension is supported",
                generators[0].target.location,
            );
        }
        let variable_mapping = self.variable_mapping.clone();
        let defined_identifiers = self.defined_identifiers.clone();
        let mut new_context = Inferencer {
            function_data: self.function_data,
            unifier: self.unifier,
            virtual_checks: self.virtual_checks,
            top_level: self.top_level,
            variable_mapping,
            primitives: self.primitives,
            calls: self.calls,
            defined_identifiers,
            // listcomp expr should not be considered as inside an exception handler...
            in_handler: false,
        };
        let generator = generators.pop().unwrap();
        if generator.is_async {
            return report_error("Async iterator not supported", generator.target.location);
        }
        new_context.infer_pattern(&generator.target)?;
        let target = new_context.fold_expr(*generator.target)?;
        let iter = new_context.fold_expr(*generator.iter)?;
        if new_context.unifier.unioned(iter.custom.unwrap(), new_context.primitives.range) {
            new_context.unify(
                target.custom.unwrap(),
                new_context.primitives.int32,
                &target.location,
            )?;
        } else {
            let list = new_context.unifier.add_ty(TypeEnum::TList { ty: target.custom.unwrap() });
            new_context.unify(iter.custom.unwrap(), list, &iter.location)?;
        }
        let ifs: Vec<_> = generator
            .ifs
            .into_iter()
            .map(|v| new_context.fold_expr(v))
            .collect::<Result<_, _>>()?;

        let elt = new_context.fold_expr(elt)?;
        // iter should be a list of targets...
        // actually it should be an iterator of targets, but we don't have iter type for now
        // if conditions should be bool
        for v in &ifs {
            new_context.unify(v.custom.unwrap(), new_context.primitives.bool, &v.location)?;
        }

        Ok(Located {
            location,
            custom: Some(new_context.unifier.add_ty(TypeEnum::TList { ty: elt.custom.unwrap() })),
            node: ExprKind::ListComp {
                elt: Box::new(elt),
                generators: vec![Comprehension {
                    target: Box::new(target),
                    iter: Box::new(iter),
                    ifs,
                    is_async: false,
                }],
            },
        })
    }

    /// Fold an ndarray `shape` argument. This function aims to fold `shape` arguments like that of
    /// <https://numpy.org/doc/stable/reference/generated/numpy.zeros.html> (for `np_zeros`).
    ///
    /// Arguments:
    ///   * `id` - The name of the function of the function call this `shape` argument is in. Used for error reporting.
    ///   * `arg_index` - The position (0-based) of this argument in the function call. Used for error reporting.
    ///   * `shape_expr` - [`Located<ExprKind>`] of the input argument.
    ///
    /// On success, it returns a tuple of
    ///   1) the `ndims` value inferred from the input `shape`,
    ///   2) and the elaborated expression. Like what other fold functions of [`Inferencer`] would normally return.
    fn fold_numpy_function_call_shape_argument(
        &mut self,
        id: StrRef,
        arg_index: usize,
        shape_expr: Located<ExprKind>,
    ) -> Result<(u64, ast::Expr<Option<Type>>), HashSet<String>> {
        /*
            ### Further explanation

            As said, this function aims to fold `shape` arguments, but this is *not* trivial.
            The root of the issue is that `nac3core` has to deduce the `ndims`
            of the created (for in the case of `np_zeros`) ndarray statically - i.e., during inference time.

            There are three types of valid input to `shape`:
              1. A python `List` (all `int32s`);   e.g., `np_zeros([600, 800, 3])`
              2. A python `Tuple` (all `int32s`);  e.g., `np_zeros((600, 800, 3))`
              3. An `int32`; e.g., `np_zeros(256)` - this is functionally equivalent to `np_zeros([256])`

            For 2. and 3., `ndims` can be deduce immediately from the inferred type of the input:
              - For 2. `ndims` is simply the number of elements found in [`TypeEnum::TTuple`] after typechecking the `shape` argument.
              - For 3. `ndims` is simply 1.

            For 1., `ndims` is supposedly the length of the input list. However, the length of the input list
            is a runtime property. Therefore (as a hack) we resort to analyzing the argument expression [`ExprKind::List`]
            itself to extract the input list length statically.

            This implies that the user could only write:

            ```python
            my_rgba_image = np_zeros([600, 800, 4])
            # the shape argument is directly written as a list literal.
            # and `nac3core` could therefore tell that ndims is `3` by
            # looking at the raw AST expression itself.
            ```

            But not:

            ```python
            my_image_dimension = [600, 800, 4]
            mystery_function_that_mutates_my_list(my_image_dimension)
            my_image = np_zeros(my_image_dimension)
            # what is the length now? what is `ndims`?

            # it is *basically impossible* to generally determine the
            # length of `my_image_dimension` statically for `ndims`!!
            ```
        */

        // Fold `shape`
        let shape = self.fold_expr(shape_expr)?;
        let shape_ty = shape.custom.unwrap(); // The inferred type of `shape`

        // Check `shape_ty` to see if its a list of int32s, a tuple of int32s, or just int32.
        // Otherwise throw an error as that would mean the user wrote an ill-typed `shape_expr`.
        //
        // Here, we also take the opportunity to deduce `ndims` statically.
        let shape_ty_enum = &*self.unifier.get_ty(shape_ty);
        let ndims = match shape_ty_enum {
            TypeEnum::TList { ty } => {
                // Handle 1. A list of int32s

                // Typecheck
                self.unifier.unify(*ty, self.primitives.int32).map_err(|err| {
                    HashSet::from([err
                        .at(Some(shape.location))
                        .to_display(self.unifier)
                        .to_string()])
                })?;

                // Special handling for (1. A python `List` (all `int32s`)).
                // Read the doc above this function to see what is going on here.
                if let ExprKind::List { elts, .. } = &shape.node {
                    // The user wrote a List literal as the input argument
                    elts.len() as u64
                } else {
                    // This means the user is passing an expression of type `List`,
                    // but it is done so indirectly (like putting a variable referencing a `List`)
                    // rather than writing a List literal. We need to report an error.
                    return Err(HashSet::from([
                        format!(
                            "Expected list literal, tuple, or int32 for argument {arg_num} of {id} at {location}. Input argument is of type list but not a list literal.",
                            arg_num = arg_index + 1,
                            location = shape.location
                        )
                    ]));
                }
            }
            TypeEnum::TTuple { ty: tuple_element_types } => {
                // Handle 2. A tuple of int32s

                // Typecheck
                // The expected type is just the tuple but with all its elements being int32.
                let expected_ty = self.unifier.add_ty(TypeEnum::TTuple {
                    ty: tuple_element_types.iter().map(|_| self.primitives.int32).collect_vec(),
                });
                self.unifier.unify(shape_ty, expected_ty).map_err(|err| {
                    HashSet::from([err
                        .at(Some(shape.location))
                        .to_display(self.unifier)
                        .to_string()])
                })?;

                // `ndims` can be deduced statically from the inferred Tuple type.
                tuple_element_types.len() as u64
            }
            TypeEnum::TObj { .. } => {
                // Handle 3. An integer (generalized as [`TypeEnum::TObj`])

                // Typecheck
                self.unify(self.primitives.int32, shape_ty, &shape.location)?;

                // Deduce `ndims`
                1
            }
            _ => {
                // The user wrote an ill-typed `shape_expr`,
                // so throw an error.
                let shape_ty_str = self.unifier.stringify(shape_ty);
                return report_error(
                    format!(
                        "Expected list literal, tuple, or int32 for argument {arg_num} of {id}, got {shape_expr_name} of type {shape_ty_str}",
                        arg_num = arg_index + 1,
                        shape_expr_name = shape.node.name(),
                    )
                    .as_str(),
                    shape.location,
                );
            }
        };

        Ok((ndims, shape))
    }

    /// Tries to fold a special call. Returns [`Some`] if the call expression `func` is a special call, otherwise
    /// returns [`None`].
    fn try_fold_special_call(
        &mut self,
        location: Location,
        func: &ast::Expr<()>,
        args: &mut Vec<ast::Expr<()>>,
        keywords: &[Located<ast::KeywordData>],
    ) -> Result<Option<ast::Expr<Option<Type>>>, HashSet<String>> {
        let Located { location: func_location, node: ExprKind::Name { id, ctx }, .. } = func else {
            return Ok(None);
        };

        // handle special functions that cannot be typed in the usual way...
        if id == &"virtual".into() {
            if args.is_empty() || args.len() > 2 || !keywords.is_empty() {
                return report_error(
                    "`virtual` can only accept 1/2 positional arguments",
                    *func_location,
                );
            }
            let arg0 = self.fold_expr(args.remove(0))?;
            let ty = if let Some(arg) = args.pop() {
                let top_level_defs = self.top_level.definitions.read();
                self.function_data.resolver.parse_type_annotation(
                    top_level_defs.as_slice(),
                    self.unifier,
                    self.primitives,
                    &arg,
                )?
            } else {
                self.unifier.get_dummy_var().ty
            };
            self.virtual_checks.push((arg0.custom.unwrap(), ty, *func_location));
            let custom = Some(self.unifier.add_ty(TypeEnum::TVirtual { ty }));
            return Ok(Some(Located {
                location,
                custom,
                node: ExprKind::Call {
                    func: Box::new(Located {
                        custom: None,
                        location: func.location,
                        node: ExprKind::Name { id: *id, ctx: *ctx },
                    }),
                    args: vec![arg0],
                    keywords: vec![],
                },
            }));
        }

        if ["int32", "float", "bool", "round", "round64", "np_isnan", "np_isinf"]
            .iter()
            .any(|fun_id| id == &(*fun_id).into())
            && args.len() == 1
        {
            let target_ty = if id == &"int32".into()
                || id == &"round".into()
                || id == &"floor".into()
                || id == &"ceil".into()
            {
                self.primitives.int32
            } else if id == &"round64".into() || id == &"floor64".into() || id == &"ceil64".into() {
                self.primitives.int64
            } else if id == &"float".into() {
                self.primitives.float
            } else if id == &"bool".into() || id == &"np_isnan".into() || id == &"np_isinf".into() {
                self.primitives.bool
            } else {
                unreachable!()
            };

            let arg0 = self.fold_expr(args.remove(0))?;
            let arg0_ty = arg0.custom.unwrap();

            let ret = if arg0_ty.obj_id(self.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            {
                let (_, ndarray_ndims) = unpack_ndarray_var_tys(self.unifier, arg0_ty);

                make_ndarray_ty(self.unifier, self.primitives, Some(target_ty), Some(ndarray_ndims))
            } else {
                target_ty
            };

            let custom = self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg {
                    name: "n".into(),
                    ty: arg0.custom.unwrap(),
                    default_value: None,
                }],
                ret,
                vars: VarMap::new(),
            }));

            return Ok(Some(Located {
                location,
                custom: Some(ret),
                node: ExprKind::Call {
                    func: Box::new(Located {
                        custom: Some(custom),
                        location: func.location,
                        node: ExprKind::Name { id: *id, ctx: *ctx },
                    }),
                    args: vec![arg0],
                    keywords: vec![],
                },
            }));
        }

        if ["np_min", "np_max"].iter().any(|fun_id| id == &(*fun_id).into()) && args.len() == 1 {
            let arg0 = self.fold_expr(args.remove(0))?;
            let arg0_ty = arg0.custom.unwrap();

            let ret = if arg0_ty.obj_id(self.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            {
                let (ndarray_dtype, _) = unpack_ndarray_var_tys(self.unifier, arg0_ty);

                ndarray_dtype
            } else {
                arg0_ty
            };

            let custom = self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg {
                    name: "a".into(),
                    ty: arg0.custom.unwrap(),
                    default_value: None,
                }],
                ret,
                vars: VarMap::new(),
            }));

            return Ok(Some(Located {
                location,
                custom: Some(ret),
                node: ExprKind::Call {
                    func: Box::new(Located {
                        custom: Some(custom),
                        location: func.location,
                        node: ExprKind::Name { id: *id, ctx: *ctx },
                    }),
                    args: vec![arg0],
                    keywords: vec![],
                },
            }));
        }

        if [
            "np_minimum",
            "np_maximum",
            "np_arctan2",
            "np_copysign",
            "np_fmax",
            "np_fmin",
            "np_ldexp",
            "np_hypot",
            "np_nextafter",
        ]
        .iter()
        .any(|fun_id| id == &(*fun_id).into())
            && args.len() == 2
        {
            let arg0 = self.fold_expr(args.remove(0))?;
            let arg0_ty = arg0.custom.unwrap();
            let arg1 = self.fold_expr(args.remove(0))?;
            let arg1_ty = arg1.custom.unwrap();

            let arg0_dtype =
                if arg0_ty.obj_id(self.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) {
                    unpack_ndarray_var_tys(self.unifier, arg0_ty).0
                } else {
                    arg0_ty
                };

            let arg1_dtype =
                if arg1_ty.obj_id(self.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) {
                    unpack_ndarray_var_tys(self.unifier, arg1_ty).0
                } else {
                    arg1_ty
                };

            let expected_arg1_dtype =
                if id == &"np_ldexp".into() { self.primitives.int32 } else { arg0_dtype };
            if !self.unifier.unioned(arg1_dtype, expected_arg1_dtype) {
                return report_error(
                    format!(
                        "Expected broadcast-compatible type of ndarray[{}, N] for second argument of {id}, got {}",
                        self.unifier.stringify(expected_arg1_dtype),
                        self.unifier.stringify(arg1_dtype),
                    ).as_str(),
                    arg0.location,
                );
            }

            let target_ty = if id == &"np_minimum".into() || id == &"np_maximum".into() {
                arg0_dtype
            } else {
                self.primitives.float
            };

            let ret = if [&arg0_ty, &arg1_ty].into_iter().any(|arg_ty| {
                arg_ty.obj_id(self.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            }) {
                // typeof_ndarray_broadcast requires both dtypes to be the same, but ldexp accepts
                // (float, int32), so convert it to align with the dtype of the first arg
                let arg1_ty = if id == &"np_ldexp".into() {
                    if arg1_ty.obj_id(self.unifier).is_some_and(|id| id == PrimDef::NDArray.id()) {
                        let (_, ndims) = unpack_ndarray_var_tys(self.unifier, arg1_ty);

                        make_ndarray_ty(self.unifier, self.primitives, Some(target_ty), Some(ndims))
                    } else {
                        target_ty
                    }
                } else {
                    arg1_ty
                };

                match typeof_ndarray_broadcast(self.unifier, self.primitives, arg0_ty, arg1_ty) {
                    Ok(broadcasted_ty) => broadcasted_ty,
                    Err(err) => return report_error(err.as_str(), location),
                }
            } else {
                target_ty
            };

            let custom = self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![
                    FuncArg { name: "x1".into(), ty: arg0.custom.unwrap(), default_value: None },
                    FuncArg { name: "x2".into(), ty: arg1.custom.unwrap(), default_value: None },
                ],
                ret,
                vars: VarMap::new(),
            }));

            return Ok(Some(Located {
                location,
                custom: Some(ret),
                node: ExprKind::Call {
                    func: Box::new(Located {
                        custom: Some(custom),
                        location: func.location,
                        node: ExprKind::Name { id: *id, ctx: *ctx },
                    }),
                    args: vec![arg0, arg1],
                    keywords: vec![],
                },
            }));
        }

        // int64, uint32 and uint64 are special because their argument can be a constant outside the
        // range of int32s
        if ["int64", "uint32", "uint64"].iter().any(|fun_id| id == &(*fun_id).into())
            && args.len() == 1
        {
            let target_ty = if id == &"int64".into() {
                self.primitives.int64
            } else if id == &"uint32".into() {
                self.primitives.uint32
            } else if id == &"uint64".into() {
                self.primitives.uint64
            } else {
                unreachable!()
            };

            // Handle constants first to ensure that their types are not defaulted to int32, which
            // causes an "Integer out of bound" error
            if let ExprKind::Constant { value: ast::Constant::Int(val), kind } = &args[0].node {
                let conv_is_ok = if self.unifier.unioned(target_ty, self.primitives.int64) {
                    i64::try_from(*val).is_ok()
                } else if self.unifier.unioned(target_ty, self.primitives.uint32) {
                    u32::try_from(*val).is_ok()
                } else if self.unifier.unioned(target_ty, self.primitives.uint64) {
                    u64::try_from(*val).is_ok()
                } else {
                    unreachable!()
                };

                return if conv_is_ok {
                    Ok(Some(Located {
                        location: args[0].location,
                        custom: Some(target_ty),
                        node: ExprKind::Constant {
                            value: ast::Constant::Int(*val),
                            kind: kind.clone(),
                        },
                    }))
                } else {
                    report_error("Integer out of bound", args[0].location)
                };
            }

            let arg0 = self.fold_expr(args.remove(0))?;
            let arg0_ty = arg0.custom.unwrap();

            let ret = if arg0_ty.obj_id(self.unifier).is_some_and(|id| id == PrimDef::NDArray.id())
            {
                let (_, ndarray_ndims) = unpack_ndarray_var_tys(self.unifier, arg0_ty);

                make_ndarray_ty(self.unifier, self.primitives, Some(target_ty), Some(ndarray_ndims))
            } else {
                target_ty
            };

            let custom = self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg {
                    name: "n".into(),
                    ty: arg0.custom.unwrap(),
                    default_value: None,
                }],
                ret,
                vars: VarMap::new(),
            }));

            return Ok(Some(Located {
                location,
                custom: Some(ret),
                node: ExprKind::Call {
                    func: Box::new(Located {
                        custom: Some(custom),
                        location: func.location,
                        node: ExprKind::Name { id: *id, ctx: *ctx },
                    }),
                    args: vec![arg0],
                    keywords: vec![],
                },
            }));
        }

        // 1-argument ndarray n-dimensional factory functions
        if ["np_ndarray".into(), "np_empty".into(), "np_zeros".into(), "np_ones".into()]
            .contains(id)
            && args.len() == 1
        {
            let shape_expr = args.remove(0);
            let (ndims, shape) =
                self.fold_numpy_function_call_shape_argument(*id, 0, shape_expr)?; // Special handling for `shape`

            let ndims = self.unifier.get_fresh_literal(vec![SymbolValue::U64(ndims)], None);
            let ret = make_ndarray_ty(
                self.unifier,
                self.primitives,
                Some(self.primitives.float),
                Some(ndims),
            );
            let custom = self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![FuncArg {
                    name: "shape".into(),
                    ty: shape.custom.unwrap(),
                    default_value: None,
                }],
                ret,
                vars: VarMap::new(),
            }));

            return Ok(Some(Located {
                location,
                custom: Some(ret),
                node: ExprKind::Call {
                    func: Box::new(Located {
                        custom: Some(custom),
                        location: func.location,
                        node: ExprKind::Name { id: *id, ctx: *ctx },
                    }),
                    args: vec![shape],
                    keywords: vec![],
                },
            }));
        }

        // 2-argument ndarray n-dimensional creation functions
        if id == &"np_full".into() && args.len() == 2 {
            let ExprKind::List { elts, .. } = &args[0].node else {
                return report_error(
                    format!(
                        "Expected List literal for first argument of {id}, got {}",
                        args[0].node.name()
                    )
                    .as_str(),
                    args[0].location,
                );
            };

            let ndims = elts.len() as u64;

            let arg0 = self.fold_expr(args.remove(0))?;
            let arg1 = self.fold_expr(args.remove(0))?;

            let ty = arg1.custom.unwrap();
            let ndims = self.unifier.get_fresh_literal(vec![SymbolValue::U64(ndims)], None);
            let ret = make_ndarray_ty(self.unifier, self.primitives, Some(ty), Some(ndims));
            let custom = self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![
                    FuncArg { name: "shape".into(), ty: arg0.custom.unwrap(), default_value: None },
                    FuncArg {
                        name: "fill_value".into(),
                        ty: arg1.custom.unwrap(),
                        default_value: None,
                    },
                ],
                ret,
                vars: VarMap::new(),
            }));

            return Ok(Some(Located {
                location,
                custom: Some(ret),
                node: ExprKind::Call {
                    func: Box::new(Located {
                        custom: Some(custom),
                        location: func.location,
                        node: ExprKind::Name { id: *id, ctx: *ctx },
                    }),
                    args: vec![arg0, arg1],
                    keywords: vec![],
                },
            }));
        }

        // 1-argument ndarray n-dimensional creation functions
        if id == &"np_array".into() && args.len() == 1 {
            let arg0 = self.fold_expr(args.remove(0))?;

            let keywords = keywords
                .iter()
                .map(|v| fold::fold_keyword(self, v.clone()))
                .collect::<Result<Vec<_>, _>>()?;
            let ndmin_kw =
                keywords.iter().find(|kwarg| kwarg.node.arg.is_some_and(|id| id == "ndmin".into()));

            let ty = arraylike_flatten_element_type(self.unifier, arg0.custom.unwrap());
            let ndims = if let Some(ndmin_kw) = ndmin_kw {
                match &ndmin_kw.node.value.node {
                    ExprKind::Constant { value, .. } => match value {
                        ast::Constant::Int(value) => *value as u64,
                        _ => return Err(HashSet::from(["Expected uint64 for ndims".to_string()])),
                    },

                    _ => arraylike_get_ndims(self.unifier, arg0.custom.unwrap()),
                }
            } else {
                arraylike_get_ndims(self.unifier, arg0.custom.unwrap())
            };
            let ndims = self.unifier.get_fresh_literal(vec![SymbolValue::U64(ndims)], None);
            let ret = make_ndarray_ty(self.unifier, self.primitives, Some(ty), Some(ndims));

            let custom = self.unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: vec![
                    FuncArg {
                        name: "object".into(),
                        ty: arg0.custom.unwrap(),
                        default_value: None,
                    },
                    FuncArg {
                        name: "copy".into(),
                        ty: self.primitives.bool,
                        default_value: Some(SymbolValue::Bool(true)),
                    },
                    FuncArg {
                        name: "ndmin".into(),
                        ty: self.primitives.int32,
                        default_value: Some(SymbolValue::U32(0)),
                    },
                ],
                ret,
                vars: VarMap::new(),
            }));

            return Ok(Some(Located {
                location,
                custom: Some(ret),
                node: ExprKind::Call {
                    func: Box::new(Located {
                        custom: Some(custom),
                        location: func.location,
                        node: ExprKind::Name { id: *id, ctx: *ctx },
                    }),
                    args: vec![arg0],
                    keywords,
                },
            }));
        }

        Ok(None)
    }

    fn fold_call(
        &mut self,
        location: Location,
        func: ast::Expr<()>,
        mut args: Vec<ast::Expr<()>>,
        keywords: Vec<Located<ast::KeywordData>>,
    ) -> Result<ast::Expr<Option<Type>>, HashSet<String>> {
        if let Some(spec_call_func) =
            self.try_fold_special_call(location, &func, &mut args, &keywords)?
        {
            return Ok(spec_call_func);
        }

        let func = Box::new(self.fold_expr(func)?);
        let args = args.into_iter().map(|v| self.fold_expr(v)).collect::<Result<Vec<_>, _>>()?;
        let keywords = keywords
            .into_iter()
            .map(|v| fold::fold_keyword(self, v))
            .collect::<Result<Vec<_>, _>>()?;

        if let TypeEnum::TFunc(sign) = &*self.unifier.get_ty(func.custom.unwrap()) {
            if sign.vars.is_empty() {
                let call = Call {
                    posargs: args.iter().map(|v| v.custom.unwrap()).collect(),
                    kwargs: keywords
                        .iter()
                        .map(|v| (*v.node.arg.as_ref().unwrap(), v.node.value.custom.unwrap()))
                        .collect(),
                    fun: RefCell::new(None),
                    ret: sign.ret,
                    loc: Some(location),
                };
                self.unifier.unify_call(&call, func.custom.unwrap(), sign).map_err(|e| {
                    HashSet::from([e.at(Some(location)).to_display(self.unifier).to_string()])
                })?;
                return Ok(Located {
                    location,
                    custom: Some(sign.ret),
                    node: ExprKind::Call { func, args, keywords },
                });
            }
        }

        let ret = self.unifier.get_dummy_var().ty;
        let call = self.unifier.add_call(Call {
            posargs: args.iter().map(|v| v.custom.unwrap()).collect(),
            kwargs: keywords
                .iter()
                .map(|v| (*v.node.arg.as_ref().unwrap(), v.custom.unwrap()))
                .collect(),
            fun: RefCell::new(None),
            ret,
            loc: Some(location),
        });
        self.calls.insert(location.into(), call);
        let call = self.unifier.add_ty(TypeEnum::TCall(vec![call]));
        self.unify(func.custom.unwrap(), call, &func.location)?;

        Ok(Located { location, custom: Some(ret), node: ExprKind::Call { func, args, keywords } })
    }

    #[allow(clippy::unnecessary_wraps)]
    fn infer_identifier(&mut self, id: StrRef) -> InferenceResult {
        Ok(if let Some(ty) = self.variable_mapping.get(&id) {
            *ty
        } else {
            let variable_mapping = &mut self.variable_mapping;
            let unifier: &mut Unifier = self.unifier;
            self.function_data
                .resolver
                .get_symbol_type(unifier, &self.top_level.definitions.read(), self.primitives, id)
                .unwrap_or_else(|_| {
                    let ty = unifier.get_dummy_var().ty;
                    variable_mapping.insert(id, ty);
                    ty
                })
        })
    }

    fn infer_constant(&mut self, constant: &ast::Constant, loc: &Location) -> InferenceResult {
        match constant {
            ast::Constant::Bool(_) => Ok(self.primitives.bool),
            ast::Constant::Int(val) => {
                let int32: Result<i32, _> = (*val).try_into();
                // int64 and unsigned integers are handled separately in functions
                if int32.is_ok() {
                    Ok(self.primitives.int32)
                } else {
                    report_error("Integer out of bound", *loc)
                }
            }
            ast::Constant::Float(_) => Ok(self.primitives.float),
            ast::Constant::Tuple(vals) => {
                let ty: Result<Vec<_>, _> =
                    vals.iter().map(|x| self.infer_constant(x, loc)).collect();
                Ok(self.unifier.add_ty(TypeEnum::TTuple { ty: ty? }))
            }
            ast::Constant::Str(_) => Ok(self.primitives.str),
            ast::Constant::None => {
                report_error("CPython `None` not supported (nac3 uses `none` instead)", *loc)
            }
            ast::Constant::Ellipsis => Ok(self.unifier.get_fresh_var(None, None).ty),
            _ => report_error("not supported", *loc),
        }
    }

    fn infer_list(&mut self, elts: &[ast::Expr<Option<Type>>]) -> InferenceResult {
        let ty = self.unifier.get_dummy_var().ty;
        for t in elts {
            self.unify(ty, t.custom.unwrap(), &t.location)?;
        }
        Ok(self.unifier.add_ty(TypeEnum::TList { ty }))
    }

    #[allow(clippy::unnecessary_wraps)]
    fn infer_tuple(&mut self, elts: &[ast::Expr<Option<Type>>]) -> InferenceResult {
        let ty = elts.iter().map(|x| x.custom.unwrap()).collect();
        Ok(self.unifier.add_ty(TypeEnum::TTuple { ty }))
    }

    /// Checks for non-class attributes
    fn infer_general_attribute(
        &mut self,
        value: &ast::Expr<Option<Type>>,
        attr: StrRef,
        ctx: ExprContext,
    ) -> InferenceResult {
        let attr_ty = self.unifier.get_dummy_var().ty;
        let fields = once((
            attr.into(),
            RecordField::new(attr_ty, ctx == ExprContext::Store, Some(value.location)),
        ))
        .collect();
        let record = self.unifier.add_record(fields);
        self.constrain(value.custom.unwrap(), record, &value.location)?;
        Ok(attr_ty)
    }

    fn infer_attribute(
        &mut self,
        value: &ast::Expr<Option<Type>>,
        attr: StrRef,
        ctx: ExprContext,
    ) -> InferenceResult {
        let ty = value.custom.unwrap();
        if let TypeEnum::TObj { obj_id, fields, .. } = &*self.unifier.get_ty(ty) {
            // just a fast path
            match (fields.get(&attr), ctx == ExprContext::Store) {
                (Some((ty, true)), _) | (Some((ty, false)), false) => Ok(*ty),
                (Some((_, false)), true) => {
                    report_error(&format!("Field `{attr}` is immutable"), value.location)
                }
                (None, mutable) => {
                    // Check whether it is a class attribute
                    let defs = self.top_level.definitions.read();
                    let result = {
                        if let TopLevelDef::Class { attributes, .. } = &*defs[obj_id.0].read() {
                            attributes.iter().find_map(|f| {
                                if f.0 == attr {
                                    return Some(f.1);
                                }
                                None
                            })
                        } else {
                            None
                        }
                    };
                    match result {
                        Some(res) if !mutable => Ok(res),
                        Some(_) => report_error(
                            &format!("Class Attribute `{attr}` is immutable"),
                            value.location,
                        ),
                        None => {
                            let t = self.unifier.stringify(ty);
                            report_error(
                                &format!("`{t}::{attr}` field/method does not exist"),
                                value.location,
                            )
                        }
                    }
                }
            }
        } else if let TypeEnum::TFunc(sign) = &*self.unifier.get_ty(ty) {
            // Access Class Attributes of classes with __init__ function using Class names e.g. Foo.ATTR1
            let result = {
                self.top_level.definitions.read().iter().find_map(|def| {
                    if let Some(rear_guard) = def.try_read() {
                        if let TopLevelDef::Class { name, attributes, .. } = &*rear_guard {
                            if name.to_string() == self.unifier.stringify(sign.ret) {
                                return attributes.iter().find_map(|f| {
                                    if f.0 == attr {
                                        return Some(f.clone().1);
                                    }
                                    None
                                });
                            }
                        }
                    }
                    None
                })
            };
            match result {
                Some(f) if ctx != ExprContext::Store => Ok(f),
                Some(_) => {
                    report_error(&format!("Class Attribute `{attr}` is immutable"), value.location)
                }
                None => self.infer_general_attribute(value, attr, ctx),
            }
        } else {
            self.infer_general_attribute(value, attr, ctx)
        }
    }

    fn infer_bool_ops(&mut self, values: &[ast::Expr<Option<Type>>]) -> InferenceResult {
        let b = self.primitives.bool;
        for v in values {
            self.constrain(v.custom.unwrap(), b, &v.location)?;
        }
        Ok(b)
    }

    fn infer_bin_ops(
        &mut self,
        location: Location,
        left: &ast::Expr<Option<Type>>,
        op: ast::Operator,
        right: &ast::Expr<Option<Type>>,
        is_aug_assign: bool,
    ) -> InferenceResult {
        let left_ty = left.custom.unwrap();
        let right_ty = right.custom.unwrap();

        let method = if let TypeEnum::TObj { fields, .. } =
            self.unifier.get_ty_immutable(left_ty).as_ref()
        {
            let (binop_name, binop_assign_name) =
                (binop_name(op).into(), binop_assign_name(op).into());
            // if is aug_assign, try aug_assign operator first
            if is_aug_assign && fields.contains_key(&binop_assign_name) {
                binop_assign_name
            } else {
                binop_name
            }
        } else {
            binop_name(op).into()
        };

        let ret = if is_aug_assign {
            // The type of augmented assignment operator should never change
            Some(left_ty)
        } else {
            typeof_binop(self.unifier, self.primitives, op, left_ty, right_ty)
                .map_err(|e| HashSet::from([format!("{e} (at {location})")]))?
        };

        self.build_method_call(location, method, left_ty, vec![right_ty], ret)
    }

    fn infer_unary_ops(
        &mut self,
        location: Location,
        op: ast::Unaryop,
        operand: &ast::Expr<Option<Type>>,
    ) -> InferenceResult {
        let method = unaryop_name(op).into();

        let ret = typeof_unaryop(self.unifier, self.primitives, op, operand.custom.unwrap())
            .map_err(|e| HashSet::from([format!("{e} (at {location})")]))?;

        self.build_method_call(location, method, operand.custom.unwrap(), vec![], ret)
    }

    fn infer_compare(
        &mut self,
        location: Location,
        left: &ast::Expr<Option<Type>>,
        ops: &[ast::Cmpop],
        comparators: &[ast::Expr<Option<Type>>],
    ) -> InferenceResult {
        if ops.len() > 1
            && once(left).chain(comparators).any(|expr| {
                expr.custom
                    .unwrap()
                    .obj_id(self.unifier)
                    .is_some_and(|id| id == PrimDef::NDArray.id())
            })
        {
            return Err(HashSet::from([String::from(
                "Comparator chaining with ndarray types not supported",
            )]));
        }

        let mut res = None;
        for (a, b, c) in izip!(once(left).chain(comparators), comparators, ops) {
            let method = comparison_name(*c)
                .ok_or_else(|| HashSet::from(["unsupported comparator".to_string()]))?
                .into();

            let ret = typeof_cmpop(
                self.unifier,
                self.primitives,
                *c,
                a.custom.unwrap(),
                b.custom.unwrap(),
            )
            .map_err(|e| HashSet::from([format!("{e} (at {})", b.location)]))?;

            res.replace(self.build_method_call(
                location,
                method,
                a.custom.unwrap(),
                vec![b.custom.unwrap()],
                ret,
            )?);
        }

        Ok(res.unwrap())
    }

    /// Infers the type of a subscript expression on an `ndarray`.
    fn infer_subscript_ndarray(
        &mut self,
        value: &ast::Expr<Option<Type>>,
        slice: &ast::Expr<Option<Type>>,
        dummy_tvar: Type,
        ndims: Type,
    ) -> InferenceResult {
        debug_assert!(matches!(
            &*self.unifier.get_ty_immutable(dummy_tvar),
            TypeEnum::TVar { is_const_generic: false, .. }
        ));

        let constrained_ty =
            make_ndarray_ty(self.unifier, self.primitives, Some(dummy_tvar), Some(ndims));
        self.constrain(value.custom.unwrap(), constrained_ty, &value.location)?;

        let TypeEnum::TLiteral { values, .. } = &*self.unifier.get_ty_immutable(ndims) else {
            panic!("Expected TLiteral for ndarray.ndims, got {}", self.unifier.stringify(ndims))
        };

        let ndims = values
            .iter()
            .map(|ndim| u64::try_from(ndim.clone()).map_err(|()| ndim.clone()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|val| {
                HashSet::from([format!(
                    "Expected non-negative literal for ndarray.ndims, got {}",
                    i128::try_from(val).unwrap()
                )])
            })?;

        assert!(!ndims.is_empty());

        // The number of dimensions subscripted by the index expression.
        // Slicing a ndarray will yield the same number of dimensions, whereas indexing into a
        // dimension will remove a dimension.
        let subscripted_dims = match &slice.node {
            ExprKind::Tuple { elts, .. } => elts.iter().fold(0, |acc, value_subexpr| {
                if let ExprKind::Slice { .. } = &value_subexpr.node {
                    acc
                } else {
                    acc + 1
                }
            }),

            ExprKind::Slice { .. } => 0,
            _ => 1,
        };

        if ndims.len() == 1 && ndims[0] - subscripted_dims == 0 {
            // ndarray[T, Literal[1]] - Non-Slice index always returns an object of type T

            assert_ne!(ndims[0], 0);

            Ok(dummy_tvar)
        } else {
            // Otherwise - Index returns an object of type ndarray[T, Literal[N - subscripted_dims]]

            // Disallow subscripting if any Literal value will subscript on an element
            let new_ndims = ndims
                .into_iter()
                .map(|v| {
                    let v = i128::from(v) - i128::from(subscripted_dims);
                    u64::try_from(v)
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| {
                    HashSet::from([format!(
                        "Cannot subscript {} by {subscripted_dims} dimensions",
                        self.unifier.stringify(value.custom.unwrap()),
                    )])
                })?;

            if new_ndims.iter().any(|v| *v == 0) {
                unimplemented!("Inference for ndarray subscript operator with Literal[0, ...] bound unimplemented")
            }

            let ndims_ty = self
                .unifier
                .get_fresh_literal(new_ndims.into_iter().map(SymbolValue::U64).collect(), None);
            let subscripted_ty =
                make_ndarray_ty(self.unifier, self.primitives, Some(dummy_tvar), Some(ndims_ty));

            Ok(subscripted_ty)
        }
    }

    fn infer_subscript(
        &mut self,
        value: &ast::Expr<Option<Type>>,
        slice: &ast::Expr<Option<Type>>,
        ctx: ExprContext,
    ) -> InferenceResult {
        let report_unscriptable_error = |unifier: &mut Unifier| {
            // User is attempting to index into a value of an unsupported type.

            let value_ty = value.custom.unwrap();
            let value_ty_str = unifier.stringify(value_ty);

            return report_error(
                format!("'{value_ty_str}' object is not subscriptable").as_str(),
                slice.location, // using the slice's location (rather than value's) because it is more clear
            );
        };

        let ty = self.unifier.get_dummy_var().ty;
        match &slice.node {
            ExprKind::Slice { lower, upper, step } => {
                for v in [lower.as_ref(), upper.as_ref(), step.as_ref()].iter().flatten() {
                    self.constrain(v.custom.unwrap(), self.primitives.int32, &v.location)?;
                }
                let list_like_ty = match &*self.unifier.get_ty(value.custom.unwrap()) {
                    TypeEnum::TList { .. } => self.unifier.add_ty(TypeEnum::TList { ty }),
                    TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
                        let (_, ndims) =
                            unpack_ndarray_var_tys(self.unifier, value.custom.unwrap());

                        make_ndarray_ty(self.unifier, self.primitives, Some(ty), Some(ndims))
                    }

                    _ => {
                        return report_unscriptable_error(self.unifier);
                    }
                };
                self.constrain(value.custom.unwrap(), list_like_ty, &value.location)?;
                Ok(list_like_ty)
            }
            ExprKind::Constant { value: ast::Constant::Int(val), .. } => {
                match &*self.unifier.get_ty(value.custom.unwrap()) {
                    TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
                        let (_, ndims) =
                            unpack_ndarray_var_tys(self.unifier, value.custom.unwrap());
                        self.infer_subscript_ndarray(value, slice, ty, ndims)
                    }
                    _ => {
                        // the index is a constant, so value can be a sequence.
                        let ind: Option<i32> = (*val).try_into().ok();
                        let ind =
                            ind.ok_or_else(|| HashSet::from(["Index must be int32".to_string()]))?;
                        let map = once((
                            ind.into(),
                            RecordField::new(ty, ctx == ExprContext::Store, Some(value.location)),
                        ))
                        .collect();
                        let seq = self.unifier.add_record(map);
                        self.constrain(value.custom.unwrap(), seq, &value.location)?;
                        Ok(ty)
                    }
                }
            }
            ExprKind::Tuple { elts, .. } => {
                if value
                    .custom
                    .unwrap()
                    .obj_id(self.unifier)
                    .is_some_and(|id| id == PrimDef::NDArray.id())
                    .not()
                {
                    return report_error(
                        "Tuple slices are only supported for ndarrays",
                        slice.location,
                    );
                }

                for elt in elts {
                    if let ExprKind::Slice { lower, upper, step } = &elt.node {
                        for v in [lower.as_ref(), upper.as_ref(), step.as_ref()].iter().flatten() {
                            self.constrain(v.custom.unwrap(), self.primitives.int32, &v.location)?;
                        }
                    } else {
                        self.constrain(elt.custom.unwrap(), self.primitives.int32, &elt.location)?;
                    }
                }

                let (_, ndims) = unpack_ndarray_var_tys(self.unifier, value.custom.unwrap());
                self.infer_subscript_ndarray(value, slice, ty, ndims)
            }
            _ => {
                if let TypeEnum::TTuple { .. } = &*self.unifier.get_ty(value.custom.unwrap()) {
                    return report_error(
                        "Tuple index must be a constant (KernelInvariant is also not supported)",
                        slice.location,
                    );
                }

                // the index is not a constant, so value can only be a list-like structure
                match &*self.unifier.get_ty(value.custom.unwrap()) {
                    TypeEnum::TList { .. } => {
                        self.constrain(
                            slice.custom.unwrap(),
                            self.primitives.int32,
                            &slice.location,
                        )?;
                        let list = self.unifier.add_ty(TypeEnum::TList { ty });
                        self.constrain(value.custom.unwrap(), list, &value.location)?;
                        Ok(ty)
                    }
                    TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
                        let (_, ndims) =
                            unpack_ndarray_var_tys(self.unifier, value.custom.unwrap());

                        let valid_index_tys = [self.primitives.int32, self.primitives.isize()]
                            .into_iter()
                            .unique()
                            .collect_vec();
                        let valid_index_ty = self
                            .unifier
                            .get_fresh_var_with_range(valid_index_tys.as_slice(), None, None)
                            .ty;
                        self.constrain(slice.custom.unwrap(), valid_index_ty, &slice.location)?;
                        self.infer_subscript_ndarray(value, slice, ty, ndims)
                    }
                    _ => report_unscriptable_error(self.unifier),
                }
            }
        }
    }

    fn infer_if_expr(
        &mut self,
        test: &ast::Expr<Option<Type>>,
        body: &ast::Expr<Option<Type>>,
        orelse: &ast::Expr<Option<Type>>,
    ) -> InferenceResult {
        self.constrain(test.custom.unwrap(), self.primitives.bool, &test.location)?;
        self.constrain(body.custom.unwrap(), orelse.custom.unwrap(), &body.location)?;
        Ok(body.custom.unwrap())
    }
}
