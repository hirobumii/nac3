use nac3core::{
    codegen::{
        expr::gen_call,
        llvm_intrinsics::{call_int_smax, call_stackrestore, call_stacksave},
        stmt::{gen_block, gen_with},
        CodeGenContext, CodeGenerator,
    },
    symbol_resolver::ValueEnum,
    toplevel::{DefinitionId, GenCall},
    typecheck::typedef::{FunSignature, FuncArg, Type, TypeEnum}
};

use nac3parser::ast::{Expr, ExprKind, Located, Stmt, StmtKind, StrRef};

use inkwell::{
    context::Context, 
    module::Linkage, 
    types::IntType, 
    values::BasicValueEnum,
    AddressSpace,
};

use pyo3::{PyObject, PyResult, Python, types::{PyDict, PyList}};

use crate::{symbol_resolver::InnerResolver, timeline::TimeFns};

use std::{
    collections::hash_map::DefaultHasher,
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};

/// The parallelism mode within a block.
#[derive(Copy, Clone, Eq, PartialEq)]
enum ParallelMode {
    /// No parallelism is currently registered for this context.
    None,

    /// Legacy (or shallow) parallelism. Default before NAC3.
    ///
    /// Each statement within the `with` block is treated as statements to be executed in parallel.
    Legacy,

    /// Deep parallelism. Default since NAC3.
    ///
    /// Each function call within the `with` block (except those within a nested `sequential` block)
    /// are treated to be executed in parallel.
    Deep
}

pub struct ArtiqCodeGenerator<'a> {
    name: String,

    /// The size of a `size_t` variable in bits.
    size_t: u32,

    /// Monotonic counter for naming `start`/`stop` variables used by `with parallel` blocks.
    name_counter: u32,

    /// Variable for tracking the start of a `with parallel` block.
    start: Option<Expr<Option<Type>>>,

    /// Variable for tracking the end of a `with parallel` block.
    end: Option<Expr<Option<Type>>>,
    timeline: &'a (dyn TimeFns + Sync),

    /// The [ParallelMode] of the current parallel context.
    ///
    /// The current parallel context refers to the nearest `with parallel` or `with legacy_parallel`
    /// statement, which is used to determine when and how the timeline should be updated.
    parallel_mode: ParallelMode,
}

impl<'a> ArtiqCodeGenerator<'a> {
    pub fn new(
        name: String,
        size_t: u32,
        timeline: &'a (dyn TimeFns + Sync),
    ) -> ArtiqCodeGenerator<'a> {
        assert!(size_t == 32 || size_t == 64);
        ArtiqCodeGenerator {
            name,
            size_t,
            name_counter: 0,
            start: None,
            end: None,
            timeline,
            parallel_mode: ParallelMode::None,
        }
    }

    /// If the generator is currently in a direct-`parallel` block context, emits IR that resets the
    /// position of the timeline to the initial timeline position before entering the `parallel`
    /// block.
    ///
    /// Direct-`parallel` block context refers to when the generator is generating statements whose
    /// closest parent `with` statement is a `with parallel` block.
    fn timeline_reset_start(
        &mut self,
        ctx: &mut CodeGenContext<'_, '_>
    ) -> Result<(), String> {
        if let Some(start) = self.start.clone() {
            let start_val = self.gen_expr(ctx, &start)?
                .unwrap()
                .to_basic_value_enum(ctx, self, start.custom.unwrap())?;
            self.timeline.emit_at_mu(ctx, start_val);
        }

        Ok(())
    }

    /// If the generator is currently in a `parallel` block context, emits IR that updates the
    /// maximum end position of the `parallel` block as specified by the timeline `end` value.
    ///
    /// In general the `end` parameter should be set to `self.end` for updating the maximum end
    /// position for the current `parallel` block. Other values can be passed in to update the
    /// maximum end position for other `parallel` blocks.
    ///
    /// `parallel`-block context refers to when the generator is generating statements within a
    /// (possibly indirect) `parallel` block.
    ///
    /// * `store_name` - The LLVM value name for the pointer to `end`. `.addr` will be appended to
    /// the end of the provided value name.
    fn timeline_update_end_max(
        &mut self,
        ctx: &mut CodeGenContext<'_, '_>,
        end: Option<Expr<Option<Type>>>,
        store_name: Option<&str>,
    ) -> Result<(), String> {
        if let Some(end) = end {
            let old_end = self.gen_expr(ctx, &end)?
                .unwrap()
                .to_basic_value_enum(ctx, self, end.custom.unwrap())?;
            let now = self.timeline.emit_now_mu(ctx);
            let max = call_int_smax(
                ctx, 
                old_end.into_int_value(), 
                now.into_int_value(), 
                Some("smax")
            );
            let end_store = self.gen_store_target(
                ctx,
                &end,
                store_name.map(|name| format!("{name}.addr")).as_deref())?
                .unwrap();
            ctx.builder.build_store(end_store, max).unwrap();
        }

        Ok(())
    }
}

