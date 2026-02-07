use std::{collections::HashSet, iter::once};

use nac3parser::ast::{
    self, Constant, Expr, ExprKind,
    Operator::{LShift, RShift},
    Stmt, StmtKind, StrRef,
};

use crate::{
    toplevel::{composer::erase_expr_type, helper::PrimDef},
    typecheck::{
        type_error::get_expr_desc,
        type_inferencer::Inferencer,
        typedef::{Type, TypeEnum},
    },
};

impl Inferencer<'_> {
    fn should_have_value(&mut self, expr: &Expr<Option<Type>>) -> Result<(), HashSet<String>> {
        if matches!(expr.custom, Some(ty) if self.unifier.unioned(ty, self.primitives.none)) {
            Err(HashSet::from([format!("Error at {}: cannot have value none", expr.location)]))
        } else {
            Ok(())
        }
    }

    fn check_pattern(
        &mut self,
        pattern: &Expr<Option<Type>>,
        defined_identifiers: &mut HashSet<StrRef>,
    ) -> Result<(), HashSet<String>> {
        match &pattern.node {
            ExprKind::Name { id, .. } if id == &"none".into() => {
                Err(HashSet::from([format!("cannot assign to a `none` (at {})", pattern.location)]))
            }
            ExprKind::Name { id, .. } => {
                defined_identifiers.insert(*id);
                self.should_have_value(pattern)?;
                Ok(())
            }
            ExprKind::List { elts, .. } | ExprKind::Tuple { elts, .. } => {
                for elt in elts {
                    self.check_pattern(elt, defined_identifiers)?;
                    self.should_have_value(elt)?;
                }
                Ok(())
            }
            ExprKind::Starred { value, .. } => {
                self.check_pattern(value, defined_identifiers)?;
                self.should_have_value(value)?;
                Ok(())
            }
            ExprKind::Subscript { value, slice, .. } => {
                self.check_expr(value, defined_identifiers)?;
                self.should_have_value(value)?;
                self.check_expr(slice, defined_identifiers)?;
                if let TypeEnum::TTuple { .. } = &*self.unifier.get_ty(value.custom.unwrap()) {
                    return Err(HashSet::from([format!(
                        "Error at {}: cannot assign to tuple element",
                        value.location
                    )]));
                }
                Ok(())
            }
            ExprKind::Constant { .. } => Err(HashSet::from([format!(
                "cannot assign to a constant (at {})",
                pattern.location
            )])),
            _ => self.check_expr(pattern, defined_identifiers),
        }
    }

    fn check_expr(
        &mut self,
        expr: &Expr<Option<Type>>,
        defined_identifiers: &mut HashSet<StrRef>,
    ) -> Result<(), HashSet<String>> {
        // there are some cases where the custom field is None
        if let Some(ty) = &expr.custom
            && !matches!(&expr.node, ExprKind::Constant { value: Constant::Ellipsis, .. })
            && ty
                .obj_id(self.unifier)
                .is_none_or(|id| id != PrimDef::List.id() && id != PrimDef::Enumerate.id())
            && !self.unifier.is_concrete(*ty, &self.function_data.bound_variables)
        {
            let ty_enum = self.unifier.get_ty(*ty);
            let hint = match &*ty_enum {
                TypeEnum::TVar { name: Some(name), range, .. } => {
                    if range.is_empty() {
                        format!(
                            "Internal compiler error: The type variable `{name}` could not be resolved to a concrete type."
                        )
                    } else {
                        let range_types: Vec<_> =
                            range.iter().map(|t| self.unifier.stringify(*t)).collect();
                        format!(
                            "Internal compiler error: The type variable `{name}` must be one of [{}].",
                            range_types.join(", ")
                        )
                    }
                }
                _ => String::from(
                    "Internal compiler error: type not inferred during type_inference stage.",
                ),
            };

            let expr_desc = get_expr_desc(expr);
            return Err(HashSet::from([format!(
                "expected concrete type for {expr_desc} at {}, but type could not be inferred. {hint}",
                expr.location
            )]));
        }
        match &expr.node {
            ExprKind::Name { id, .. } => {
                if self.top_level.builtin_registry.match_builtin(&erase_expr_type(expr)).is_some() {
                    return Ok(());
                }
                self.should_have_value(expr)?;
                if defined_identifiers.insert(*id) {
                    let value = self.function_data.resolver.get_symbol_type(
                        self.unifier,
                        &self.top_level.definitions.read(),
                        self.primitives,
                        *id,
                    );
                    if let Err(e) = value {
                        defined_identifiers.remove(id);
                        return Err(HashSet::from([format!(
                            "type error at identifier `{id}` ({e}) at {}",
                            expr.location
                        )]));
                    }
                }
            }
            ExprKind::List { elts, .. }
            | ExprKind::Tuple { elts, .. }
            | ExprKind::BoolOp { values: elts, .. } => {
                for elt in elts {
                    self.check_expr(elt, defined_identifiers)?;
                    self.should_have_value(elt)?;
                }
            }
            ExprKind::Attribute { value, .. } => {
                self.check_expr(value, defined_identifiers)?;
                self.should_have_value(value)?;
            }
            ExprKind::BinOp { left, op, right } => {
                self.check_expr(left, defined_identifiers)?;
                self.check_expr(right, defined_identifiers)?;
                self.should_have_value(left)?;
                self.should_have_value(right)?;

                // Check whether a bitwise shift has a negative RHS constant value
                if (*op == LShift || *op == RShift)
                    && let ExprKind::Constant { value, .. } = &right.node
                {
                    let Constant::Int(rhs_val) = value else { unreachable!() };

                    if *rhs_val < 0 {
                        return Err(HashSet::from([format!(
                            "shift count is negative at {}",
                            right.location
                        )]));
                    }
                }
            }
            ExprKind::UnaryOp { operand, .. } => {
                self.check_expr(operand, defined_identifiers)?;
                self.should_have_value(operand)?;
            }
            ExprKind::Compare { left, comparators, .. } => {
                for elt in once(left.as_ref()).chain(comparators.iter()) {
                    self.check_expr(elt, defined_identifiers)?;
                    self.should_have_value(elt)?;
                }
            }
            ExprKind::Subscript { value, slice, .. } => {
                self.should_have_value(value)?;
                self.check_expr(value, defined_identifiers)?;
                self.check_expr(slice, defined_identifiers)?;
            }
            ExprKind::IfExp { test, body, orelse } => {
                self.check_expr(test, defined_identifiers)?;
                self.check_expr(body, defined_identifiers)?;
                self.check_expr(orelse, defined_identifiers)?;
            }
            ExprKind::Slice { lower, upper, step } => {
                for elt in [lower.as_ref(), upper.as_ref(), step.as_ref()].iter().flatten() {
                    self.should_have_value(elt)?;
                    self.check_expr(elt, defined_identifiers)?;
                }
            }
            ExprKind::Lambda { args, body } => {
                let mut defined_identifiers = defined_identifiers.clone();
                for arg in &args.args {
                    // TODO: should we check the types here?
                    defined_identifiers.insert(arg.node.arg);
                }
                self.check_expr(body, &mut defined_identifiers)?;
            }
            ExprKind::ListComp { elt, generators, .. } => {
                // in our type inference stage, we already make sure that there is only 1 generator
                let ast::Comprehension { target, iter, ifs, .. } = &generators[0];
                self.check_expr(iter, defined_identifiers)?;
                self.should_have_value(iter)?;
                let mut defined_identifiers = defined_identifiers.clone();
                self.check_pattern(target, &mut defined_identifiers)?;
                self.should_have_value(target)?;
                for term in once(elt.as_ref()).chain(ifs.iter()) {
                    self.check_expr(term, &mut defined_identifiers)?;
                    self.should_have_value(term)?;
                }
            }
            ExprKind::Call { func, args, keywords } => {
                for expr in once(func.as_ref())
                    .chain(args.iter())
                    .chain(keywords.iter().map(|v| v.node.value.as_ref()))
                {
                    self.check_expr(expr, defined_identifiers)?;
                    self.should_have_value(expr)?;
                }
            }
            ExprKind::Constant { .. } => {}
            _ => {
                unimplemented!()
            }
        }
        Ok(())
    }

    /// Check that the return value is a non-`alloca` type, effectively only allowing primitive types.
    ///
    /// This is a workaround preventing the caller from using a variable `alloca`-ed in the body, which
    /// is freed when the function returns.
    fn check_return_value_ty(&mut self, ret_ty: Type) -> bool {
        if cfg!(feature = "no-escape-analysis") {
            true
        } else {
            match &*self.unifier.get_ty_immutable(ret_ty) {
                TypeEnum::TObj { .. } => [
                    self.primitives.int32,
                    self.primitives.int64,
                    self.primitives.uint32,
                    self.primitives.uint64,
                    self.primitives.float,
                    self.primitives.bool,
                ]
                .iter()
                .any(|allowed_ty| self.unifier.unioned(ret_ty, *allowed_ty)),
                TypeEnum::TTuple { ty, .. } => ty.iter().all(|t| self.check_return_value_ty(*t)),
                _ => false,
            }
        }
    }

    // check statements for proper identifier def-use and return on all paths
    fn check_stmt(
        &mut self,
        stmt: &Stmt<Option<Type>>,
        defined_identifiers: &mut HashSet<StrRef>,
    ) -> Result<bool, HashSet<String>> {
        match &stmt.node {
            StmtKind::For { target, iter, body, orelse, .. } => {
                self.check_expr(iter, defined_identifiers)?;
                self.should_have_value(iter)?;
                let mut local_defined_identifiers = defined_identifiers.clone();
                for stmt in orelse {
                    self.check_stmt(stmt, &mut local_defined_identifiers)?;
                }
                let mut local_defined_identifiers = defined_identifiers.clone();
                self.check_pattern(target, &mut local_defined_identifiers)?;
                self.should_have_value(target)?;
                for stmt in body {
                    self.check_stmt(stmt, &mut local_defined_identifiers)?;
                }
                Ok(false)
            }
            StmtKind::If { test, body, orelse, .. } => {
                self.check_expr(test, defined_identifiers)?;
                self.should_have_value(test)?;
                let mut body_identifiers = defined_identifiers.clone();
                let mut orelse_identifiers = defined_identifiers.clone();
                let body_returned = self.check_block(body, &mut body_identifiers)?;
                let orelse_returned = self.check_block(orelse, &mut orelse_identifiers)?;

                for ident in &body_identifiers {
                    if orelse_identifiers.contains(ident) {
                        defined_identifiers.insert(*ident);
                    }
                }
                Ok(body_returned && orelse_returned)
            }
            StmtKind::While { test, body, orelse, .. } => {
                self.check_expr(test, defined_identifiers)?;
                self.should_have_value(test)?;
                let mut defined_identifiers = defined_identifiers.clone();
                self.check_block(body, &mut defined_identifiers)?;
                self.check_block(orelse, &mut defined_identifiers)?;
                Ok(false)
            }
            StmtKind::With { items, body, .. } => {
                let mut new_defined_identifiers = defined_identifiers.clone();
                for item in items {
                    self.check_expr(&item.context_expr, defined_identifiers)?;
                    if let Some(var) = item.optional_vars.as_ref() {
                        self.check_pattern(var, &mut new_defined_identifiers)?;
                    }
                }
                self.check_block(body, &mut new_defined_identifiers)?;
                Ok(false)
            }
            StmtKind::Try { body, handlers, orelse, finalbody, .. } => {
                self.check_block(body, &mut defined_identifiers.clone())?;
                self.check_block(orelse, &mut defined_identifiers.clone())?;
                for handler in handlers {
                    let mut defined_identifiers = defined_identifiers.clone();
                    let ast::ExcepthandlerKind::ExceptHandler { name, body, .. } = &handler.node;
                    if let Some(name) = name {
                        defined_identifiers.insert(*name);
                    }
                    self.check_block(body, &mut defined_identifiers)?;
                }
                self.check_block(finalbody, defined_identifiers)?;
                Ok(false)
            }
            StmtKind::Expr { value, .. } => {
                self.check_expr(value, defined_identifiers)?;
                Ok(false)
            }
            StmtKind::Assign { targets, value, .. } => {
                self.check_expr(value, defined_identifiers)?;
                self.should_have_value(value)?;
                for target in targets {
                    self.check_pattern(target, defined_identifiers)?;
                }
                Ok(false)
            }
            StmtKind::AnnAssign { target, value, .. } => {
                if let Some(value) = value {
                    self.check_expr(value, defined_identifiers)?;
                    self.should_have_value(value)?;
                    self.check_pattern(target, defined_identifiers)?;
                }
                Ok(false)
            }
            StmtKind::Return { value, .. } => {
                if let Some(value) = value {
                    self.check_expr(value, defined_identifiers)?;
                    self.should_have_value(value)?;

                    // Check that the return value is a non-`alloca` type, effectively only allowing primitive types.
                    // This is a workaround preventing the caller from using a variable `alloca`-ed in the body, which
                    // is freed when the function returns.
                    if let Some(ret_ty) = value.custom {
                        // Explicitly allow ellipsis as a return value, as the type of the ellipsis is contextually
                        // inferred and just generates an unconditional assertion
                        if matches!(
                            value.node,
                            ExprKind::Constant { value: Constant::Ellipsis, .. }
                        ) {
                            return Ok(true);
                        }

                        if !self.check_return_value_ty(ret_ty) {
                            return Err(HashSet::from([format!(
                                "return value of type {} must be a primitive or a tuple of primitives at {}",
                                self.unifier.stringify(ret_ty),
                                value.location,
                            )]));
                        }
                    }
                }
                Ok(true)
            }
            StmtKind::Raise { exc, .. } => {
                if let Some(value) = exc {
                    self.check_expr(value, defined_identifiers)?;
                }
                Ok(true)
            }
            StmtKind::Global { .. } => Err(HashSet::from([format!(
                "global statement is not supported at {}",
                stmt.location
            )])),
            // break, raise, etc.
            _ => Ok(false),
        }
    }

    pub fn check_block(
        &mut self,
        block: &[Stmt<Option<Type>>],
        defined_identifiers: &mut HashSet<StrRef>,
    ) -> Result<bool, HashSet<String>> {
        let mut ret = false;
        for stmt in block {
            if ret {
                eprintln!("warning: dead code at {}\n", stmt.location);
            }
            if self.check_stmt(stmt, defined_identifiers)? {
                ret = true;
            }
        }
        Ok(ret)
    }
}
