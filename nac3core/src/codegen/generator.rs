use inkwell::{
    types::{BasicTypeEnum, IntType},
    values::{BasicValueEnum, IntValue, PointerValue},
};

use nac3parser::ast::{Expr, Stmt, StrRef};

use super::{
    CodeGenContext, bool_to_int_type,
    expr::{RtValue, gen_call, gen_constructor, gen_expr, gen_func_instance},
    stmt::{
        gen_array_var, gen_assign, gen_assign_target_list, gen_block, gen_for, gen_if, gen_setitem,
        gen_stmt, gen_store_target, gen_var, gen_while, gen_with,
    },
    values::ArraySliceValue,
};
use crate::{
    symbol_resolver::ValueEnum,
    toplevel::{DefinitionId, TopLevelDef},
    typecheck::typedef::{FunSignature, Type},
};

pub trait CodeGenerator {
    /// Return the module name for the code generator.
    fn get_name(&self) -> &str;

    /// Generate function call and returns the function return value.
    /// - obj: Optional object for method call.
    /// - fun: Function signature and definition ID.
    /// - params: Function parameters. Note that this does not include the object even if the
    ///   function is a class method.
    fn gen_call<'ctx>(
        &mut self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        obj: Option<(Type, ValueEnum<'ctx>)>,
        fun: (&FunSignature, DefinitionId),
        params: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    ) -> Result<Option<BasicValueEnum<'ctx>>, String>
    where
        Self: Sized,
    {
        gen_call(self, ctx, obj, fun, params)
    }

