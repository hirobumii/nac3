use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};

use indexmap::IndexMap;
use itertools::Itertools;
use parking_lot::RwLock;

use nac3parser::ast::{self, Expr, ExprKind, Ident, Located, StrRef, fold::Fold};

use super::{
    DefinitionId, FunInstance, GenCall, Location, builtins, get_type_from_type_annotation_kinds,
    get_type_var_contained_in_type_annotation, make_self_type_annotation,
    parse_ast_to_type_annotation_kinds, type_annotation::TypeAnnotation,
};
use crate::{
    codegen::{expr::get_subst_key, stmt::exn_constructor},
    symbol_resolver::SymbolValue,
    toplevel::{Stmt, SymbolResolver, TopLevelContext, TopLevelDef},
    typecheck::{
        type_inferencer::{CodeLocation, FunctionData, IdentifierInfo, Inferencer, PrimitiveStore},
        typedef::{CallId, FunSignature, FuncArg, Type, TypeEnum, TypeVar, Unifier, VarMap},
    },
};

/// Function type to check if a (possibly nested) symbol is annotated with a given type.
type IsSymbolAnnotatedFn<'a> =
    dyn Fn(StrRef, Option<&TopLevelDef>, &Located<ExprKind>) -> Result<bool, String> + 'a;

/// Function type to check if a decorator references a specific decorator function.
type IsDecoratorFn<'a> = dyn Fn(&Located<ExprKind>) -> Result<bool, String> + 'a;

pub struct ComposerConfig<'a> {
    /// A function that checks whether a symbol is annotated with a class that indicates a class
    /// variable should be mutable, or [`None`] if such a class is not supported.
    ///
    /// See [`ComposerConfig::has_kernel_ann`].
    pub has_kernel_ann_fn: Option<Box<IsSymbolAnnotatedFn<'a>>>,

    /// A function that checks whether a symbol is annotated with a class that indicates a class
    /// variable should be immutable.
    ///
    /// See [`ComposerConfig::has_invariant_ann`].
    pub has_invariant_ann_fn: Box<IsSymbolAnnotatedFn<'a>>,

    /// A function that checks whether a decorator indicates that the function should be treated as
    /// `extern`, i.e. defined not as part of the compiled Python binary and hence does not contain
    /// a function body.
    ///
    /// See [`ComposerConfig::is_extern_decorator`].
    pub is_extern_decorator_fn: Box<IsDecoratorFn<'a>>,
}

impl ComposerConfig<'_> {
    /// Checks whether the attribute with `attr_name` with type hint `type_ann` is annotated with a
    /// class that indicates a variable should be located on-device and mutable, usually
    /// `Kernel[T]`.
    ///
    /// The `attr_name` is resolved within the class `class_ctx` if it is provided, otherwise it is
    /// resolved in module context.
    ///
    /// Returns `Ok(None)` if this functionality is not supported.
    pub fn has_kernel_ann(
        &self,
        attr_name: StrRef,
        class_ctx: Option<&TopLevelDef>,
        type_ann: &Located<ExprKind>,
    ) -> Result<Option<bool>, String> {
        self.has_kernel_ann_fn.as_ref().map(|f| f(attr_name, class_ctx, type_ann)).transpose()
    }

    /// Checks whether the attribute with `attr_name` with type hint `type_ann` is annotated with a
    /// class that indicates a variable should be located on-device and immutable, usually
    /// `KernelInvariant[T]`.
    ///
    /// The `attr_name` is resolved within the class `class_ctx` if it is provided, otherwise it is
    /// resolved in module context.
    pub fn has_invariant_ann(
        &self,
        attr_name: StrRef,
        class_ctx: Option<&TopLevelDef>,
        type_ann: &Located<ExprKind>,
    ) -> Result<bool, String> {
        (*self.has_invariant_ann_fn)(attr_name, class_ctx, type_ann)
    }

    /// Checks whether the `decorator` indicates that the function should be an `extern` function,
    /// usually `@extern`.
    ///
    /// An `extern` function is a function that is only declared in the compiled Python binary and
    /// whose implementation is defined elsewhere, such as compiler builtins or functions that are
    /// executed on the host interpreter.
    pub fn is_extern_decorator(&self, decorator: &Located<ExprKind>) -> Result<bool, String> {
        (*self.is_extern_decorator_fn)(decorator)
    }
}

impl Default for ComposerConfig<'_> {
    fn default() -> Self {
        ComposerConfig {
            has_kernel_ann_fn: None,
            has_invariant_ann_fn: Box::new(|_, _, ann| {
                Ok(matches!(
                    &ann.node,
                    ExprKind::Subscript { value, .. } if matches!(
                        &value.node,
                        ExprKind::Name { id, .. } if id == &"KernelInvariant".into()
                    )
                ))
            }),
            is_extern_decorator_fn: Box::new(|decorator| {
                Ok(matches!(
                    &decorator.node,
                    ExprKind::Name { id, .. } if id == &"extern".into()
                ))
            }),
        }
    }
}

pub type DefAst = (Arc<RwLock<TopLevelDef>>, Option<Stmt<()>>);
pub struct TopLevelComposer<'a> {
    // list of top level definitions, same as top level context
    pub definition_ast_list: Vec<DefAst>,
    // start as a primitive unifier, will add more top_level defs inside
    pub unifier: Unifier,
    // primitive store
    pub primitives_ty: PrimitiveStore,
    // keyword list to prevent same user-defined name
    pub keyword_list: HashSet<StrRef>,
    // to prevent duplicate definition
    pub defined_names: HashSet<String>,
    // get the class def id of a class method
    pub method_class: HashMap<DefinitionId, DefinitionId>,
    // number of built-in function and classes in the definition list, later skip
    pub builtin_num: usize,
    pub core_config: ComposerConfig<'a>,
    /// The size of a native word on the target platform.
    pub size_t: u32,
}

/// The specification for a builtin function, consisting of the function name, the function
/// signature, and a [code generation callback][`GenCall`].
pub type BuiltinFuncSpec = (StrRef, FunSignature, Arc<GenCall>);

/// A function that creates a [`BuiltinFuncSpec`] using the provided [`PrimitiveStore`] and
/// [`Unifier`].
pub type BuiltinFuncCreator = dyn Fn(&PrimitiveStore, &mut Unifier) -> BuiltinFuncSpec;

impl<'a> TopLevelComposer<'a> {
    /// return a composer and things to make a "primitive" symbol resolver, so that the symbol
    /// resolver can later figure out primitive tye definitions when passed a primitive type name
    ///
    /// `lateinit_builtins` are specifically for the ARTIQ module. Since the [`Unifier`] instance
    /// used to create builtin functions do not persist until method compilation, any types
    /// created (e.g. [`TypeEnum::TVar`]) also do not persist. Those functions should be instead put
    /// in `lateinit_builtins`, where they will be instantiated with the [`Unifier`] instance used
    /// for method compilation.
    #[must_use]
    pub fn new(
        builtins: Vec<BuiltinFuncSpec>,
        lateinit_builtins: Vec<Box<BuiltinFuncCreator>>,
        core_config: ComposerConfig<'a>,
        size_t: u32,
    ) -> (Self, HashMap<StrRef, DefinitionId>, HashMap<StrRef, Type>) {
        let (primitives_ty, mut unifier) = Self::make_primitives(size_t);
        let mut definition_ast_list = builtins::get_builtins(&mut unifier, &primitives_ty);
        let mut keyword_list: HashSet<StrRef> = HashSet::from_iter(vec![
            "Generic".into(),
            "virtual".into(),
            "list".into(),
            "tuple".into(),
            "int32".into(),
            "int64".into(),
            "uint32".into(),
            "uint64".into(),
            "float".into(),
            "bool".into(),
            "none".into(),
            "None".into(),
            "range".into(),
            "str".into(),
            "self".into(),
            "Kernel".into(),
            "KernelInvariant".into(),
            "Some".into(),
            "Option".into(),
        ]);
        let defined_names = HashSet::default();
        let method_class = HashMap::default();

        let mut builtin_id = HashMap::default();
        let mut builtin_ty = HashMap::default();

        let builtin_name_list = definition_ast_list
            .iter()
            .map(|def_ast| match *def_ast.0.read() {
                TopLevelDef::Class { name, .. } | TopLevelDef::Module { name, .. } => {
                    name.to_string()
                }
                TopLevelDef::Function { simple_name, .. }
                | TopLevelDef::Variable { simple_name, .. } => simple_name.to_string(),
            })
            .collect_vec();

        for (id, name) in builtin_name_list.iter().enumerate() {
            let name = (**name).into();
            let def = definition_ast_list[id].0.read();
            if let TopLevelDef::Function { name: func_name, simple_name, signature, .. } = &*def {
                assert_eq!(
                    name, *simple_name,
                    "Simple name of builtin function should match builtin name list"
                );

                // Do not add member functions into the list of builtin IDs;
                // Here we assume that all builtin top-level functions have the same name and simple
                // name, and all member functions have something prefixed to its name
                if *func_name != simple_name.to_string() {
                    continue;
                }
                builtin_ty.insert(name, *signature);
                builtin_id.insert(name, DefinitionId(id));
            } else if let TopLevelDef::Class { name, constructor, object_id, .. } = &*def {
                assert_eq!(
                    id, object_id.0,
                    "Object id of class '{name}' should match its index in builtin name list"
                );
                if let Some(constructor) = constructor {
                    builtin_ty.insert(*name, *constructor);
                }
                builtin_id.insert(*name, DefinitionId(id));
            }
        }

        // Materialize lateinit_builtins, now that the unifier is ready
        let lateinit_builtins = lateinit_builtins
            .into_iter()
            .map(|builtin| builtin(&primitives_ty, &mut unifier))
            .collect_vec();

        for (name, sig, codegen_callback) in builtins.into_iter().chain(lateinit_builtins) {
            let fun_sig = unifier.add_ty(TypeEnum::TFunc(sig));
            builtin_ty.insert(name, fun_sig);
            builtin_id.insert(name, DefinitionId(definition_ast_list.len()));
            definition_ast_list.push((
                Arc::new(RwLock::new(TopLevelDef::Function {
                    name: name.into(),
                    simple_name: name,
                    signature: fun_sig,
                    instance_to_stmt: HashMap::default(),
                    instance_to_symbol: HashMap::default(),
                    var_id: Vec::default(),
                    resolver: None,
                    codegen_callback: Some(codegen_callback),
                    loc: None,
                })),
                None,
            ));
            keyword_list.insert(name);
        }

        (
            TopLevelComposer {
                builtin_num: definition_ast_list.len(),
                definition_ast_list,
                primitives_ty,
                unifier,
                keyword_list,
                defined_names,
                method_class,
                core_config,
                size_t,
            },
            builtin_id,
            builtin_ty,
        )
    }