impl<'b> CodeGenerator for ArtiqCodeGenerator<'b> {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_size_type<'ctx>(&self, ctx: &'ctx Context) -> IntType<'ctx> {
        if self.size_t == 32 {
            ctx.i32_type()
        } else {
            ctx.i64_type()
        }
    }

    fn gen_block<'ctx, 'a, 'c, I: Iterator<Item=&'c Stmt<Option<Type>>>>(
        &mut self,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        stmts: I
    ) -> Result<(), String> where Self: Sized {
        // Legacy parallel emits timeline end-update/timeline-reset after each top-level statement
        // in the parallel block
        if self.parallel_mode == ParallelMode::Legacy {
            for stmt in stmts {
                self.gen_stmt(ctx, stmt)?;

                if ctx.is_terminated() {
                    break;
                }

                self.timeline_update_end_max(ctx, self.end.clone(), Some("end"))?;
                self.timeline_reset_start(ctx)?;
            }

            Ok(())
        } else {
            gen_block(self, ctx, stmts)
        }
    }

    fn gen_call<'ctx>(
        &mut self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        obj: Option<(Type, ValueEnum<'ctx>)>,
        fun: (&FunSignature, DefinitionId),
        params: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    ) -> Result<Option<BasicValueEnum<'ctx>>, String> {
        let result = gen_call(self, ctx, obj, fun, params)?;

        // Deep parallel emits timeline end-update/timeline-reset after each function call
        if self.parallel_mode == ParallelMode::Deep {
            self.timeline_update_end_max(ctx, self.end.clone(), Some("end"))?;
            self.timeline_reset_start(ctx)?;
        }

        Ok(result)
    }

    fn gen_with(
        &mut self,
        ctx: &mut CodeGenContext<'_, '_>,
        stmt: &Stmt<Option<Type>>,
    ) -> Result<(), String> {
        let StmtKind::With { items, body, .. } = &stmt.node else {
            unreachable!()
        };

        if items.len() == 1 && items[0].optional_vars.is_none() {
            let item = &items[0];

            // Behavior of parallel and sequential:
            // Each function call (indirectly, can be inside a sequential block) within a parallel
            // block will update the end variable to the maximum now_mu in the block.
            // Each function call directly inside a parallel block will reset the timeline after
            // execution. A parallel block within a sequential block (or not within any block) will
            // set the timeline to the max now_mu within the block (and the outer max now_mu will also
            // be updated).
            //
            // Implementation: We track the start and end separately.
            // - If there is a start variable, it indicates that we are directly inside a
            // parallel block and we have to reset the timeline after every function call.
            // - If there is a end variable, it indicates that we are (indirectly) inside a
            // parallel block, and we should update the max end value.
            if let ExprKind::Name { id, ctx: name_ctx } = &item.context_expr.node {
                if id == &"parallel".into() || id == &"legacy_parallel".into() {
                    let old_start = self.start.take();
                    let old_end = self.end.take();
                    let old_parallel_mode = self.parallel_mode;

                    let now = if let Some(old_start) = &old_start {
                        self.gen_expr(ctx, old_start)?
                            .unwrap()
                            .to_basic_value_enum(ctx, self, old_start.custom.unwrap())?
                    } else {
                        self.timeline.emit_now_mu(ctx)
                    };

                    // Emulate variable allocation, as we need to use the CodeGenContext
                    // HashMap to store our variable due to lifetime limitation
                    // Note: we should be able to store variables directly if generic
                    // associative type is used by limiting the lifetime of CodeGenerator to
                    // the LLVM Context.
                    // The name is guaranteed to be unique as users cannot use this as variable
                    // name.
                    self.start = old_start.clone().map_or_else(
                        || {
                            let start = format!("with-{}-start", self.name_counter).into();
                            let start_expr = Located {
                                // location does not matter at this point
                                location: stmt.location,
                                node: ExprKind::Name { id: start, ctx: name_ctx.clone() },
                                custom: Some(ctx.primitives.int64),
                            };
                            let start = self
                                .gen_store_target(ctx, &start_expr, Some("start.addr"))?
                                .unwrap();
                            ctx.builder.build_store(start, now).unwrap();
                            Ok(Some(start_expr)) as Result<_, String>
                        },
                        |v| Ok(Some(v)),
                    )?;
                    let end = format!("with-{}-end", self.name_counter).into();
                    let end_expr = Located {
                        // location does not matter at this point
                        location: stmt.location,
                        node: ExprKind::Name { id: end, ctx: name_ctx.clone() },
                        custom: Some(ctx.primitives.int64),
                    };
                    let end = self
                        .gen_store_target(ctx, &end_expr, Some("end.addr"))?
                        .unwrap();
                    ctx.builder.build_store(end, now).unwrap();
                    self.end = Some(end_expr);
                    self.name_counter += 1;
                    self.parallel_mode = match id.to_string().as_str() {
                        "parallel" => ParallelMode::Deep,
                        "legacy_parallel" => ParallelMode::Legacy,
                        _ => unreachable!(),
                    };

                    self.gen_block(ctx, body.iter())?;

                    let current = ctx.builder.get_insert_block().unwrap();

                    // if the current block is terminated, move before the terminator
                    // we want to set the timeline before reaching the terminator
                    // TODO: This may be unsound if there are multiple exit paths in the
                    // block... e.g.
                    // if ...:
                    //     return
                    // Perhaps we can fix this by using actual with block?
                    let reset_position = if let Some(terminator) = current.get_terminator() {
                        ctx.builder.position_before(&terminator);
                        true
                    } else {
                        false
                    };

                    // set duration
                    let end_expr = self.end.take().unwrap();
                    let end_val = self
                        .gen_expr(ctx, &end_expr)?
                        .unwrap()
                        .to_basic_value_enum(ctx, self, end_expr.custom.unwrap())?;

                    // inside a sequential block
                    if old_start.is_none() {
                        self.timeline.emit_at_mu(ctx, end_val);
                    }

                    // inside a parallel block, should update the outer max now_mu
                    self.timeline_update_end_max(ctx, old_end.clone(), Some("outer.end"))?;

                    self.parallel_mode = old_parallel_mode;
                    self.end = old_end;
                    self.start = old_start;

                    if reset_position {
                        ctx.builder.position_at_end(current);
                    }

                    return Ok(());
                } else if id == &"sequential".into() {
                    // For deep parallel, temporarily take away start to avoid function calls in
                    // the block from resetting the timeline.
                    // This does not affect legacy parallel, as the timeline will be reset after
                    // this block finishes execution.
                    let start = self.start.take();
                    self.gen_block(ctx, body.iter())?;
                    self.start = start;

                    // Reset the timeline when we are exiting the sequential block
                    // Legacy parallel does not need this, since it will be reset after codegen
                    // for this statement is completed
                    if self.parallel_mode == ParallelMode::Deep {
                        self.timeline_reset_start(ctx)?;
                    }

                    return Ok(());
                }
            }
        }

        // not parallel/sequential
        gen_with(self, ctx, stmt)
    }
}

