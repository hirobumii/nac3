use std::{collections::HashMap, fmt::Debug, sync::Arc};

use inkwell::values::BasicValueEnum;
use nac3parser::ast::{self, Location, Stmt, StrRef};
use parking_lot::RwLock;

use crate::{
    codegen::CodeGenContext,
    symbol_resolver::{SymbolResolver, ValueEnum},
    toplevel::composer::BuiltinRegistry,
    typecheck::{
        type_inferencer::{CodeLocation, PrimitiveStore},
        typedef::{CallId, FunSignature, SharedUnifier, Type, TypeVarId, VarMap},
    },
};

use type_annotation::{
    TypeAnnotation, check_overload_type_annotation_compatible, get_type_from_type_annotation_kinds,
    get_type_var_contained_in_type_annotation, make_self_type_annotation,
    parse_ast_to_type_annotation_kinds,
};

pub mod builtins;
pub mod composer;
pub mod helper;
pub mod numpy;
#[cfg(test)]
mod test;
pub mod type_annotation;

/// Index of a top-level definition (class, function, or module) in the global definition list.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, Debug)]
pub struct DefinitionId(pub usize);

type GenCallCallback = dyn for<'ctx, 'a> Fn(
        &mut CodeGenContext<'ctx, 'a>,
        Option<(Type, ValueEnum<'ctx>)>,
        (&FunSignature, DefinitionId),
        Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    ) -> anyhow::Result<Option<BasicValueEnum<'ctx>>>
    + Send
    + Sync;

/// A callback that overrides code generation for a specific function.
///
/// Used by frontends to implement custom calling conventions (e.g., RPC serialization in
/// nac3artiq) instead of generating a normal function call.
pub struct GenCall {
    fp: Box<GenCallCallback>,
}

impl GenCall {
    #[must_use]
    pub fn new(fp: Box<GenCallCallback>) -> Self {
        Self { fp }
    }

    /// Creates a dummy instance of [`GenCall`], which invokes [`unreachable!()`] with the given
    /// `reason`.
    #[must_use]
    pub fn create_dummy(reason: String) -> Self {
        Self::new(Box::new(move |_, _, _, _| unreachable!("{reason}")))
    }

    pub fn run<'ctx>(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        obj: Option<(Type, ValueEnum<'ctx>)>,
        fun: (&FunSignature, DefinitionId),
        args: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    ) -> anyhow::Result<Option<BasicValueEnum<'ctx>>> {
        (self.fp)(ctx, obj, fun, args)
    }
}

impl Debug for GenCall {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

/// A monomorphized instance of a generic function, containing the typed AST body and the
/// type variable substitutions for this particular instantiation.
#[derive(Clone, Debug)]
pub struct FunInstance {
    pub body: Arc<Vec<Stmt<Option<Type>>>>,
    pub calls: Arc<HashMap<CodeLocation, CallId>>,
    pub subst: VarMap,
    pub unifier_id: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunAttribute {
    StaticMethod,
}

/// A top-level definition: module, class, or function.
///
/// Definitions are stored in a global list and referenced by [`DefinitionId`]. During type
/// analysis, fields and method signatures are populated. During code generation, function
/// instances are created on demand as generic functions are called with concrete type arguments.
#[derive(Debug, Clone)]
pub enum TopLevelDef {
    Module {
        /// Name of the module
        name: StrRef,
        /// Simple name of the module.
        simple_name: String,
        /// Module ID used for [`TypeEnum`]
        module_id: DefinitionId,
        /// [`DefinitionId`] of [`TopLevelDef::Class`] within the module
        classes: Vec<(StrRef, DefinitionId)>,
        /// [`DefinitionId`] of [`TopLevelDef::Function`] within the module
        functions: Vec<(StrRef, DefinitionId)>,
        /// Symbol resolver of the module defined the class.
        resolver: Option<Arc<dyn SymbolResolver + Send + Sync>>,
        /// Definition location.
        loc: Option<Location>,
    },
    Class {
        /// Name for error messages and symbols.
        name: StrRef,
        /// Simple name of the module.
        simple_name: String,
        /// Object ID used for [`TypeEnum`].
        object_id: DefinitionId,
        /// type variables bounded to the class.
        type_vars: Vec<Type>,
        /// Class fields.
        ///
        /// Name, type, and whether the field is mutable.
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
        /// Attributes of the function, e.g. `@staticmethod`
        attributes: Vec<FunAttribute>,
        /// Function instance to symbol mapping
        ///
        /// * Key: String representation of type variable values, sorted by variable ID in ascending
        ///   order, including type variables associated with the class.
        /// * Value: Function symbol name.
        instance_to_symbol: HashMap<String, String>,
        /// Function instances to annotated AST mapping
        ///
        /// * Key: String representation of type variable values, sorted by variable ID in ascending
        ///   order, including type variables associated with the class. Excluding rigid type
        ///   variables.
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

/// Global compilation context shared across all codegen workers.
///
/// Contains the full list of top-level definitions, per-module unifiers, and the builtin
/// registry. Created by `TopLevelComposer::make_top_level_context()` after type analysis.
pub struct TopLevelContext {
    pub definitions: Arc<RwLock<Vec<Arc<RwLock<TopLevelDef>>>>>,
    pub unifiers: Arc<RwLock<Vec<(SharedUnifier, PrimitiveStore)>>>,
    pub personality_symbol: Option<String>,
    pub builtin_registry: Arc<dyn BuiltinRegistry>,
}