    #[must_use]
    pub fn make_top_level_context(&self) -> TopLevelContext {
        TopLevelContext {
            definitions: RwLock::new(
                self.definition_ast_list.iter().map(|(x, ..)| x.clone()).collect_vec(),
            )
            .into(),
            // NOTE: only one for now
            unifiers: Arc::new(RwLock::new(vec![(
                self.unifier.get_shared_unifier(),
                self.primitives_ty,
            )])),
            personality_symbol: Some("__nac3_personality".into()),
        }
    }

    #[must_use]
    pub fn extract_def_list(&self) -> Vec<Arc<RwLock<TopLevelDef>>> {
        self.definition_ast_list.iter().map(|(def, ..)| def.clone()).collect_vec()
    }

    /// register top level modules
    pub fn register_top_level_module(
        &mut self,
        module_name: &str,
        name_to_pyid: &Rc<HashMap<StrRef, u64>>,
        resolver: Arc<dyn SymbolResolver + Send + Sync>,
        location: Option<Location>,
    ) -> Result<DefinitionId, String> {
        let mut classes: HashMap<StrRef, DefinitionId> = HashMap::new();
        let mut methods: HashMap<StrRef, DefinitionId> = HashMap::new();
        let mut attributes: Vec<(StrRef, DefinitionId)> = Vec::new();

        for (name, _) in name_to_pyid.iter() {
            if let Ok(def_id) = resolver.get_identifier_def(*name) {
                // Avoid repeated attribute instances resulting from multiple imports of same module
                if self.defined_names.contains(&format!("{module_name}.{name}")) {
                    match &*self.definition_ast_list[def_id.0].0.read() {
                        TopLevelDef::Class { .. } => {
                            classes.insert(*name, def_id);
                        }
                        TopLevelDef::Function { .. } => {
                            methods.insert(*name, def_id);
                        }
                        _ => attributes.push((*name, def_id)),
                    }
                }
            };
        }
        let module_def = TopLevelDef::Module {
            name: module_name.to_string().into(),
            simple_name: module_name
                .rsplit_once('.')
                .map_or(module_name, |(_, nme)| nme)
                .to_string(),
            module_id: DefinitionId(self.definition_ast_list.len()),
            classes: classes.into_iter().collect(),
            functions: methods.into_iter().collect(),
            attributes,
            resolver: Some(resolver),
            loc: location,
        };

        self.definition_ast_list.push((Arc::new(RwLock::new(module_def)), None));
        Ok(DefinitionId(self.definition_ast_list.len() - 1))
    }

    /// register, just remember the names of top level classes/function
    /// and check duplicate class/method/function definition
    pub fn register_top_level(
        &mut self,
        ast: Stmt<()>,
        resolver: Option<Arc<dyn SymbolResolver + Send + Sync>>,
        mod_path: &str,
        allow_no_constructor: bool,
    ) -> Result<(StrRef, DefinitionId, Option<Type>), String> {
        type MethodInfo = (
            // the simple method name without class name
            StrRef,
            // in this top level def, method name is prefixed with the class name
            Arc<RwLock<TopLevelDef>>,
            DefinitionId,
            Type,
            Stmt<()>,
        );

        let defined_names = &mut self.defined_names;
        match &ast.node {
            ast::StmtKind::ClassDef { name: class_name, bases, body, .. } => {
                if self.keyword_list.contains(class_name) {
                    return Err(format!(
                        "cannot use keyword `{}` as a class name (at {})",
                        class_name, ast.location
                    ));
                }
                let fully_qualified_class_name = if mod_path.is_empty() {
                    *class_name
                } else {
                    format!("{}.{}", &mod_path, class_name).into()
                };
                if !defined_names.insert(fully_qualified_class_name.into()) {
                    return Err(format!(
                        "duplicate definition of class `{}` (at {})",
                        class_name, ast.location
                    ));
                }

                let class_name = *class_name;
                let class_def_id = self.definition_ast_list.len();

                // since later when registering class method, ast will still be used,
                // here push None temporarily, later will move the ast inside
                let constructor_ty = self.unifier.get_dummy_var().ty;
                let mut class_def_ast = (
                    Arc::new(RwLock::new(Self::make_top_level_class_def(
                        DefinitionId(class_def_id),
                        resolver.clone(),
                        fully_qualified_class_name,
                        Some(constructor_ty),
                        Some(ast.location),
                    ))),
                    None,
                );

                // parse class def body and register class methods into the def list.
                // module's symbol resolver would not know the name of the class methods,
                // thus cannot return their definition_id
                let mut class_method_name_def_ids: Vec<MethodInfo> = Vec::new();
                // we do not push anything to the def list, so we keep track of the index
                // and then push in the correct order after the for loop
                let mut class_method_index_offset = 0;
                let init_id = "__init__".into();
                let exception_id = "Exception".into();
                // TODO: Fix this hack. We will generate constructor for classes that inherit
                // from Exception class (directly or indirectly), but this code cannot handle
                // subclass of other exception classes.
                let mut contains_constructor = bases
                    .iter().any(|base| matches!(base.node, ast::ExprKind::Name { id, .. } if id == exception_id));
                for b in body {
                    if let ast::StmtKind::FunctionDef { name: method_name, .. } = &b.node {
                        if method_name == &init_id {
                            contains_constructor = true;
                        }
                        if self.keyword_list.contains(method_name) {
                            return Err(format!(
                                "cannot use keyword `{}` as a method name (at {})",
                                method_name, b.location
                            ));
                        }
                        let global_class_method_name = Self::make_class_method_name(
                            fully_qualified_class_name.into(),
                            &method_name.to_string(),
                        );
                        if !defined_names.insert(global_class_method_name.clone()) {
                            return Err(format!(
                                "class method `{}` defined twice (at {})",
                                global_class_method_name, b.location
                            ));
                        }
                        let method_def_id = self.definition_ast_list.len() + {
                            // plus 1 here since we already have the class def
                            class_method_index_offset += 1;
                            class_method_index_offset
                        };

                        // dummy method define here
                        let dummy_method_type = self.unifier.get_dummy_var().ty;
                        class_method_name_def_ids.push((
                            *method_name,
                            RwLock::new(Self::make_top_level_function_def(
                                global_class_method_name,
                                *method_name,
                                // later unify with parsed type
                                dummy_method_type,
                                resolver.clone(),
                                Some(b.location),
                            ))
                            .into(),
                            DefinitionId(method_def_id),
                            dummy_method_type,
                            b.clone(),
                        ));
                    }
                }

                // move the ast to the entry of the class in the ast_list
                class_def_ast.1 = Some(ast);
                // get the methods into the top level class_def
                for (name, _, id, ty, ..) in &class_method_name_def_ids {
                    let mut class_def = class_def_ast.0.write();
                    let TopLevelDef::Class { methods, .. } = &mut *class_def else {
                        unreachable!()
                    };

                    methods.push((*name, *ty, *id));
                    self.method_class.insert(*id, DefinitionId(class_def_id));
                }
                // now class_def_ast and class_method_def_ast_ids are ok, put them into actual def list in correct order
                self.definition_ast_list.push(class_def_ast);
                for (_, def, _, _, ast) in class_method_name_def_ids {
                    self.definition_ast_list.push((def, Some(ast)));
                }

                let result_ty = if allow_no_constructor || contains_constructor {
                    Some(constructor_ty)
                } else {
                    None
                };
                Ok((class_name, DefinitionId(class_def_id), result_ty))
            }

            ast::StmtKind::FunctionDef { name, .. } => {
                let global_fun_name = if mod_path.is_empty() {
                    name.to_string()
                } else {
                    format!("{mod_path}.{name}")
                };
                if !defined_names.insert(global_fun_name.clone()) {
                    return Err(format!(
                        "top level function `{}` defined twice (at {})",
                        global_fun_name, ast.location
                    ));
                }

                let fun_name = *name;
                let ty_to_be_unified = self.unifier.get_dummy_var().ty;
                // add to the definition list
                self.definition_ast_list.push((
                    RwLock::new(Self::make_top_level_function_def(
                        global_fun_name,
                        *name,
                        // dummy here, unify with correct type later
                        ty_to_be_unified,
                        resolver,
                        Some(ast.location),
                    ))
                    .into(),
                    Some(ast),
                ));

                // return
                Ok((
                    fun_name,
                    DefinitionId(self.definition_ast_list.len() - 1),
                    Some(ty_to_be_unified),
                ))
            }

            ast::StmtKind::Assign { .. } => {
                // Assignment statements can assign to (and therefore create) more than one
                // variable, but this function only allows returning one set of symbol information.
                // We want to avoid changing this to return a `Vec` of symbol info, as this would
                // require `iter().next().unwrap()` on every variable created from a non-Assign
                // statement.
                //
                // Make callers use `register_top_level_var` instead, as it provides more
                // fine-grained control over which symbols to register, while also simplifying the
                // usage of this function.
                panic!(
                    "Registration of top-level Assign statements must use TopLevelComposer::register_top_level_var (at {})",
                    ast.location
                );
            }

            ast::StmtKind::AnnAssign { target, annotation, .. } => {
                let ExprKind::Name { id: name, .. } = target.node else {
                    return Err(format!(
                        "global variable declaration must be an identifier (at {})",
                        target.location
                    ));
                };

                self.register_top_level_var(
                    name,
                    Some(annotation.as_ref().clone()),
                    resolver,
                    mod_path,
                    target.location,
                )
            }

            _ => Err(format!(
                "registrations of constructs other than top level classes/functions/variables are not supported (at {})",
                ast.location
            )),
        }
    }