fn gen_rpc_tag(
    ctx: &mut CodeGenContext<'_, '_>,
    ty: Type,
    buffer: &mut Vec<u8>,
) -> Result<(), String> {
    use nac3core::typecheck::typedef::TypeEnum::*;

    let int32 = ctx.primitives.int32;
    let int64 = ctx.primitives.int64;
    let float = ctx.primitives.float;
    let bool = ctx.primitives.bool;
    let str = ctx.primitives.str;
    let none = ctx.primitives.none;

    if ctx.unifier.unioned(ty, int32) {
        buffer.push(b'i');
    } else if ctx.unifier.unioned(ty, int64) {
        buffer.push(b'I');
    } else if ctx.unifier.unioned(ty, float) {
        buffer.push(b'f');
    } else if ctx.unifier.unioned(ty, bool) {
        buffer.push(b'b');
    } else if ctx.unifier.unioned(ty, str) {
        buffer.push(b's');
    } else if ctx.unifier.unioned(ty, none) {
        buffer.push(b'n');
    } else {
        let ty_enum = ctx.unifier.get_ty(ty);
        match &*ty_enum {
            TTuple { ty } => {
                buffer.push(b't');
                buffer.push(ty.len() as u8);
                for ty in ty {
                    gen_rpc_tag(ctx, *ty, buffer)?;
                }
            }
            TList { ty } => {
                buffer.push(b'l');
                gen_rpc_tag(ctx, *ty, buffer)?;
            }
            TNDArray { .. } => {
                todo!()
            }
            _ => return Err(format!("Unsupported type: {:?}", ctx.unifier.stringify(ty))),
        }
    }
    Ok(())
}