    /// Generate object constructor and returns the constructed object.
    /// - signature: Function signature of the constructor.
    /// - def: Class definition for the constructor class.
    /// - params: Function parameters.
    fn gen_constructor<'ctx>(
        &mut self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        signature: &FunSignature,
        def: &TopLevelDef,
        params: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    ) -> Result<BasicValueEnum<'ctx>, String>
    where
        Self: Sized,
    {
        gen_constructor(self, ctx, signature, def, params)
    }

    /// Generate a function instance.
    /// - obj: Optional object for method call.
    /// - fun: Function signature, definition ID and the substitution key.
    /// - params: Function parameters. Note that this does not include the object even if the
    ///   function is a class method.
    ///
    /// Note that this function should check if the function is generated in another thread (due to
    /// possible race condition), see the default implementation for an example.
    fn gen_func_instance(
        &mut self,
        ctx: &mut CodeGenContext<'_, '_>,
        obj: Option<Type>,
        fun: (&FunSignature, &mut TopLevelDef, String),
        id: usize,
    ) -> Result<String, String> {
        gen_func_instance(ctx, obj, fun, id)
    }

    /// Generate the code for an expression.
    fn gen_expr<'ctx>(
        &mut self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        expr: &Expr<Option<Type>>,
    ) -> Result<RtValue<'ctx>, String>
    where
        Self: Sized,
    {
        gen_expr(self, ctx, expr)
    }

    /// Allocate memory for a variable and return a pointer pointing to it.
    /// The default implementation places the allocations at the start of the function.
    fn gen_var_alloc<'ctx>(
        &mut self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ty: BasicTypeEnum<'ctx>,
        name: Option<&str>,
    ) -> Result<PointerValue<'ctx>, String> {
        gen_var(ctx, ty, name)
    }

    /// Allocate memory for a variable and return a pointer pointing to it.
    /// The default implementation places the allocations at the start of the function.
    fn gen_array_var_alloc<'ctx>(
        &mut self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        ty: BasicTypeEnum<'ctx>,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> Result<ArraySliceValue<'ctx>, String> {
        gen_array_var(ctx, ty, size, name)
    }

    /// Return a pointer pointing to the target of the expression.
    fn gen_store_target<'ctx>(
        &mut self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        pattern: &Expr<Option<Type>>,
        name: Option<&str>,
    ) -> Result<Option<PointerValue<'ctx>>, String>
    where
        Self: Sized,
    {
        gen_store_target(self, ctx, pattern, name)
    }

    /// Generate code for an assignment expression.
    fn gen_assign<'ctx>(
        &mut self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        target: &Expr<Option<Type>>,
        value: &ValueEnum<'ctx>,
        value_ty: Type,
    ) -> Result<(), String>
    where
        Self: Sized,
    {
        gen_assign(self, ctx, target, value, value_ty)
    }

    /// Generate code for an assignment expression where LHS is a `"target_list"`.
    ///
    /// See <https://docs.python.org/3/reference/simple_stmts.html#assignment-statements>.
    fn gen_assign_target_list<'ctx>(
        &mut self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        targets: &[Expr<Option<Type>>],
        value: &ValueEnum<'ctx>,
        value_ty: Type,
    ) -> Result<(), String>
    where
        Self: Sized,
    {
        gen_assign_target_list(self, ctx, targets, value, value_ty)
    }

    /// Generate code for an item assignment.
    ///
    /// i.e., `target[key] = value`
    fn gen_setitem<'ctx>(
        &mut self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        target: &Expr<Option<Type>>,
        key: &Expr<Option<Type>>,
        value: &ValueEnum<'ctx>,
        value_ty: Type,
    ) -> Result<(), String>
    where
        Self: Sized,
    {
        gen_setitem(self, ctx, target, key, value, value_ty)
    }

    /// Generate code for a while expression.
    /// Return true if the while loop must early return
    fn gen_while(
        &mut self,
        ctx: &mut CodeGenContext<'_, '_>,
        stmt: &Stmt<Option<Type>>,
    ) -> Result<(), String>
    where
        Self: Sized,
    {
        gen_while(self, ctx, stmt)
    }

    /// Generate code for a for expression.
    /// Return true if the for loop must early return
    fn gen_for(
        &mut self,
        ctx: &mut CodeGenContext<'_, '_>,
        stmt: &Stmt<Option<Type>>,
    ) -> Result<(), String>
    where
        Self: Sized,
    {
        gen_for(self, ctx, stmt)
    }

    /// Generate code for an if expression.
    /// Return true if the statement must early return
    fn gen_if(
        &mut self,
        ctx: &mut CodeGenContext<'_, '_>,
        stmt: &Stmt<Option<Type>>,
    ) -> Result<(), String>
    where
        Self: Sized,
    {
        gen_if(self, ctx, stmt)
    }

    fn gen_with(
        &mut self,
        ctx: &mut CodeGenContext<'_, '_>,
        stmt: &Stmt<Option<Type>>,
    ) -> Result<(), String>
    where
        Self: Sized,
    {
        gen_with(self, ctx, stmt)
    }

    /// Generate code for a statement
    ///
    /// Return true if the statement must early return
    fn gen_stmt(
        &mut self,
        ctx: &mut CodeGenContext<'_, '_>,
        stmt: &Stmt<Option<Type>>,
    ) -> Result<(), String>
    where
        Self: Sized,
    {
        gen_stmt(self, ctx, stmt)
    }

    /// Generates code for a block statement.
    fn gen_block<'a, I: Iterator<Item = &'a Stmt<Option<Type>>>>(
        &mut self,
        ctx: &mut CodeGenContext<'_, '_>,
        stmts: I,
    ) -> Result<(), String>
    where
        Self: Sized,
    {
        gen_block(self, ctx, stmts)
    }

    /// Converts the value of a boolean-like value `bool_value` into an `i1`.
    fn bool_to_i1<'ctx>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        bool_value: IntValue<'ctx>,
    ) -> IntValue<'ctx> {
        self.bool_to_int_type(ctx, bool_value, ctx.i1)
    }

    /// Converts the value of a boolean-like value `bool_value` into an `i8`.
    fn bool_to_i8<'ctx>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        bool_value: IntValue<'ctx>,
    ) -> IntValue<'ctx> {
        self.bool_to_int_type(ctx, bool_value, ctx.i8)
    }

    /// See [`bool_to_int_type`].
    fn bool_to_int_type<'ctx>(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        bool_value: IntValue<'ctx>,
        ty: IntType<'ctx>,
    ) -> IntValue<'ctx> {
        bool_to_int_type(&ctx.builder, bool_value, ty)
    }
}

pub struct DefaultCodeGenerator {
    name: String,
}

impl DefaultCodeGenerator {
    #[must_use]
    pub fn new(name: String) -> DefaultCodeGenerator {
        DefaultCodeGenerator { name }
    }
}

impl CodeGenerator for DefaultCodeGenerator {
    fn get_name(&self) -> &str {
        &self.name
    }
}