    /// Registers a top-level variable with the given `name` into the composer.
    ///
    /// - `annotation` - The type annotation of the top-level variable, or [`None`] if no type
    ///   annotation is provided.
    /// - `location` - The location of the top-level variable.
    pub fn register_top_level_var(
        &mut self,
        name: Ident,
        annotation: Option<Expr>,
        resolver: Option<Arc<dyn SymbolResolver + Send + Sync>>,
        mod_path: &str,
        location: Location,
    ) -> Result<(StrRef, DefinitionId, Option<Type>), String> {
        if self.keyword_list.contains(&name) {
            return Err(format!("cannot use keyword `{name}` as a class name (at {location})"));
        }

        let global_var_name =
            if mod_path.is_empty() { name.to_string() } else { format!("{mod_path}.{name}") };

        if !self.defined_names.insert(global_var_name.clone()) {
            return Err(format!(
                "global variable `{global_var_name}` defined twice (at {location})"
            ));
        }

        let ty_to_be_unified = self.unifier.get_dummy_var().ty;
        self.definition_ast_list.push((
            RwLock::new(Self::make_top_level_variable_def(
                global_var_name,
                name,
                // dummy here, unify with correct type later,
                ty_to_be_unified,
                annotation,
                resolver,
                Some(location),
            ))
            .into(),
            None,
        ));

        Ok((name, DefinitionId(self.definition_ast_list.len() - 1), Some(ty_to_be_unified)))
    }

    /// Analyze the AST and modify the corresponding `TopLevelDef`
    pub fn start_analysis(&mut self, inference: bool) -> Result<(), HashSet<String>> {
        self.analyze_top_level_class_definition()?;
        self.analyze_top_level_class_fields_methods()?;
        self.analyze_top_level_function()?;
        self.analyze_top_level_variables()?;
        if inference {
            self.analyze_function_instance()?;
        }
        Ok(())
    }

    /// step 1, analyze the top level class definitions
    ///
    /// Checks for class type variables and ancestors adding them to the `TopLevelDef` list
    fn analyze_top_level_class_definition(&mut self) -> Result<(), HashSet<String>> {
        let def_list = &self.definition_ast_list;
        let unifier = &mut self.unifier;
        let primitives_store = &self.primitives_ty;
        let mut errors = HashSet::new();

        // Initially only copy the definitions of buitin classes and functions
        // class definitions are added in the same order as they appear in the program
        let mut temp_def_list: Vec<Arc<RwLock<TopLevelDef>>> =
            def_list.iter().take(self.builtin_num).map(|f| f.0.clone()).collect_vec();

        // Check for class generic variables and ancestors
        for (class_def, class_ast) in def_list.iter().skip(self.builtin_num) {
            if class_ast.is_some() && matches!(&*class_def.read(), TopLevelDef::Class { .. }) {
                // Add class type variables and direct parents to the `TopLevelDef`
                if let Err(e) = Self::analyze_class_bases(
                    class_def,
                    class_ast,
                    &temp_def_list,
                    unifier,
                    primitives_store,
                ) {
                    errors.extend(e);
                }

                // Add class ancestors
                Self::analyze_class_ancestors(class_def, &temp_def_list);

                // special case classes that inherit from Exception
                let TopLevelDef::Class { ancestors: class_ancestors, .. } = &*class_def.read()
                else {
                    unreachable!()
                };

                if class_ancestors
                    .iter()
                    .any(|ann| matches!(ann, TypeAnnotation::CustomClass { id, .. } if id.0 == 7))
                {
                    // if inherited from Exception, the body should be a pass
                    let ast::StmtKind::ClassDef { body, .. } = &class_ast.as_ref().unwrap().node
                    else {
                        unreachable!()
                    };
                    for stmt in body {
                        if matches!(
                            stmt.node,
                            ast::StmtKind::FunctionDef { .. } | ast::StmtKind::AnnAssign { .. }
                        ) {
                            errors.extend(Err(HashSet::from(["Classes inherited from exception should have no custom fields/methods"])));
                        }
                    }
                }
            }
            temp_def_list.push(class_def.clone());
        }

        // deal with ancestors of Exception object
        let TopLevelDef::Class { name, ancestors, object_id, .. } = &mut *def_list[7].0.write()
        else {
            unreachable!()
        };
        assert_eq!(*name, "Exception".into());
        ancestors.push(make_self_type_annotation(&[], *object_id));

        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(())
    }