fn rpc_codegen_callback_fn<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    obj: Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    generator: &mut dyn CodeGenerator,
) -> Result<Option<BasicValueEnum<'ctx>>, String> {
    let ptr_type = ctx.ctx.i8_type().ptr_type(AddressSpace::default());
    let size_type = generator.get_size_type(ctx.ctx);
    let int8 = ctx.ctx.i8_type();
    let int32 = ctx.ctx.i32_type();
    let tag_ptr_type = ctx.ctx.struct_type(&[ptr_type.into(), size_type.into()], false);

    let service_id = int32.const_int(fun.1.0 as u64, false);
    // -- setup rpc tags
    let mut tag = Vec::new();
    if obj.is_some() {
        tag.push(b'O');
    }
    for arg in &fun.0.args {
        gen_rpc_tag(ctx, arg.ty, &mut tag)?;
    }
    tag.push(b':');
    gen_rpc_tag(ctx, fun.0.ret, &mut tag)?;

    let mut hasher = DefaultHasher::new();
    tag.hash(&mut hasher);
    let hash = format!("{}", hasher.finish());

    let tag_ptr = ctx
        .module
        .get_global(hash.as_str())
        .unwrap_or_else(|| {
            let tag_arr_ptr = ctx.module.add_global(
                int8.array_type(tag.len() as u32),
                None,
                format!("tagptr{}", fun.1 .0).as_str(),
            );
            tag_arr_ptr.set_initializer(&int8.const_array(
                &tag.iter().map(|v| int8.const_int(*v as u64, false)).collect::<Vec<_>>(),
            ));
            tag_arr_ptr.set_linkage(Linkage::Private);
            let tag_ptr = ctx.module.add_global(tag_ptr_type, None, &hash);
            tag_ptr.set_linkage(Linkage::Private);
            tag_ptr.set_initializer(&ctx.ctx.const_struct(
                &[
                    tag_arr_ptr.as_pointer_value().const_cast(ptr_type).into(),
                    size_type.const_int(tag.len() as u64, false).into(),
                ],
                false,
            ));
            tag_ptr
        })
        .as_pointer_value();

    let arg_length = args.len() + usize::from(obj.is_some());

    let stackptr = call_stacksave(ctx, Some("rpc.stack"));
    let args_ptr = ctx.builder
        .build_array_alloca(
            ptr_type,
            ctx.ctx.i32_type().const_int(arg_length as u64, false),
            "argptr",
        )
        .unwrap();

    // -- rpc args handling
    let mut keys = fun.0.args.clone();
    let mut mapping = HashMap::new();
    for (key, value) in args {
        mapping.insert(key.unwrap_or_else(|| keys.remove(0).name), value);
    }
    // default value handling
    for k in keys {
        mapping.insert(
            k.name,
            ctx.gen_symbol_val(generator, &k.default_value.unwrap(), k.ty).into()
        );
    }
    // reorder the parameters
    let mut real_params = fun
        .0
        .args
        .iter()
        .map(|arg| mapping.remove(&arg.name).unwrap().to_basic_value_enum(ctx, generator, arg.ty))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(obj) = obj {
        if let ValueEnum::Static(obj) = obj.1 {
            real_params.insert(0, obj.get_const_obj(ctx, generator));
        } else {
            // should be an error here...
            panic!("only host object is allowed");
        }
    }

    for (i, arg) in real_params.iter().enumerate() {
        let arg_slot = generator.gen_var_alloc(ctx, arg.get_type(), Some(&format!("rpc.arg{i}"))).unwrap();
        ctx.builder.build_store(arg_slot, *arg).unwrap();
        let arg_slot = ctx.builder.build_bitcast(arg_slot, ptr_type, "rpc.arg").unwrap();
        let arg_ptr = unsafe {
            ctx.builder.build_gep(
                args_ptr,
                &[int32.const_int(i as u64, false)],
                &format!("rpc.arg{i}"),
            )
        }.unwrap();
        ctx.builder.build_store(arg_ptr, arg_slot).unwrap();
    }

    // call
    let rpc_send = ctx.module.get_function("rpc_send").unwrap_or_else(|| {
        ctx.module.add_function(
            "rpc_send",
            ctx.ctx.void_type().fn_type(
                &[
                    int32.into(),
                    tag_ptr_type.ptr_type(AddressSpace::default()).into(),
                    ptr_type.ptr_type(AddressSpace::default()).into(),
                ],
                false,
            ),
            None,
        )
    });
    ctx.builder
        .build_call(
            rpc_send,
            &[service_id.into(), tag_ptr.into(), args_ptr.into()],
            "rpc.send",
        )
        .unwrap();

    // reclaim stack space used by arguments
    call_stackrestore(ctx, stackptr);

    // -- receive value:
    // T result = {
    //   void *ret_ptr = alloca(sizeof(T));
    //   void *ptr = ret_ptr;
    //   loop: int size = rpc_recv(ptr);
    //   // Non-zero: Provide `size` bytes of extra storage for variable-length data.
    //   if(size) { ptr = alloca(size); goto loop; }
    //   else *(T*)ret_ptr
    // }
    let rpc_recv = ctx.module.get_function("rpc_recv").unwrap_or_else(|| {
        ctx.module.add_function("rpc_recv", int32.fn_type(&[ptr_type.into()], false), None)
    });

    if ctx.unifier.unioned(fun.0.ret, ctx.primitives.none) {
        ctx.build_call_or_invoke(rpc_recv, &[ptr_type.const_null().into()], "rpc_recv");
        return Ok(None);
    }

    let prehead_bb = ctx.builder.get_insert_block().unwrap();
    let current_function = prehead_bb.get_parent().unwrap();
    let head_bb = ctx.ctx.append_basic_block(current_function, "rpc.head");
    let alloc_bb = ctx.ctx.append_basic_block(current_function, "rpc.continue");
    let tail_bb = ctx.ctx.append_basic_block(current_function, "rpc.tail");

    let ret_ty = ctx.get_llvm_abi_type(generator, fun.0.ret);
    let need_load = !ret_ty.is_pointer_type();
    let slot = ctx.builder.build_alloca(ret_ty, "rpc.ret.slot").unwrap();
    let slotgen = ctx.builder.build_bitcast(slot, ptr_type, "rpc.ret.ptr").unwrap();
    ctx.builder.build_unconditional_branch(head_bb).unwrap();
    ctx.builder.position_at_end(head_bb);

    let phi = ctx.builder.build_phi(ptr_type, "rpc.ptr").unwrap();
    phi.add_incoming(&[(&slotgen, prehead_bb)]);
    let alloc_size = ctx
        .build_call_or_invoke(rpc_recv, &[phi.as_basic_value()], "rpc.size.next")
        .unwrap()
        .into_int_value();
    let is_done = ctx.builder
        .build_int_compare(
            inkwell::IntPredicate::EQ,
            int32.const_zero(),
            alloc_size,
            "rpc.done",
        )
        .unwrap();

    ctx.builder.build_conditional_branch(is_done, tail_bb, alloc_bb).unwrap();
    ctx.builder.position_at_end(alloc_bb);

    let alloc_ptr = ctx.builder.build_array_alloca(ptr_type, alloc_size, "rpc.alloc").unwrap();
    let alloc_ptr = ctx.builder.build_bitcast(alloc_ptr, ptr_type, "rpc.alloc.ptr").unwrap();
    phi.add_incoming(&[(&alloc_ptr, alloc_bb)]);
    ctx.builder.build_unconditional_branch(head_bb).unwrap();

    ctx.builder.position_at_end(tail_bb);

    let result = ctx.builder.build_load(slot, "rpc.result").unwrap();
    if need_load {
        call_stackrestore(ctx, stackptr);
    }
    Ok(Some(result))
}

