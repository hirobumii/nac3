use std::{
    borrow::BorrowMut,
    collections::{HashMap, HashSet},
    fmt::Debug,
    iter::FromIterator,
    sync::Arc,
};

use super::codegen::CodeGenContext;
use super::typecheck::type_inferencer::PrimitiveStore;
use super::typecheck::typedef::{
    FunSignature, FuncArg, SharedUnifier, Type, TypeEnum, Unifier, VarMap,
};
use crate::{
    codegen::CodeGenerator,
    symbol_resolver::{SymbolResolver, ValueEnum},
    typecheck::{
        type_inferencer::CodeLocation,
        typedef::{CallId, TypeVarId},
    },
};
use inkwell::values::BasicValueEnum;
use itertools::Itertools;
use nac3parser::ast::{self, Location, Stmt, StrRef};
use parking_lot::RwLock;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, Debug)]
pub struct DefinitionId(pub usize);

pub mod builtins;
pub mod composer;
pub mod helper;
pub mod primitive_type;
pub mod type_annotation;
use composer::*;
use type_annotation::*;
#[cfg(test)]
mod test;

type GenCallCallback = dyn for<'ctx, 'a> Fn(
        &mut CodeGenContext<'ctx, 'a>,
        Option<(Type, ValueEnum<'ctx>)>,
        (&FunSignature, DefinitionId),
        Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
        &mut dyn CodeGenerator,
    ) -> Result<Option<BasicValueEnum<'ctx>>, String>
    + Send
    + Sync;

pub struct GenCall {
    fp: Box<GenCallCallback>,
}

impl GenCall {
    #[must_use]
    pub fn new(fp: Box<GenCallCallback>) -> GenCall {
        GenCall { fp }
    }

    /// Creates a dummy instance of [`GenCall`], which invokes [`unreachable!()`] with the given
    /// `reason`.
    #[must_use]
    pub fn create_dummy(reason: String) -> GenCall {
        Self::new(Box::new(move |_, _, _, _, _| unreachable!("{reason}")))
    }

    pub fn run<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        obj: Option<(Type, ValueEnum<'ctx>)>,
        fun: (&FunSignature, DefinitionId),
        args: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
        generator: &mut dyn CodeGenerator,
    ) -> Result<Option<BasicValueEnum<'ctx>>, String> {
        (self.fp)(ctx, obj, fun, args, generator)
    }
}

impl Debug for GenCall {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct FunInstance {
    pub body: Arc<Vec<Stmt<Option<Type>>>>,
    pub calls: Arc<HashMap<CodeLocation, CallId>>,
    pub subst: VarMap,
    pub unifier_id: usize,
}

#[derive(Debug, Clone)]
pub enum TopLevelDef {
    Class {
        /// Name for error messages and symbols.
        name: StrRef,
        /// Object ID used for [`TypeEnum`].
        object_id: DefinitionId,
        /// type variables bounded to the class.
        type_vars: Vec<Type>,
        /// Class fields.
        ///
        /// Name and type is mutable.
        fields: Vec<(StrRef, Type, bool)>,
        /// Class Attributes.
        ///
        /// Name, type, value.
        attributes: Vec<(StrRef, Type, ast::Constant)>,
        /// Class methods, pointing to the corresponding function definition.
        methods: Vec<(StrRef, Type, DefinitionId)>,
        /// Ancestor classes, including itself.
        ancestors: Vec<TypeAnnotation>,
        /// Symbol resolver of the module defined the class; [None] if it is built-in type.
        resolver: Option<Arc<dyn SymbolResolver + Send + Sync>>,
        /// Constructor type.
        constructor: Option<Type>,
        /// Definition location.
        loc: Option<Location>,
    },
    Function {
        /// Prefix for symbol, should be unique globally.
        name: String,
        /// Simple name, the same as in method/function definition.
        simple_name: StrRef,
        /// Function signature.
        signature: Type,
        /// Instantiated type variable IDs.
        var_id: Vec<TypeVarId>,
        /// Function instance to symbol mapping
        ///
        /// * Key: String representation of type variable values, sorted by variable ID in ascending
        /// order, including type variables associated with the class.
        /// * Value: Function symbol name.
        instance_to_symbol: HashMap<String, String>,
        /// Function instances to annotated AST mapping
        ///
        /// * Key: String representation of type variable values, sorted by variable ID in ascending
        /// order, including type variables associated with the class. Excluding rigid type
        /// variables.
        ///
        /// Rigid type variables that would be substituted when the function is instantiated.
        instance_to_stmt: HashMap<String, FunInstance>,
        /// Symbol resolver of the module defined the class.
        resolver: Option<Arc<dyn SymbolResolver + Send + Sync>>,
        /// Custom code generation callback.
        codegen_callback: Option<Arc<GenCall>>,
        /// Definition location.
        loc: Option<Location>,
    },
}

pub struct TopLevelContext {
    pub definitions: Arc<RwLock<Vec<Arc<RwLock<TopLevelDef>>>>>,
    pub unifiers: Arc<RwLock<Vec<(SharedUnifier, PrimitiveStore)>>>,
    pub personality_symbol: Option<String>,
}