    /// step 2, class fields and methods
    fn analyze_top_level_class_fields_methods(&mut self) -> Result<(), HashSet<String>> {
        // Allow resolving definition IDs in error messages
        if self.unifier.top_level.is_none() {
            let ctx = Arc::new(self.make_top_level_context());
            self.unifier.top_level = Some(ctx);
        }

        let def_list = &self.definition_ast_list;
        let temp_def_list = self.extract_def_list();
        let unifier = &mut self.unifier;
        let primitives_store = &self.primitives_ty;

        let mut errors: HashSet<String> = HashSet::new();
        let mut type_var_to_concrete_def: HashMap<Type, TypeAnnotation> = HashMap::new();

        for (class_def, class_ast) in def_list.iter().skip(self.builtin_num) {
            if class_ast.is_some() && matches!(&*class_def.read(), TopLevelDef::Class { .. }) {
                if let Err(e) = Self::analyze_single_class_methods_fields(
                    class_def,
                    &class_ast.as_ref().unwrap().node,
                    &temp_def_list,
                    unifier,
                    primitives_store,
                    &mut type_var_to_concrete_def,
                    (&self.keyword_list, &self.core_config),
                ) {
                    errors.extend(e);
                }

                // The errors need to be reported before copying methods from parent to child classes
                if !errors.is_empty() {
                    return Err(errors);
                }

                // The lock on `class_def` must be released once the ancestors are updated
                {
                    let mut class_def = class_def.write();
                    let TopLevelDef::Class { ancestors, .. } = &*class_def else { unreachable!() };
                    // Methods/fields needs to be processed only if class inherits from another class
                    if ancestors.len() > 1 {
                        if let Err(e) = Self::analyze_single_class_ancestors(
                            &mut class_def,
                            &temp_def_list,
                            unifier,
                            primitives_store,
                            &mut type_var_to_concrete_def,
                        ) {
                            errors.extend(e);
                        };
                    }
                }

                let mut subst_list = Some(Vec::new());
                // unification of previously assigned typevar
                let mut unification_helper = |ty, def| -> Result<(), HashSet<String>> {
                    let target_ty = get_type_from_type_annotation_kinds(
                        &temp_def_list,
                        unifier,
                        primitives_store,
                        &def,
                        &mut subst_list,
                    )?;
                    unifier
                        .unify(ty, target_ty)
                        .map_err(|e| HashSet::from([e.to_display(unifier).to_string()]))?;
                    Ok(())
                };
                for (ty, def) in &type_var_to_concrete_def {
                    if let Err(e) = unification_helper(*ty, def.clone()) {
                        errors.extend(e);
                    }
                }
                for ty in subst_list.unwrap() {
                    let TypeEnum::TObj { obj_id, params, fields } = &*unifier.get_ty(ty) else {
                        unreachable!()
                    };

                    let mut new_fields = HashMap::new();
                    let mut need_subst = false;
                    for (name, (ty, mutable)) in fields {
                        let substituted = unifier.subst(*ty, params);
                        need_subst |= substituted.is_some();
                        new_fields.insert(*name, (substituted.unwrap_or(*ty), *mutable));
                    }
                    if need_subst {
                        let new_ty = unifier.add_ty(TypeEnum::TObj {
                            obj_id: *obj_id,
                            params: params.clone(),
                            fields: new_fields,
                        });
                        if let Err(e) = unifier.unify(ty, new_ty) {
                            errors.insert(e.to_display(unifier).to_string());
                        }
                    }
                }
            }
        }

        for (def, _) in def_list.iter().skip(self.builtin_num) {
            match &*def.read() {
                TopLevelDef::Class { resolver: Some(resolver), .. }
                | TopLevelDef::Function { resolver: Some(resolver), .. } => {
                    if let Err(e) =
                        resolver.handle_deferred_eval(unifier, &temp_def_list, primitives_store)
                    {
                        errors.insert(e);
                    }
                }
                _ => {}
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(())
    }

    /// step 3, after class methods are done, top level functions have nothing unknown
    fn analyze_top_level_function(&mut self) -> Result<(), HashSet<String>> {
        let def_list = &self.definition_ast_list;
        let keyword_list = &self.keyword_list;
        let temp_def_list = self.extract_def_list();
        let unifier = &mut self.unifier;
        let primitives_store = &self.primitives_ty;

        let mut analyze = |function_def: &Arc<RwLock<TopLevelDef>>, function_ast: &Option<Stmt>| {
            let mut function_def = function_def.write();
            let function_def = &mut *function_def;
            let Some(function_ast) = function_ast.as_ref() else {
                // if let TopLevelDef::Function { name, .. } = ``
                return Ok(());
            };

            let TopLevelDef::Function { signature: dummy_ty, resolver, var_id, .. } = function_def
            else {
                // not top level function def, skip
                return Ok(());
            };

            if matches!(unifier.get_ty(*dummy_ty).as_ref(), TypeEnum::TFunc(_)) {
                // already have a function type, is class method, skip
                return Ok(());
            }
            let ast::StmtKind::FunctionDef { args, returns, .. } = &function_ast.node else {
                unreachable!("must be both function");
            };

            let resolver = resolver.as_ref();
            let resolver = resolver.unwrap();
            let resolver = &**resolver;

            let mut function_var_map = VarMap::new();

            let vararg = args
                .vararg
                .as_ref()
                .map(|vararg| -> Result<_, HashSet<String>> {
                    let vararg = vararg.as_ref();

                    let annotation = vararg
                        .node
                        .annotation
                        .as_ref()
                        .ok_or_else(|| {
                            HashSet::from([format!(
                                "function parameter `{}` needs type annotation at {}",
                                vararg.node.arg, vararg.location
                            )])
                        })?
                        .as_ref();

                    let type_annotation = parse_ast_to_type_annotation_kinds(
                        resolver,
                        temp_def_list.as_slice(),
                        unifier,
                        primitives_store,
                        annotation,
                        // NOTE: since only class need this, for function
                        // it should be fine to be empty map
                        HashMap::new(),
                    )?;

                    let type_vars_within =
                        get_type_var_contained_in_type_annotation(&type_annotation)
                            .into_iter()
                            .map(|x| -> Result<TypeVar, HashSet<String>> {
                                let TypeAnnotation::TypeVar(ty) = x else {
                                    unreachable!("must be type var annotation kind")
                                };

                                let id = Self::get_var_id(ty, unifier)?;
                                Ok(TypeVar { id, ty })
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                    for var in type_vars_within {
                        if let Some(prev_ty) = function_var_map.insert(var.id, var.ty) {
                            // if already have the type inserted, make sure they are the same thing
                            assert_eq!(prev_ty, var.ty);
                        }
                    }

                    let ty = get_type_from_type_annotation_kinds(
                        temp_def_list.as_ref(),
                        unifier,
                        primitives_store,
                        &type_annotation,
                        &mut None,
                    )?;

                    Ok(FuncArg {
                        name: vararg.node.arg,
                        ty,
                        default_value: Some(SymbolValue::Tuple(Vec::default())),
                        is_vararg: true,
                    })
                })
                .transpose()?;

            let mut arg_types = {
                // make sure no duplicate parameter
                let mut defined_parameter_name: HashSet<_> = HashSet::new();
                for x in &args.args {
                    if !defined_parameter_name.insert(x.node.arg)
                        || keyword_list.contains(&x.node.arg)
                    {
                        return Err(HashSet::from([format!(
                            "top level function must have unique parameter names \
                            and names should not be the same as the keywords (at {})",
                            x.location
                        )]));
                    }
                }

                let arg_with_default: Vec<(&ast::Located<ast::ArgData<()>>, Option<&Expr>)> = args
                    .args
                    .iter()
                    .rev()
                    .zip(
                        args.defaults
                            .iter()
                            .rev()
                            .map(|x| -> Option<&Expr> { Some(x) })
                            .chain(std::iter::repeat(None)),
                    )
                    .collect_vec();

                arg_with_default
                    .iter()
                    .rev()
                    .map(|(x, default)| -> Result<FuncArg, HashSet<String>> {
                        let annotation = x
                            .node
                            .annotation
                            .as_ref()
                            .ok_or_else(|| {
                                HashSet::from([format!(
                                    "function parameter `{}` needs type annotation at {}",
                                    x.node.arg, x.location
                                )])
                            })?
                            .as_ref();

                        let type_annotation = parse_ast_to_type_annotation_kinds(
                            resolver,
                            temp_def_list.as_slice(),
                            unifier,
                            primitives_store,
                            annotation,
                            // NOTE: since only class need this, for function
                            // it should be fine to be empty map
                            HashMap::new(),
                        )?;

                        let type_vars_within =
                            get_type_var_contained_in_type_annotation(&type_annotation)
                                .into_iter()
                                .map(|x| -> Result<TypeVar, HashSet<String>> {
                                    let TypeAnnotation::TypeVar(ty) = x else {
                                        unreachable!("must be type var annotation kind")
                                    };

                                    let id = Self::get_var_id(ty, unifier)?;
                                    Ok(TypeVar { id, ty })
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                        for var in type_vars_within {
                            if let Some(prev_ty) = function_var_map.insert(var.id, var.ty) {
                                // if already have the type inserted, make sure they are the same thing
                                assert_eq!(prev_ty, var.ty);
                            }
                        }

                        let ty = get_type_from_type_annotation_kinds(
                            temp_def_list.as_ref(),
                            unifier,
                            primitives_store,
                            &type_annotation,
                            &mut None,
                        )?;

                        Ok(FuncArg {
                            name: x.node.arg,
                            ty,
                            default_value: match default {
                                None => None,
                                Some(default) => Some({
                                    let v = Self::parse_parameter_default_value(default, resolver)?;
                                    Self::check_default_param_type(
                                        &v,
                                        &type_annotation,
                                        primitives_store,
                                        unifier,
                                    )
                                    .map_err(|err| {
                                        HashSet::from([format!("{} (at {})", err, x.location)])
                                    })?;
                                    v
                                }),
                            },
                            is_vararg: false,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            };

            if let Some(vararg) = vararg {
                arg_types.push(vararg);
            };

            let arg_types = arg_types;

            let return_ty = {
                if let Some(returns) = returns {
                    let return_ty_annotation = {
                        let return_annotation = returns.as_ref();
                        parse_ast_to_type_annotation_kinds(
                            resolver,
                            &temp_def_list,
                            unifier,
                            primitives_store,
                            return_annotation,
                            // NOTE: since only class need this, for function
                            // it should be fine to be empty map
                            HashMap::new(),
                        )?
                    };

                    let type_vars_within =
                        get_type_var_contained_in_type_annotation(&return_ty_annotation)
                            .into_iter()
                            .map(|x| -> Result<TypeVar, HashSet<String>> {
                                let TypeAnnotation::TypeVar(ty) = x else {
                                    unreachable!("must be type var here")
                                };

                                let id = Self::get_var_id(ty, unifier)?;
                                Ok(TypeVar { id, ty })
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                    for var in type_vars_within {
                        if let Some(prev_ty) = function_var_map.insert(var.id, var.ty) {
                            // if already have the type inserted, make sure they are the same thing
                            assert_eq!(prev_ty, var.ty);
                        }
                    }

                    get_type_from_type_annotation_kinds(
                        &temp_def_list,
                        unifier,
                        primitives_store,
                        &return_ty_annotation,
                        &mut None,
                    )?
                } else {
                    primitives_store.none
                }
            };
            var_id.extend_from_slice(function_var_map
                .iter()
                .filter_map(|(id, ty)| {
                    if matches!(&*unifier.get_ty(*ty), TypeEnum::TVar { range, .. } if range.is_empty()) {
                        None
                    } else {
                        Some(*id)
                    }
                })
                .collect_vec()
                .as_slice()
            );
            let function_ty = unifier.add_ty(TypeEnum::TFunc(FunSignature {
                args: arg_types,
                ret: return_ty,
                vars: function_var_map,
            }));
            unifier.unify(*dummy_ty, function_ty).map_err(|e| {
                HashSet::from([e.at(Some(function_ast.location)).to_display(unifier).to_string()])
            })?;
            Ok(())
        };

        let mut errors = HashSet::new();
        for (function_def, function_ast) in def_list.iter().skip(self.builtin_num) {
            if function_ast.is_none() {
                continue;
            }
            if let Err(e) = analyze(function_def, function_ast) {
                errors.extend(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(())
    }

    fn analyze_single_class_methods_fields(
        class_def: &Arc<RwLock<TopLevelDef>>,
        class_ast: &ast::StmtKind<()>,
        temp_def_list: &[Arc<RwLock<TopLevelDef>>],
        unifier: &mut Unifier,
        primitives: &PrimitiveStore,
        type_var_to_concrete_def: &mut HashMap<Type, TypeAnnotation>,
        core_info: (&HashSet<StrRef>, &ComposerConfig),
    ) -> Result<(), HashSet<String>> {
        let (keyword_list, core_config) = core_info;

        // Clone the Class TLD
        let class_def_tld = class_def.read().clone();

        let mut class_def = class_def.write();
        let TopLevelDef::Class {
            object_id,
            ancestors,
            fields,
            attributes,
            methods,
            resolver,
            type_vars,
            ..
        } = &mut *class_def
        else {
            unreachable!("here must be toplevel class def");
        };
        let ast::StmtKind::ClassDef { name, bases, body, .. } = &class_ast else {
            unreachable!("here must be class def ast")
        };

        let (
            class_id,
            _class_name,
            _class_bases_ast,
            class_body_ast,
            _class_ancestor_def,
            class_fields_def,
            class_attributes_def,
            class_methods_def,
            class_type_vars_def,
            class_resolver,
        ) = (
            *object_id, *name, bases, body, ancestors, fields, attributes, methods, type_vars,
            resolver,
        );

        let class_resolver = class_resolver.as_ref().unwrap();
        let class_resolver = class_resolver.as_ref();

        let mut defined_fields: HashSet<_> = HashSet::new();
        for b in class_body_ast {
            match &b.node {
                ast::StmtKind::FunctionDef { args, returns, name, .. } => {
                    let (method_dummy_ty, method_id) =
                        Self::get_class_method_def_info(class_methods_def, *name)?;

                    let mut method_var_map = VarMap::new();

                    let arg_types: Vec<FuncArg> = {
                        // Function arguments must have:
                        // 1) `self` as first argument (we currently do not support staticmethods)
                        // 2) unique names
                        // 3) names different than keywords
                        match args.args.first() {
                            Some(id) if id.node.arg == "self".into() => {},
                            _ => return Err(HashSet::from([format!(
                                "{name} method must have a `self` parameter (at {})", b.location
                            )])),
                        }
                        let mut defined_parameter_name: HashSet<_> = HashSet::new();
                        for arg in args.args.iter().skip(1) {
                            if !defined_parameter_name.insert(arg.node.arg) {
                                return Err(HashSet::from([format!("class method must have a unique parameter names (at {})", b.location)]));
                            }
                            if keyword_list.contains(&arg.node.arg) {
                                return Err(HashSet::from([format!("parameter names should not be the same as the keywords (at {})", b.location)]));
                            }
                        }

                        // `self` must not be provided type annotation or default value
                        if args.args.len() == args.defaults.len() {
                            return Err(HashSet::from([format!("`self` cannot have a default value (at {})", b.location)]));
                        }
                        if args.args[0].node.annotation.is_some() {
                            return Err(HashSet::from([format!("`self` cannot have a type annotation (at {})", b.location)]));
                        }
                        let mut result = Vec::new();
                        let no_defaults = args.args.len() - args.defaults.len() - 1;
                        for (idx, x) in args.args.iter().skip(1).enumerate() {
                            let type_ann = {
                                let Some(annotation_expr) = x.node.annotation.as_ref() else {return Err(HashSet::from([format!("type annotation needed for `{}` (at {})", x.node.arg, x.location)]));};
                                parse_ast_to_type_annotation_kinds(
                                    class_resolver,
                                    temp_def_list,
                                    unifier,
                                    primitives,
                                    annotation_expr,
                                    vec![(class_id, class_type_vars_def.clone())]
                                        .into_iter()
                                        .collect::<HashMap<_, _>>(),
                                )?
                            };
                            // find type vars within this method parameter type annotation
                            let type_vars_within = get_type_var_contained_in_type_annotation(&type_ann);
                            // handle the class type var and the method type var
                            for type_var_within in type_vars_within {
                                let TypeAnnotation::TypeVar(ty) = type_var_within else {
                                    unreachable!("must be type var annotation")
                                };

                                let id = Self::get_var_id(ty, unifier)?;
                                if let Some(prev_ty) = method_var_map.insert(id, ty) {
                                    // if already in the list, make sure they are the same?
                                    assert_eq!(prev_ty, ty);
                                }
                            }
                            // finish handling type vars
                            let dummy_func_arg = FuncArg {
                                name: x.node.arg,
                                ty: unifier.get_dummy_var().ty,
                                default_value: if idx < no_defaults { None } else {
                                    let default_idx = idx - no_defaults;

                                    Some({
                                        let v = Self::parse_parameter_default_value(&args.defaults[default_idx], class_resolver)?;
                                        Self::check_default_param_type(&v, &type_ann, primitives, unifier).map_err(|err| HashSet::from([format!("{} (at {})", err, x.location)]))?;
                                        v
                                    })
                                },
                                is_vararg: false,
                            };
                            // push the dummy type and the type annotation
                            // into the list for later unification
                            type_var_to_concrete_def
                                .insert(dummy_func_arg.ty, type_ann.clone());
                            result.push(dummy_func_arg);
                        }
                        result
                    };

                    let ret_type = {
                        if let Some(result) = returns {
                            let result = result.as_ref();
                            let annotation = parse_ast_to_type_annotation_kinds(
                                class_resolver,
                                temp_def_list,
                                unifier,
                                primitives,
                                result,
                                vec![(class_id, class_type_vars_def.clone())]
                                    .into_iter()
                                    .collect::<HashMap<_, _>>(),
                            )?;
                            // find type vars within this return type annotation
                            let type_vars_within =
                                get_type_var_contained_in_type_annotation(&annotation);
                            // handle the class type var and the method type var
                            for type_var_within in type_vars_within {
                                let TypeAnnotation::TypeVar(ty) = type_var_within else {
                                    unreachable!("must be type var annotation");
                                };

                                let id = Self::get_var_id(ty, unifier)?;
                                if let Some(prev_ty) = method_var_map.insert(id, ty) {
                                    // if already in the list, make sure they are the same?
                                    assert_eq!(prev_ty, ty);
                                }
                            }
                            let dummy_return_type = unifier.get_dummy_var().ty;
                            type_var_to_concrete_def.insert(dummy_return_type, annotation.clone());
                            dummy_return_type
                        } else {
                            // if do not have return annotation, return none
                            // for uniform handling, still use type annotation
                            let dummy_return_type = unifier.get_dummy_var().ty;
                            type_var_to_concrete_def.insert(
                                dummy_return_type,
                                TypeAnnotation::Primitive(primitives.none),
                            );
                            dummy_return_type
                        }
                    };

                    let TopLevelDef::Function { var_id, .. } =
                        &mut *temp_def_list.get(method_id.0).unwrap().write() else {
                        unreachable!()
                    };
                    var_id.extend_from_slice(method_var_map
                        .iter()
                        .filter_map(|(id, ty)| {
                            if matches!(&*unifier.get_ty(*ty), TypeEnum::TVar { range, .. } if range.is_empty()) {
                                None
                            } else {
                                Some(*id)
                            }
                        })
                        .collect_vec()
                        .as_slice()
                    );
                    let method_type = unifier.add_ty(TypeEnum::TFunc(FunSignature {
                        args: arg_types,
                        ret: ret_type,
                        vars: method_var_map,
                    }));

                    // unify now since function type is not in type annotation define
                    // which should be fine since type within method_type will be subst later
                    unifier
                        .unify(method_dummy_ty, method_type)
                        .map_err(|e| HashSet::from([e.to_display(unifier).to_string()]))?;
                }
                ast::StmtKind::AnnAssign { target, annotation, value, .. } => {
                    if let ExprKind::Name { id: attr, .. } = &target.node {
                        if defined_fields.insert(attr.to_string()) {
                            let dummy_field_type = unifier.get_dummy_var().ty;

                            let annotation = match value {
                                None => {
                                    // handle Kernel[T], KernelInvariant[T]
                                    let (annotation, mutable) = match &annotation.node {
                                            ExprKind::Subscript { slice, .. }
                                        if core_config.has_invariant_ann(*attr, Some(&class_def_tld), annotation).map_err(|err| HashSet::from([err]))? =>
                                        {
                                            (slice, false)
                                        }
                                        ExprKind::Subscript { slice, .. }
                                            if core_config.has_kernel_ann(*attr, Some(&class_def_tld), annotation).map_err(|err| HashSet::from([err]))?.unwrap_or_default() =>
                                        {
                                            (slice, true)
                                        }
                                        _ if core_config.has_kernel_ann_fn.is_none() => (annotation, true),
                                        _ => continue, // ignore fields annotated otherwise
                                    };
                                    class_fields_def.push((*attr, dummy_field_type, mutable));
                                    annotation
                                }
                                // Supporting Class Attributes
                                Some(boxed_expr) => {
                                    // Class attributes are set as immutable regardless
                                    let (annotation, _) = match &annotation.node {
                                        ExprKind::Subscript { slice, .. } => (slice, false),
                                        _ if core_config.has_kernel_ann_fn.is_none() => (annotation, false),
                                        _ => continue,
                                    };

                                    match &**boxed_expr {
                                        ast::Located {location: _, custom: (), node: ExprKind::Constant { value: v, kind: _ }} => {
                                            // Restricting the types allowed to be defined as class attributes
                                            match v {
                                                ast::Constant::Bool(_) | ast::Constant::Str(_) | ast::Constant::Int(_) | ast::Constant::Float(_) => {}
                                                _ => {
                                                    return Err(HashSet::from([
                                                    format!(
                                                        "unsupported statement in class definition body (at {})",
                                                        b.location
                                                    ),
                                                ]))
                                                }
                                            }
                                            class_attributes_def.push((*attr, dummy_field_type, v.clone()));
                                        }
                                        _ => {
                                            return Err(HashSet::from([
                                                format!(
                                                    "unsupported statement in class definition body (at {})",
                                                    b.location
                                                ),
                                            ]))
                                        }
                                    }
                                    annotation
                                }
                            };
                            let parsed_annotation = parse_ast_to_type_annotation_kinds(
                                class_resolver,
                                temp_def_list,
                                unifier,
                                primitives,
                                annotation.as_ref(),
                                vec![(class_id, class_type_vars_def.clone())]
                                    .into_iter()
                                    .collect::<HashMap<_, _>>(),
                            )?;
                            // find type vars within this return type annotation
                            let type_vars_within =
                                get_type_var_contained_in_type_annotation(&parsed_annotation);
                            // handle the class type var and the method type var
                            for type_var_within in type_vars_within {
                                let TypeAnnotation::TypeVar(t) = type_var_within else {
                                    unreachable!("must be type var annotation")
                                };

                                if !class_type_vars_def.contains(&t){
                                    return Err(HashSet::from([
                                        format!(
                                            "class fields can only use type \
                                                vars over which the class is generic (at {})",
                                            annotation.location
                                        ),
                                    ]))
                                }
                            }
                            type_var_to_concrete_def.insert(dummy_field_type, parsed_annotation);
                        } else {
                            return Err(HashSet::from([
                                format!(
                                    "same class fields `{}` defined twice (at {})",
                                    attr, target.location
                                ),
                            ]))
                        }
                    } else {
                        return Err(HashSet::from([
                            format!(
                                "unsupported statement type in class definition body (at {})",
                                target.location
                            ),
                        ]))
                    }
                }
                ast::StmtKind::Assign { .. } // we don't class attributes
                | ast::StmtKind::Expr { value: _, .. } // typically a docstring; ignoring all expressions matches CPython behavior
                | ast::StmtKind::Pass { .. } => {}
                _ => {
                    return Err(HashSet::from([
                        format!(
                            "unsupported statement type in class definition body (at {})",
                            b.location
                        ),
                    ]))
                }
            }
        }
        Ok(())
    }

    fn analyze_single_class_ancestors(
        class_def: &mut TopLevelDef,
        temp_def_list: &[Arc<RwLock<TopLevelDef>>],
        unifier: &mut Unifier,
        _primitives: &PrimitiveStore,
        type_var_to_concrete_def: &mut HashMap<Type, TypeAnnotation>,
    ) -> Result<(), HashSet<String>> {
        let TopLevelDef::Class {
            object_id,
            ancestors,
            fields,
            attributes,
            methods,
            resolver,
            type_vars,
            ..
        } = class_def
        else {
            unreachable!("here must be class def ast")
        };
        let (
            _class_id,
            class_ancestor_def,
            class_fields_def,
            class_attribute_def,
            class_methods_def,
            _class_type_vars_def,
            _class_resolver,
        ) = (*object_id, ancestors, fields, attributes, methods, type_vars, resolver);

        // since when this function is called, the ancestors of the direct parent
        // are supposed to be already handled, so we only need to deal with the direct parent
        let base = class_ancestor_def.get(1).unwrap();
        let TypeAnnotation::CustomClass { id, params: _ } = base else {
            unreachable!("must be class type annotation")
        };
        let base = temp_def_list.get(id.0).unwrap();
        let base = base.read();
        let TopLevelDef::Class { methods, fields, attributes, .. } = &*base else {
            unreachable!("must be top level class def")
        };

        // handle methods override
        // since we need to maintain the order, create a new list
        let mut new_child_methods: IndexMap<StrRef, (Type, DefinitionId)> =
            methods.iter().map(|m| (m.0, (m.1, m.2))).collect();

        for (class_method_name, class_method_ty, class_method_defid) in &*class_methods_def {
            if let Some((ty, _)) = new_child_methods
                .insert(*class_method_name, (*class_method_ty, *class_method_defid))
            {
                let ok = class_method_name == &"__init__".into()
                    || Self::check_overload_function_type(
                        *class_method_ty,
                        ty,
                        unifier,
                        type_var_to_concrete_def,
                    );
                if !ok {
                    return Err(HashSet::from([format!(
                        "method {class_method_name} has same name as ancestors' method, but incompatible type"
                    )]));
                }
            }
        }
        class_methods_def.clear();
        class_methods_def
            .extend(new_child_methods.iter().map(|f| (*f.0, f.1.0, f.1.1)).collect_vec());

        // handle class fields
        let mut new_child_fields: IndexMap<StrRef, (Type, bool)> =
            fields.iter().map(|f| (f.0, (f.1, f.2))).collect();
        let mut new_child_attributes: IndexMap<StrRef, (Type, ast::Constant)> =
            attributes.iter().map(|f| (f.0, (f.1, f.2.clone()))).collect();
        // Overriding class fields and attributes is currently not supported
        for (name, ty, mutable) in &*class_fields_def {
            if new_child_fields.insert(*name, (*ty, *mutable)).is_some()
                || new_child_attributes.contains_key(name)
            {
                return Err(HashSet::from([format!(
                    "field `{name}` has already declared in the ancestor classes"
                )]));
            }
        }
        for (name, ty, val) in &*class_attribute_def {
            if new_child_attributes.insert(*name, (*ty, val.clone())).is_some()
                || new_child_fields.contains_key(name)
            {
                return Err(HashSet::from([format!(
                    "attribute `{name}` has already declared in the ancestor classes"
                )]));
            }
        }

        class_fields_def.clear();
        class_fields_def
            .extend(new_child_fields.iter().map(|f| (*f.0, f.1.0, f.1.1)).collect_vec());
        class_attribute_def.clear();
        class_attribute_def.extend(
            new_child_attributes.iter().map(|f| (*f.0, f.1.0, f.1.1.clone())).collect_vec(),
        );
        Ok(())
    }

    /// step 5, analyze and call type inferencer to fill the `instance_to_stmt` of
    /// [`TopLevelDef::Function`]
    fn analyze_function_instance(&mut self) -> Result<(), HashSet<String>> {
        // first get the class constructor type correct for the following type check in function body
        // also do class field instantiation check
        let init_str_id = "__init__".into();
        let mut definition_extension = Vec::new();
        let mut constructors = Vec::new();
        let def_list = self.extract_def_list();
        let primitives_ty = &self.primitives_ty;
        let definition_ast_list = &self.definition_ast_list;
        let unifier = &mut self.unifier;

        // first, fix function typevar ids
        // they may be changed with our use of placeholders
        for (def, _) in definition_ast_list.iter().skip(self.builtin_num) {
            if let TopLevelDef::Function { signature, var_id, .. } = &mut *def.write() {
                if let TypeEnum::TFunc(FunSignature { args, ret, vars }) =
                    unifier.get_ty(*signature).as_ref()
                {
                    let new_var_ids = vars
                        .values()
                        .map(|v| match &*unifier.get_ty(*v) {
                            TypeEnum::TVar { id, .. } => *id,
                            _ => unreachable!(),
                        })
                        .collect_vec();
                    if new_var_ids != *var_id {
                        let new_signature = FunSignature {
                            args: args.clone(),
                            ret: *ret,
                            vars: new_var_ids
                                .iter()
                                .zip(vars.values())
                                .map(|(id, v)| (*id, *v))
                                .collect(),
                        };
                        unifier
                            .unification_table
                            .set_value(*signature, Rc::new(TypeEnum::TFunc(new_signature)));
                        *var_id = new_var_ids;
                    }
                }
            }
        }

        let mut analyze = |i, def: &Arc<RwLock<TopLevelDef>>, ast: &Option<Stmt>| {
            let class_def = def.read();
            if let TopLevelDef::Class {
                constructor,
                ancestors,
                methods,
                fields,
                type_vars,
                name: class_name,
                object_id,
                resolver: _,
                ..
            } = &*class_def
            {
                let self_type = get_type_from_type_annotation_kinds(
                    &def_list,
                    unifier,
                    primitives_ty,
                    &make_self_type_annotation(type_vars, *object_id),
                    &mut None,
                )?;
                if ancestors
                    .iter()
                    .any(|ann| matches!(ann, TypeAnnotation::CustomClass { id, .. } if id.0 == 7))
                {
                    // create constructor for these classes
                    let PrimitiveStore { str: string, int64, .. } = *primitives_ty;
                    let signature = unifier.add_ty(TypeEnum::TFunc(FunSignature {
                        args: vec![
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
                        ],
                        ret: self_type,
                        vars: VarMap::default(),
                    }));
                    let cons_fun = TopLevelDef::Function {
                        name: format!("{}.{}", class_name, "__init__"),
                        simple_name: init_str_id,
                        signature,
                        var_id: Vec::default(),
                        instance_to_symbol: HashMap::default(),
                        instance_to_stmt: HashMap::default(),
                        resolver: None,
                        codegen_callback: Some(Arc::new(GenCall::new(Box::new(exn_constructor)))),
                        loc: None,
                    };
                    constructors.push((i, signature, definition_extension.len()));
                    definition_extension.push((Arc::new(RwLock::new(cons_fun)), None));
                    unifier.unify(constructor.unwrap(), signature).map_err(|e| {
                        HashSet::from([e
                            .at(Some(ast.as_ref().unwrap().location))
                            .to_display(unifier)
                            .to_string()])
                    })?;
                    return Ok(());
                }
                let mut init_id: Option<DefinitionId> = None;
                // get the class constructor type correct
                let (contor_args, contor_type_vars) = {
                    let mut constructor_args: Vec<FuncArg> = Vec::new();
                    let mut type_vars = VarMap::new();
                    for (name, func_sig, id) in methods {
                        if *name == init_str_id {
                            init_id = Some(*id);
                            let func_ty_enum = unifier.get_ty(*func_sig);
                            let TypeEnum::TFunc(FunSignature { args, vars, .. }) =
                                func_ty_enum.as_ref()
                            else {
                                unreachable!("must be typeenum::tfunc")
                            };

                            constructor_args.extend_from_slice(args);
                            type_vars.extend(vars);
                        }
                    }
                    (constructor_args, type_vars)
                };
                let contor_type = unifier.add_ty(TypeEnum::TFunc(FunSignature {
                    args: contor_args,
                    ret: self_type,
                    vars: contor_type_vars,
                }));
                unifier.unify(constructor.unwrap(), contor_type).map_err(|e| {
                    HashSet::from([e
                        .at(Some(ast.as_ref().unwrap().location))
                        .to_display(unifier)
                        .to_string()])
                })?;

                // class field instantiation check
                if let (Some(init_id), false) = (init_id, fields.is_empty()) {
                    let init_ast = definition_ast_list.get(init_id.0).unwrap().1.as_ref().unwrap();
                    if let ast::StmtKind::FunctionDef { name, body, .. } = &init_ast.node {
                        if *name != init_str_id {
                            unreachable!("must be init function here")
                        }

                        let all_inited = Self::get_all_assigned_field(
                            object_id.0,
                            definition_ast_list,
                            body.as_slice(),
                        )?;
                        for (f, _, _) in fields {
                            if !all_inited.contains(f) {
                                return Err(HashSet::from([format!(
                                    "fields `{}` of class `{}` not fully initialized in the initializer (at {})",
                                    f, class_name, body[0].location,
                                )]));
                            }
                        }
                    }
                }
            }
            Ok(())
        };

        let mut errors = HashSet::new();
        for (i, (def, ast)) in definition_ast_list.iter().enumerate().skip(self.builtin_num) {
            if ast.is_none() {
                continue;
            }
            if let Err(e) = analyze(i, def, ast) {
                errors.extend(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        for (i, signature, id) in constructors {
            let TopLevelDef::Class { methods, .. } = &mut *self.definition_ast_list[i].0.write()
            else {
                unreachable!()
            };

            methods.push((
                init_str_id,
                signature,
                DefinitionId(self.definition_ast_list.len() + id),
            ));
        }
        self.definition_ast_list.extend_from_slice(&definition_extension);

        let ctx = Arc::new(self.make_top_level_context());
        // type inference inside function body
        let def_list = self.extract_def_list();
        let primitives_ty = &self.primitives_ty;
        let definition_ast_list = &self.definition_ast_list;
        let unifier = &mut self.unifier;
        let method_class = &mut self.method_class;
        let mut analyze_2 = |id, def: &Arc<RwLock<TopLevelDef>>, ast: &Option<Stmt>| {
            if ast.is_none() {
                return Ok(());
            }

            let (name, simple_name, signature, resolver) = {
                let function_def = def.read();
                let TopLevelDef::Function { name, simple_name, signature, resolver, .. } =
                    &*function_def
                else {
                    return Ok(());
                };

                (name.clone(), *simple_name, *signature, resolver.clone())
            };

            let signature_ty_enum = unifier.get_ty(signature);
            let TypeEnum::TFunc(FunSignature { args, ret, vars, .. }) = signature_ty_enum.as_ref()
            else {
                unreachable!("must be typeenum::tfunc")
            };

            let mut vars = vars.clone();
            // None if is not class method
            let uninst_self_type = {
                if let Some(class_id) = method_class.get(&DefinitionId(id)) {
                    let class_def = definition_ast_list.get(class_id.0).unwrap();
                    let class_def = class_def.0.read();
                    let TopLevelDef::Class { type_vars, .. } = &*class_def else {
                        unreachable!("must be class def")
                    };

                    let ty_ann = make_self_type_annotation(type_vars, *class_id);
                    let self_ty = get_type_from_type_annotation_kinds(
                        &def_list,
                        unifier,
                        primitives_ty,
                        &ty_ann,
                        &mut None,
                    )?;
                    vars.extend(type_vars.iter().map(|ty| {
                        let TypeEnum::TVar { id, .. } = &*unifier.get_ty(*ty) else {
                            unreachable!()
                        };

                        (*id, *ty)
                    }));
                    Some((self_ty, type_vars.clone()))
                } else {
                    None
                }
            };
            // carefully handle those with bounds, without bounds and no typevars
            // if class methods, `vars` also contains all class typevars here
            let (type_var_subst_comb, no_range_vars) = {
                let mut no_ranges: Vec<Type> = Vec::new();
                let var_combs = vars
                    .values()
                    .map(|ty| {
                        unifier.get_instantiations(*ty).unwrap_or_else(|| {
                            let TypeEnum::TVar { name, loc, is_const_generic: false, .. } =
                                &*unifier.get_ty(*ty)
                            else {
                                unreachable!()
                            };

                            let rigid = unifier.get_fresh_rigid_var(*name, *loc).ty;
                            no_ranges.push(rigid);
                            vec![rigid]
                        })
                    })
                    .multi_cartesian_product()
                    .collect_vec();
                let mut result: Vec<VarMap> = Vec::default();
                for comb in var_combs {
                    result.push(vars.keys().copied().zip(comb).collect());
                }
                // NOTE: if is empty, means no type var, append a empty subst, ok to do this?
                if result.is_empty() {
                    result.push(VarMap::new());
                }
                (result, no_ranges)
            };

            for subst in type_var_subst_comb {
                // for each instance
                let inst_ret = unifier.subst(*ret, &subst).unwrap_or(*ret);
                let inst_args = {
                    args.iter()
                        .map(|a| FuncArg {
                            name: a.name,
                            ty: unifier.subst(a.ty, &subst).unwrap_or(a.ty),
                            default_value: a.default_value.clone(),
                            is_vararg: false,
                        })
                        .collect_vec()
                };
                let self_type = {
                    uninst_self_type.clone().map(|(self_type, type_vars)| {
                        let subst_for_self = {
                            let class_ty_var_ids = type_vars
                                .iter()
                                .map(|x| {
                                    if let TypeEnum::TVar { id, .. } = &*unifier.get_ty(*x) {
                                        *id
                                    } else {
                                        unreachable!("must be type var here");
                                    }
                                })
                                .collect::<HashSet<_>>();
                            subst
                                .iter()
                                .filter_map(|(ty_var_id, ty_var_target)| {
                                    if class_ty_var_ids.contains(ty_var_id) {
                                        Some((*ty_var_id, *ty_var_target))
                                    } else {
                                        None
                                    }
                                })
                                .collect::<VarMap>()
                        };
                        unifier.subst(self_type, &subst_for_self).unwrap_or(self_type)
                    })
                };
                let mut identifiers = {
                    let mut result = HashMap::new();
                    if self_type.is_some() {
                        result.insert("self".into(), IdentifierInfo::default());
                    }
                    result.extend(inst_args.iter().map(|x| (x.name, IdentifierInfo::default())));
                    result
                };
                let mut calls: HashMap<CodeLocation, CallId> = HashMap::new();
                let mut inferencer = Inferencer {
                    top_level: ctx.as_ref(),
                    defined_identifiers: identifiers.clone(),
                    function_data: &mut FunctionData {
                        resolver: resolver.as_ref().unwrap().clone(),
                        return_type: if unifier.unioned(inst_ret, primitives_ty.none) {
                            None
                        } else {
                            Some(inst_ret)
                        },
                        // NOTE: allowed type vars
                        bound_variables: no_range_vars.clone(),
                    },
                    unifier,
                    variable_mapping: {
                        let mut result: HashMap<StrRef, Type> = HashMap::new();
                        if let Some(self_ty) = self_type {
                            result.insert("self".into(), self_ty);
                        }
                        result.extend(inst_args.iter().map(|x| (x.name, x.ty)));
                        result
                    },
                    primitives: primitives_ty,
                    virtual_checks: &mut Vec::new(),
                    calls: &mut calls,
                    in_handler: false,
                };

                let ast::StmtKind::FunctionDef { body, decorator_list, .. } =
                    ast.clone().unwrap().node
                else {
                    unreachable!("must be function def ast")
                };

                if decorator_list.first().map_or(Ok(false), |decorator| {
                    self.core_config
                        .is_extern_decorator(decorator)
                        .map_err(|err| HashSet::from([err]))
                })? {
                    let TopLevelDef::Function { instance_to_symbol, .. } = &mut *def.write() else {
                        unreachable!()
                    };
                    instance_to_symbol.insert(String::new(), simple_name.to_string());
                    continue;
                }

                let fun_body =
                    body.into_iter()
                        .map(|b| inferencer.fold_stmt(b))
                        .collect::<Result<Vec<_>, _>>()?;

                let returned = inferencer.check_block(fun_body.as_slice(), &mut identifiers)?;
                {
                    // check virtuals
                    let defs = ctx.definitions.read();
                    for (subtype, base, loc) in &*inferencer.virtual_checks {
                        let base_id = {
                            let base = inferencer.unifier.get_ty(*base);
                            if let TypeEnum::TObj { obj_id, .. } = &*base {
                                *obj_id
                            } else {
                                return Err(HashSet::from([format!(
                                    "Base type should be a class (at {loc})"
                                )]));
                            }
                        };
                        let subtype_id = {
                            let ty = inferencer.unifier.get_ty(*subtype);
                            if let TypeEnum::TObj { obj_id, .. } = &*ty {
                                *obj_id
                            } else {
                                let base_repr = inferencer.unifier.stringify(*base);
                                let subtype_repr = inferencer.unifier.stringify(*subtype);
                                return Err(HashSet::from([format!(
                                    "Expected a subtype of {base_repr}, but got {subtype_repr} (at {loc})"
                                )]));
                            }
                        };
                        let subtype_entry = defs[subtype_id.0].read();
                        let TopLevelDef::Class { ancestors, .. } = &*subtype_entry else {
                            unreachable!()
                        };

                        let m = ancestors.iter()
                            .find(|kind| matches!(kind, TypeAnnotation::CustomClass { id, .. } if *id == base_id));
                        if m.is_none() {
                            let base_repr = inferencer.unifier.stringify(*base);
                            let subtype_repr = inferencer.unifier.stringify(*subtype);
                            return Err(HashSet::from([format!(
                                "Expected a subtype of {base_repr}, but got {subtype_repr} (at {loc})"
                            )]));
                        }
                    }
                }
                if !unifier.unioned(inst_ret, primitives_ty.none) && !returned {
                    let def_ast_list = &definition_ast_list;
                    let ret_str = unifier.internal_stringify(
                        inst_ret,
                        &mut |id| {
                            let TopLevelDef::Class { name, .. } = &*def_ast_list[id].0.read()
                            else {
                                unreachable!("must be class id here")
                            };

                            name.to_string()
                        },
                        &mut |id| format!("typevar{id}"),
                        &mut None,
                    );
                    return Err(HashSet::from([format!(
                        "expected return type of `{}` in function `{}` (at {})",
                        ret_str,
                        name,
                        ast.as_ref().unwrap().location
                    )]));
                }

                let TopLevelDef::Function { instance_to_stmt, .. } = &mut *def.write() else {
                    unreachable!()
                };
                instance_to_stmt.insert(
                    get_subst_key(
                        unifier,
                        self_type,
                        &subst,
                        Some(&vars.keys().copied().collect()),
                    ),
                    FunInstance {
                        body: Arc::new(fun_body),
                        unifier_id: 0,
                        calls: Arc::new(calls),
                        subst,
                    },
                );
            }

            Ok(())
        };

        for (id, (def, ast)) in self.definition_ast_list.iter().enumerate().skip(self.builtin_num) {
            if ast.is_none() {
                continue;
            }
            if let Err(e) = analyze_2(id, def, ast) {
                errors.extend(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(())
    }

    /// Step 4. Analyze and populate the types of global variables.
    fn analyze_top_level_variables(&mut self) -> Result<(), HashSet<String>> {
        let def_list = &self.definition_ast_list;
        let temp_def_list = self.extract_def_list();
        let unifier = &mut self.unifier;
        let primitives_store = &self.primitives_ty;

        let mut analyze = |variable_def: &Arc<RwLock<TopLevelDef>>| -> Result<_, HashSet<String>> {
            let TopLevelDef::Variable { ty: dummy_ty, ty_decl, resolver, loc, .. } =
                &*variable_def.read()
            else {
                // not top level variable def, skip
                return Ok(());
            };

            let resolver = &**resolver.as_ref().unwrap();

            if let Some(ty_decl) = ty_decl {
                let ty_annotation = parse_ast_to_type_annotation_kinds(
                    resolver,
                    &temp_def_list,
                    unifier,
                    primitives_store,
                    ty_decl,
                    HashMap::new(),
                )?;
                let ty_from_ty_annotation = get_type_from_type_annotation_kinds(
                    &temp_def_list,
                    unifier,
                    primitives_store,
                    &ty_annotation,
                    &mut None,
                )?;

                unifier.unify(*dummy_ty, ty_from_ty_annotation).map_err(|e| {
                    HashSet::from([e.at(Some(loc.unwrap())).to_display(unifier).to_string()])
                })?;
            }

            Ok(())
        };

        let mut errors = HashSet::new();
        for (variable_def, _) in def_list.iter().skip(self.builtin_num) {
            if let Err(e) = analyze(variable_def) {
                errors.extend(e);
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        Ok(())
    }
}