pub fn attributes_writeback(
    ctx: &mut CodeGenContext<'_, '_>,
    generator: &mut dyn CodeGenerator,
    inner_resolver: &InnerResolver,
    host_attributes: &PyObject,
) -> Result<(), String> {
    Python::with_gil(|py| -> PyResult<Result<(), String>> {
        let host_attributes: &PyList = host_attributes.downcast(py)?;
        let top_levels = ctx.top_level.definitions.read();
        let globals = inner_resolver.global_value_ids.read();
        let int32 = ctx.ctx.i32_type();
        let zero = int32.const_zero();
        let mut values = Vec::new();
        let mut scratch_buffer = Vec::new();
        for val in (*globals).values() {
            let val = val.as_ref(py);
            let ty = inner_resolver.get_obj_type(py, val, &mut ctx.unifier, &top_levels, &ctx.primitives)?;
            if let Err(ty) = ty {
                return Ok(Err(ty))
            }
            let ty = ty.unwrap();
            match &*ctx.unifier.get_ty(ty) {
                TypeEnum::TObj { fields, obj_id, .. }
                    if *obj_id != ctx.primitives.option.get_obj_id(&ctx.unifier) =>
                {
                    // we only care about primitive attributes
                    // for non-primitive attributes, they should be in another global
                    let mut attributes = Vec::new();
                    let obj = inner_resolver.get_obj_value(py, val, ctx, generator, ty)?.unwrap();
                    for (name, (field_ty, is_mutable)) in fields {
                        if !is_mutable {
                            continue
                        }
                        if gen_rpc_tag(ctx, *field_ty, &mut scratch_buffer).is_ok() {
                            attributes.push(name.to_string());
                            let index = ctx.get_attr_index(ty, *name);
                            values.push((*field_ty, ctx.build_gep_and_load(
                                            obj.into_pointer_value(),
                                            &[zero, int32.const_int(index as u64, false)], None)));
                        }
                    }
                    if !attributes.is_empty() {
                        let pydict = PyDict::new(py);
                        pydict.set_item("obj", val)?;
                        pydict.set_item("fields", attributes)?;
                        host_attributes.append(pydict)?;
                    }
                },
                TypeEnum::TList { ty: elem_ty } => {
                    if gen_rpc_tag(ctx, *elem_ty, &mut scratch_buffer).is_ok() {
                        let pydict = PyDict::new(py);
                        pydict.set_item("obj", val)?;
                        host_attributes.append(pydict)?;
                        values.push((ty, inner_resolver.get_obj_value(py, val, ctx, generator, ty)?.unwrap()));
                    }
                },
                TypeEnum::TNDArray { ty: elem_ty, .. } => {
                    if gen_rpc_tag(ctx, *elem_ty, &mut scratch_buffer).is_ok() {
                        let pydict = PyDict::new(py);
                        pydict.set_item("obj", val)?;
                        host_attributes.append(pydict)?;
                        values.push((ty, inner_resolver.get_obj_value(py, val, ctx, generator, ty)?.unwrap()));
                    }
                },
                _ => {}
            }
        }
        let fun = FunSignature {
            args: values.iter().enumerate().map(|(i, (ty, _))| FuncArg {
                name: i.to_string().into(),
                ty: *ty,
                default_value: None
            }).collect(),
            ret: ctx.primitives.none,
            vars: HashMap::default()
        };
        let args: Vec<_> = values.into_iter().map(|(_, val)| (None, ValueEnum::Dynamic(val))).collect();
        if let Err(e) = rpc_codegen_callback_fn(ctx, None, (&fun, DefinitionId(0)), args, generator) {
            return Ok(Err(e));
        }
        Ok(Ok(()))
    }).unwrap()?;
    Ok(())
}

pub fn rpc_codegen_callback() -> Arc<GenCall> {
    Arc::new(GenCall::new(Box::new(|ctx, obj, fun, args, generator| {
        rpc_codegen_callback_fn(ctx, obj, fun, args, generator)
    })))
}
