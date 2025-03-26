use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    iter::once,
    mem,
    sync::Arc,
};

use itertools::Itertools;
use pyo3::{
    types::{PyDict, PyList},
    PyObject, PyResult, Python,
};

use super::{symbol_resolver::InnerResolver, timeline::TimeFns, SpecialPythonId};
use nac3core::{
    codegen::{
        expr::{create_fn_and_call, destructure_range, gen_call, infer_and_call_function},
        llvm_intrinsics::{call_int_smax, call_memcpy, call_stackrestore, call_stacksave},
        stmt::{gen_block, gen_for_callback_incrementing, gen_if_callback, gen_with},
        type_aligned_alloca,
        types::{ndarray::NDArrayType, RangeType},
        values::{
            ArrayLikeIndexer, ArrayLikeValue, ArraySliceValue, ListValue, ProxyValue,
            UntypedArrayLikeAccessor,
        },
        CodeGenContext, CodeGenerator,
    },
    inkwell::{
        context::Context,
        module::Linkage,
        targets::TargetMachine,
        types::{BasicType, IntType},
        values::{BasicValueEnum, IntValue, PointerValue, StructValue},
        AddressSpace, IntPredicate, OptimizationLevel,
    },
    nac3parser::ast::{Expr, ExprKind, Located, Stmt, StmtKind, StrRef},
    symbol_resolver::ValueEnum,
    toplevel::{
        helper::{extract_ndims, PrimDef},
        numpy::unpack_ndarray_var_tys,
        DefinitionId, GenCall,
    },
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{iter_type_vars, FunSignature, FuncArg, Type, TypeEnum, VarMap},
    },
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
    Deep,
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

    /// The [`ParallelMode`] of the current parallel context.
    ///
    /// The current parallel context refers to the nearest `with parallel` or `with legacy_parallel`
    /// statement, which is used to determine when and how the timeline should be updated.
    parallel_mode: ParallelMode,

    /// Specially treated python IDs to identify `with parallel` and `with sequential` blocks.
    special_ids: SpecialPythonId,
}

impl<'a> ArtiqCodeGenerator<'a> {
    pub fn new(
        name: String,
        size_t: IntType<'_>,
        timeline: &'a (dyn TimeFns + Sync),
        special_ids: SpecialPythonId,
    ) -> ArtiqCodeGenerator<'a> {
        assert!(matches!(size_t.get_bit_width(), 32 | 64));
        ArtiqCodeGenerator {
            name,
            size_t: size_t.get_bit_width(),
            name_counter: 0,
            start: None,
            end: None,
            timeline,
            parallel_mode: ParallelMode::None,
            special_ids,
        }
    }

    #[must_use]
    pub fn with_target_machine(
        name: String,
        ctx: &Context,
        target_machine: &TargetMachine,
        timeline: &'a (dyn TimeFns + Sync),
        special_ids: SpecialPythonId,
    ) -> ArtiqCodeGenerator<'a> {
        let llvm_usize = ctx.ptr_sized_int_type(&target_machine.get_target_data(), None);
        Self::new(name, llvm_usize, timeline, special_ids)
    }

    /// If the generator is currently in a direct-`parallel` block context, emits IR that resets the
    /// position of the timeline to the initial timeline position before entering the `parallel`
    /// block.
    ///
    /// Direct-`parallel` block context refers to when the generator is generating statements whose
    /// closest parent `with` statement is a `with parallel` block.
    fn timeline_reset_start(&mut self, ctx: &mut CodeGenContext<'_, '_>) -> Result<(), String> {
        if let Some(start) = self.start.clone() {
            let start_val = self.gen_expr(ctx, &start)?.unwrap().to_basic_value_enum(
                ctx,
                self,
                start.custom.unwrap(),
            )?;
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
    ///   the end of the provided value name.
    fn timeline_update_end_max(
        &mut self,
        ctx: &mut CodeGenContext<'_, '_>,
        end: Option<Expr<Option<Type>>>,
        store_name: Option<&str>,
    ) -> Result<(), String> {
        if let Some(end) = end {
            let old_end = self.gen_expr(ctx, &end)?.unwrap().to_basic_value_enum(
                ctx,
                self,
                end.custom.unwrap(),
            )?;
            let now = self.timeline.emit_now_mu(ctx);
            let max =
                call_int_smax(ctx, old_end.into_int_value(), now.into_int_value(), Some("smax"));
            let end_store = self
                .gen_store_target(
                    ctx,
                    &end,
                    store_name.map(|name| format!("{name}.addr")).as_deref(),
                )?
                .unwrap();
            ctx.builder.build_store(end_store, max).unwrap();
        }

        Ok(())
    }
}

impl CodeGenerator for ArtiqCodeGenerator<'_> {
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

    fn gen_block<'ctx, 'a, 'c, I: Iterator<Item = &'c Stmt<Option<Type>>>>(
        &mut self,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        stmts: I,
    ) -> Result<(), String>
    where
        Self: Sized,
    {
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
        let StmtKind::With { items, body, .. } = &stmt.node else { unreachable!() };

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
                let resolver = ctx.resolver.clone();
                if let Some(static_value) =
                    if let Some((_ptr, static_value, _counter)) = ctx.var_assignment.get(id) {
                        static_value.clone()
                    } else if let Some(ValueEnum::Static(val)) =
                        resolver.get_symbol_value(*id, ctx, self)
                    {
                        Some(val)
                    } else {
                        None
                    }
                {
                    let python_id = static_value.get_unique_identifier();
                    if python_id == self.special_ids.parallel
                        || python_id == self.special_ids.legacy_parallel
                    {
                        let old_start = self.start.take();
                        let old_end = self.end.take();
                        let old_parallel_mode = self.parallel_mode;

                        let now = if let Some(old_start) = &old_start {
                            self.gen_expr(ctx, old_start)?.unwrap().to_basic_value_enum(
                                ctx,
                                self,
                                old_start.custom.unwrap(),
                            )?
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
                                    node: ExprKind::Name { id: start, ctx: *name_ctx },
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
                            node: ExprKind::Name { id: end, ctx: *name_ctx },
                            custom: Some(ctx.primitives.int64),
                        };
                        let end = self.gen_store_target(ctx, &end_expr, Some("end.addr"))?.unwrap();
                        ctx.builder.build_store(end, now).unwrap();
                        self.end = Some(end_expr);
                        self.name_counter += 1;
                        self.parallel_mode = if python_id == self.special_ids.parallel {
                            ParallelMode::Deep
                        } else if python_id == self.special_ids.legacy_parallel {
                            ParallelMode::Legacy
                        } else {
                            unreachable!()
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
                        let end_val = self.gen_expr(ctx, &end_expr)?.unwrap().to_basic_value_enum(
                            ctx,
                            self,
                            end_expr.custom.unwrap(),
                        )?;

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
                    } else if python_id == self.special_ids.sequential {
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

    let PrimitiveStore { int32, int64, float, bool, str, none, .. } = ctx.primitives;

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
            TTuple { ty, is_vararg_ctx: false } => {
                buffer.push(b't');
                buffer.push(ty.len() as u8);
                for ty in ty {
                    gen_rpc_tag(ctx, *ty, buffer)?;
                }
            }
            TObj { obj_id, params, .. } if *obj_id == PrimDef::List.id() => {
                let ty = iter_type_vars(params).next().unwrap().ty;

                buffer.push(b'l');
                gen_rpc_tag(ctx, ty, buffer)?;
            }
            TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
                let (ndarray_dtype, ndarray_ndims) = unpack_ndarray_var_tys(&mut ctx.unifier, ty);
                let ndarray_ndims = if let TLiteral { values, .. } =
                    &*ctx.unifier.get_ty_immutable(ndarray_ndims)
                {
                    if values.len() != 1 {
                        return Err(format!("NDArray types with multiple literal bounds for ndims is not supported: {}", ctx.unifier.stringify(ty)));
                    }

                    let value = values[0].clone();
                    u64::try_from(value.clone())
                        .map_err(|()| format!("Expected u64 for ndarray.ndims, got {value}"))?
                } else {
                    unreachable!()
                };
                assert!(
                    (0u64..=u64::from(u8::MAX)).contains(&ndarray_ndims),
                    "Only NDArrays of sizes between 0 and 255 can be RPCed"
                );

                buffer.push(b'a');
                buffer.push((ndarray_ndims & 0xFF) as u8);
                gen_rpc_tag(ctx, ndarray_dtype, buffer)?;
            }
            _ => return Err(format!("Unsupported type: {:?}", ctx.unifier.stringify(ty))),
        }
    }
    Ok(())
}

/// Formats an RPC argument to conform to the expected format required by `send_value`.
///
/// See `artiq/firmware/libproto_artiq/rpc_proto.rs` for the expected format.
fn format_rpc_arg<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (arg, arg_ty, arg_idx): (BasicValueEnum<'ctx>, Type, usize),
) -> PointerValue<'ctx> {
    let llvm_i8 = ctx.ctx.i8_type();
    let llvm_pi8 = llvm_i8.ptr_type(AddressSpace::default());

    let arg_slot = match &*ctx.unifier.get_ty_immutable(arg_ty) {
        TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
            // NAC3: NDArray = { usize, usize*, T* }
            // libproto_artiq: NDArray = [data[..], dim_sz[..]]

            let llvm_usize = ctx.get_size_type();

            let (elem_ty, ndims) = unpack_ndarray_var_tys(&mut ctx.unifier, arg_ty);
            let ndims = extract_ndims(&ctx.unifier, ndims);
            let dtype = ctx.get_llvm_type(generator, elem_ty);
            let ndarray = NDArrayType::new(ctx, dtype, ndims)
                .map_pointer_value(arg.into_pointer_value(), None);

            let ndims = llvm_usize.const_int(ndims, false);

            // `ndarray.data` is possibly not contiguous, and we need it to be contiguous for
            // the reader.
            // Turning it into a ContiguousNDArray to get a `data` that is contiguous.
            let carray = ndarray.make_contiguous_ndarray(generator, ctx);

            let sizeof_usize = llvm_usize.size_of();
            let sizeof_usize =
                ctx.builder.build_int_truncate_or_bit_cast(sizeof_usize, llvm_usize, "").unwrap();

            let sizeof_pdata = dtype.ptr_type(AddressSpace::default()).size_of();
            let sizeof_pdata =
                ctx.builder.build_int_truncate_or_bit_cast(sizeof_pdata, llvm_usize, "").unwrap();

            let sizeof_buf_shape = ctx.builder.build_int_mul(sizeof_usize, ndims, "").unwrap();
            let sizeof_buf = ctx.builder.build_int_add(sizeof_buf_shape, sizeof_pdata, "").unwrap();

            // buf = { data: void*, shape: [size_t; ndims]; }
            let buf = ctx.builder.build_array_alloca(llvm_i8, sizeof_buf, "rpc.arg").unwrap();
            let buf = ArraySliceValue::from_ptr_val(buf, sizeof_buf, Some("rpc.arg"));
            let buf_data = buf.base_ptr(ctx, generator);
            let buf_shape =
                unsafe { buf.ptr_offset_unchecked(ctx, generator, &sizeof_pdata, None) };

            // Write to `buf->data`
            let carray_data = carray.load_data(ctx);
            let carray_data = ctx.builder.build_pointer_cast(carray_data, llvm_pi8, "").unwrap();
            call_memcpy(ctx, buf_data, carray_data, sizeof_pdata);

            // Write to `buf->shape`
            let carray_shape = ndarray.shape().base_ptr(ctx, generator);
            let carray_shape_i8 =
                ctx.builder.build_pointer_cast(carray_shape, llvm_pi8, "").unwrap();
            call_memcpy(ctx, buf_shape, carray_shape_i8, sizeof_buf_shape);

            buf.base_ptr(ctx, generator)
        }

        _ => {
            let arg_slot = generator
                .gen_var_alloc(ctx, arg.get_type(), Some(&format!("rpc.arg{arg_idx}")))
                .unwrap();
            ctx.builder.build_store(arg_slot, arg).unwrap();

            ctx.builder
                .build_bit_cast(arg_slot, llvm_pi8, "rpc.arg")
                .map(BasicValueEnum::into_pointer_value)
                .unwrap()
        }
    };

    debug_assert_eq!(arg_slot.get_type(), llvm_pi8);

    arg_slot
}

/// Formats an RPC return value to conform to the expected format required by NAC3.
fn format_rpc_ret<'ctx>(
    generator: &mut dyn CodeGenerator,
    ctx: &mut CodeGenContext<'ctx, '_>,
    ret_ty: Type,
) -> Option<BasicValueEnum<'ctx>> {
    // -- receive value:
    // T result = {
    //   void *ret_ptr = alloca(sizeof(T));
    //   void *ptr = ret_ptr;
    //   loop: int size = rpc_recv(ptr);
    //   // Non-zero: Provide `size` bytes of extra storage for variable-length data.
    //   if(size) { ptr = alloca(size); goto loop; }
    //   else *(T*)ret_ptr
    // }

    let llvm_i8 = ctx.ctx.i8_type();
    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_i8_8 = ctx.ctx.struct_type(&[llvm_i8.array_type(8).into()], false);
    let llvm_pi8 = llvm_i8.ptr_type(AddressSpace::default());
    let llvm_usize = ctx.get_size_type();
    let llvm_pusize = llvm_usize.ptr_type(AddressSpace::default());

    let rpc_recv = ctx.module.get_function("rpc_recv").unwrap_or_else(|| {
        ctx.module.add_function("rpc_recv", llvm_i32.fn_type(&[llvm_pi8.into()], false), None)
    });

    if ctx.unifier.unioned(ret_ty, ctx.primitives.none) {
        ctx.build_call_or_invoke(rpc_recv, &[llvm_pi8.const_null().into()], "rpc_recv");
        return None;
    }

    let prehead_bb = ctx.builder.get_insert_block().unwrap();
    let current_function = prehead_bb.get_parent().unwrap();
    let head_bb = ctx.ctx.append_basic_block(current_function, "rpc.head");
    let alloc_bb = ctx.ctx.append_basic_block(current_function, "rpc.continue");
    let tail_bb = ctx.ctx.append_basic_block(current_function, "rpc.tail");

    let llvm_ret_ty = ctx.get_llvm_abi_type(generator, ret_ty);

    let result = match &*ctx.unifier.get_ty_immutable(ret_ty) {
        TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
            let num_0 = llvm_usize.const_zero();

            // Round `val` up to its modulo `power_of_two`
            let round_up = |ctx: &mut CodeGenContext<'ctx, '_>,
                            val: IntValue<'ctx>,
                            power_of_two: IntValue<'ctx>| {
                debug_assert_eq!(
                    val.get_type().get_bit_width(),
                    power_of_two.get_type().get_bit_width()
                );

                let llvm_val_t = val.get_type();

                let max_rem = ctx
                    .builder
                    .build_int_sub(power_of_two, llvm_val_t.const_int(1, false), "")
                    .unwrap();
                ctx.builder
                    .build_and(
                        ctx.builder.build_int_add(val, max_rem, "").unwrap(),
                        ctx.builder.build_not(max_rem, "").unwrap(),
                        "",
                    )
                    .unwrap()
            };

            // Allocate the resulting ndarray
            // A condition after format_rpc_ret ensures this will not be popped this off.
            let (dtype, ndims) = unpack_ndarray_var_tys(&mut ctx.unifier, ret_ty);
            let dtype_llvm = ctx.get_llvm_type(generator, dtype);
            let ndims = extract_ndims(&ctx.unifier, ndims);
            let ndarray = NDArrayType::new(ctx, dtype_llvm, ndims)
                .construct_uninitialized(generator, ctx, None);

            // NOTE: Current content of `ndarray`:
            //   - * `data` - **NOT YET** allocated.
            //   - * `itemsize` - initialized to be size_of(dtype).
            //   - * `ndims` - initialized.
            //   - * `shape` - allocated; has uninitialized values.
            //   - * `strides` - allocated; has uninitialized values.

            let itemsize = ndarray.load_itemsize(ctx); // Same as doing a `ctx.get_llvm_type` on `dtype` and get its `size_of()`.

            // Allocates a buffer for the initial RPC'ed object, which is guaranteed to be
            // (4 + 4 * ndims) bytes with 8-byte alignment
            let sizeof_usize = llvm_usize.size_of();
            let sizeof_usize =
                ctx.builder.build_int_truncate_or_bit_cast(sizeof_usize, llvm_usize, "").unwrap();

            let sizeof_ptr = llvm_i8.ptr_type(AddressSpace::default()).size_of();
            let sizeof_ptr =
                ctx.builder.build_int_z_extend_or_bit_cast(sizeof_ptr, llvm_usize, "").unwrap();

            let sizeof_shape =
                ctx.builder.build_int_mul(ndarray.load_ndims(ctx), sizeof_usize, "").unwrap();

            // Size of the buffer for the initial `rpc_recv()`.
            let unaligned_buffer_size =
                ctx.builder.build_int_add(sizeof_ptr, sizeof_shape, "").unwrap();

            let stackptr = call_stacksave(ctx, None);
            let buffer = type_aligned_alloca(
                generator,
                ctx,
                llvm_i8_8,
                unaligned_buffer_size,
                Some("rpc.buffer"),
            );
            let buffer = ArraySliceValue::from_ptr_val(buffer, unaligned_buffer_size, None);

            // The first call to `rpc_recv` reads the top-level ndarray object: [pdata, shape]
            //
            // The returned value is the number of bytes for `ndarray.data`.
            let ndarray_nbytes = ctx
                .build_call_or_invoke(
                    rpc_recv,
                    &[buffer.base_ptr(ctx, generator).into()], // Reads [usize; ndims]
                    "rpc.size.next",
                )
                .map(BasicValueEnum::into_int_value)
                .unwrap();

            // debug_assert(ndarray_nbytes > 0)
            if ctx.registry.llvm_options.opt_level == OptimizationLevel::None {
                let cmp = ctx
                    .builder
                    .build_int_compare(IntPredicate::UGT, ndarray_nbytes, num_0, "")
                    .unwrap();

                ctx.make_assert(
                    generator,
                    cmp,
                    "0:AssertionError",
                    "Unexpected RPC termination for ndarray - Expected data buffer next",
                    [None, None, None],
                    ctx.current_loc,
                );
            }

            // Copy shape from the buffer to `ndarray.shape`.
            // We need to skip the first `sizeof(uint8_t*)` bytes to skip the `pdata` in `[pdata, shape]`.
            let pbuffer_shape =
                unsafe { buffer.ptr_offset_unchecked(ctx, generator, &sizeof_ptr, None) };
            let pbuffer_shape =
                ctx.builder.build_pointer_cast(pbuffer_shape, llvm_pusize, "").unwrap();

            // Copy shape from buffer to `ndarray.shape`
            ndarray.copy_shape_from_array(generator, ctx, pbuffer_shape);

            // Restore stack from before allocation of buffer
            call_stackrestore(ctx, stackptr);

            // Allocate `ndarray.data`.
            // `ndarray.shape` must be initialized beforehand in this implementation
            //   (for ndarray.create_data() to know how many elements to allocate)
            unsafe { ndarray.create_data(generator, ctx) }; // NOTE: the strides of `ndarray` has also been set to contiguous in `create_data`.

            // debug_assert(nelems * sizeof(T) >= ndarray_nbytes)
            if ctx.registry.llvm_options.opt_level == OptimizationLevel::None {
                let num_elements = ndarray.size(ctx);

                let expected_ndarray_nbytes =
                    ctx.builder.build_int_mul(num_elements, itemsize, "").unwrap();
                let cmp = ctx
                    .builder
                    .build_int_compare(
                        IntPredicate::UGE,
                        expected_ndarray_nbytes,
                        ndarray_nbytes,
                        "",
                    )
                    .unwrap();

                ctx.make_assert(
                    generator,
                    cmp,
                    "0:AssertionError",
                    "Unexpected allocation size request for ndarray data - Expected up to {0} bytes, got {1} bytes",
                    [Some(expected_ndarray_nbytes), Some(ndarray_nbytes), None],
                    ctx.current_loc,
                );
            }

            let ndarray_data = ndarray.data().base_ptr(ctx, generator);

            // NOTE: Currently on `prehead_bb`
            ctx.builder.build_unconditional_branch(head_bb).unwrap();

            // Inserting into `head_bb`. Do `rpc_recv` for `data` recursively.
            ctx.builder.position_at_end(head_bb);

            let phi = ctx.builder.build_phi(llvm_pi8, "rpc.ptr").unwrap();
            phi.add_incoming(&[(&ndarray_data, prehead_bb)]);

            let alloc_size = ctx
                .build_call_or_invoke(rpc_recv, &[phi.as_basic_value()], "rpc.size.next")
                .map(BasicValueEnum::into_int_value)
                .unwrap();

            let is_done = ctx
                .builder
                .build_int_compare(IntPredicate::EQ, llvm_i32.const_zero(), alloc_size, "rpc.done")
                .unwrap();
            ctx.builder.build_conditional_branch(is_done, tail_bb, alloc_bb).unwrap();

            ctx.builder.position_at_end(alloc_bb);
            // Align the allocation to sizeof(T)
            let alloc_size = round_up(ctx, alloc_size, itemsize);
            // TODO(Derppening): Candidate for refactor into type_aligned_alloca
            let alloc_ptr = ctx
                .builder
                .build_array_alloca(
                    dtype_llvm,
                    ctx.builder.build_int_unsigned_div(alloc_size, itemsize, "").unwrap(),
                    "rpc.alloc",
                )
                .unwrap();
            let alloc_ptr =
                ctx.builder.build_pointer_cast(alloc_ptr, llvm_pi8, "rpc.alloc.ptr").unwrap();
            phi.add_incoming(&[(&alloc_ptr, alloc_bb)]);
            ctx.builder.build_unconditional_branch(head_bb).unwrap();

            ctx.builder.position_at_end(tail_bb);
            ndarray.as_abi_value(ctx).into()
        }

        _ => {
            let slot = ctx.builder.build_alloca(llvm_ret_ty, "rpc.ret.slot").unwrap();
            let slotgen = ctx.builder.build_bit_cast(slot, llvm_pi8, "rpc.ret.ptr").unwrap();
            ctx.builder.build_unconditional_branch(head_bb).unwrap();
            ctx.builder.position_at_end(head_bb);

            let phi = ctx.builder.build_phi(llvm_pi8, "rpc.ptr").unwrap();
            phi.add_incoming(&[(&slotgen, prehead_bb)]);
            let alloc_size = ctx
                .build_call_or_invoke(rpc_recv, &[phi.as_basic_value()], "rpc.size.next")
                .unwrap()
                .into_int_value();
            let is_done = ctx
                .builder
                .build_int_compare(IntPredicate::EQ, llvm_i32.const_zero(), alloc_size, "rpc.done")
                .unwrap();

            ctx.builder.build_conditional_branch(is_done, tail_bb, alloc_bb).unwrap();
            ctx.builder.position_at_end(alloc_bb);

            let alloc_ptr =
                ctx.builder.build_array_alloca(llvm_pi8, alloc_size, "rpc.alloc").unwrap();
            let alloc_ptr =
                ctx.builder.build_bit_cast(alloc_ptr, llvm_pi8, "rpc.alloc.ptr").unwrap();
            phi.add_incoming(&[(&alloc_ptr, alloc_bb)]);
            ctx.builder.build_unconditional_branch(head_bb).unwrap();

            ctx.builder.position_at_end(tail_bb);
            ctx.builder.build_load(slot, "rpc.result").unwrap()
        }
    };

    Some(result)
}

fn rpc_codegen_callback_fn<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    obj: Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    generator: &mut dyn CodeGenerator,
    is_async: bool,
) -> Result<Option<BasicValueEnum<'ctx>>, String> {
    let int8 = ctx.ctx.i8_type();
    let int32 = ctx.ctx.i32_type();
    let size_type = ctx.get_size_type();
    let ptr_type = int8.ptr_type(AddressSpace::default());
    let tag_ptr_type = ctx.ctx.struct_type(&[ptr_type.into(), size_type.into()], false);

    let service_id = int32.const_int(fun.1 .0 as u64, false);
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
                &tag.iter().map(|v| int8.const_int(u64::from(*v), false)).collect::<Vec<_>>(),
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
    let args_ptr = ctx
        .builder
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
        mapping
            .insert(k.name, ctx.gen_symbol_val(generator, &k.default_value.unwrap(), k.ty).into());
    }
    // reorder the parameters
    let mut real_params = fun
        .0
        .args
        .iter()
        .map(|arg| {
            mapping
                .remove(&arg.name)
                .unwrap()
                .to_basic_value_enum(ctx, generator, arg.ty)
                .map(|llvm_val| (llvm_val, arg.ty))
        })
        .collect::<Result<Vec<(_, _)>, _>>()?;
    if let Some(obj) = obj {
        if let ValueEnum::Static(obj_val) = obj.1 {
            real_params.insert(0, (obj_val.get_const_obj(ctx, generator), obj.0));
        } else {
            // should be an error here...
            panic!("only host object is allowed");
        }
    }

    for (i, (arg, arg_ty)) in real_params.iter().enumerate() {
        let arg_slot = format_rpc_arg(generator, ctx, (*arg, *arg_ty, i));
        let arg_ptr = unsafe {
            ctx.builder.build_gep(
                args_ptr,
                &[int32.const_int(i as u64, false)],
                &format!("rpc.arg{i}"),
            )
        }
        .unwrap();
        ctx.builder.build_store(arg_ptr, arg_slot).unwrap();
    }

    // call
    infer_and_call_function(
        ctx,
        if is_async { "rpc_send_async" } else { "rpc_send" },
        None,
        &[service_id.into(), tag_ptr.into(), args_ptr.into()],
        Some("rpc.send"),
        None,
    );

    // reclaim stack space used by arguments
    call_stackrestore(ctx, stackptr);

    if is_async {
        // async RPCs do not return any values
        Ok(None)
    } else {
        let result = format_rpc_ret(generator, ctx, fun.0.ret);

        if !result.is_some_and(|res| res.get_type().is_pointer_type()) {
            // An RPC returning an NDArray would not touch here.
            call_stackrestore(ctx, stackptr);
        }

        Ok(result)
    }
}

pub fn attributes_writeback<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    generator: &mut dyn CodeGenerator,
    inner_resolver: &InnerResolver,
    host_attributes: &PyObject,
    return_obj: Option<(Type, ValueEnum<'ctx>)>,
) -> Result<(), String> {
    Python::with_gil(|py| -> PyResult<Result<(), String>> {
        let host_attributes: &PyList = host_attributes.downcast(py)?;
        let top_levels = ctx.top_level.definitions.read();
        let globals = inner_resolver.global_value_ids.read();
        let int32 = ctx.ctx.i32_type();
        let zero = int32.const_zero();
        let mut values = Vec::new();
        let mut scratch_buffer = Vec::new();

        if let Some((ty, obj)) = return_obj {
            values.push((ty, obj.to_basic_value_enum(ctx, generator, ty).unwrap()));
        }

        for val in (*globals).values() {
            let val = val.as_ref(py);
            let ty = inner_resolver.get_obj_type(
                py,
                val,
                &mut ctx.unifier,
                &top_levels,
                &ctx.primitives,
            )?;
            if let Err(ty) = ty {
                return Ok(Err(ty));
            }
            let ty = ty.unwrap();
            match &*ctx.unifier.get_ty(ty) {
                TypeEnum::TObj { fields, obj_id, .. }
                    if *obj_id != ctx.primitives.option.obj_id(&ctx.unifier).unwrap() =>
                {
                    // we only care about primitive attributes
                    // for non-primitive attributes, they should be in another global
                    let mut attributes = Vec::new();
                    let obj = inner_resolver.get_obj_value(py, val, ctx, generator, ty)?.unwrap();
                    for (name, (field_ty, is_mutable)) in fields {
                        if !is_mutable {
                            continue;
                        }
                        if gen_rpc_tag(ctx, *field_ty, &mut scratch_buffer).is_ok() {
                            attributes.push(name.to_string());
                            let (index, _) = ctx.get_attr_index(ty, *name);
                            values.push((
                                *field_ty,
                                ctx.build_gep_and_load(
                                    obj.into_pointer_value(),
                                    &[zero, int32.const_int(index as u64, false)],
                                    None,
                                ),
                            ));
                        }
                    }
                    if !attributes.is_empty() {
                        let pydict = PyDict::new(py);
                        pydict.set_item("obj", val)?;
                        pydict.set_item("fields", attributes)?;
                        host_attributes.append(pydict)?;
                    }
                }
                TypeEnum::TObj { obj_id, params, .. } if *obj_id == PrimDef::List.id() => {
                    let elem_ty = iter_type_vars(params).next().unwrap().ty;

                    if gen_rpc_tag(ctx, elem_ty, &mut scratch_buffer).is_ok() {
                        let pydict = PyDict::new(py);
                        pydict.set_item("obj", val)?;
                        host_attributes.append(pydict)?;
                        values.push((
                            ty,
                            inner_resolver.get_obj_value(py, val, ctx, generator, ty)?.unwrap(),
                        ));
                    }
                }
                TypeEnum::TModule { attributes, .. } => {
                    let mut fields = Vec::new();
                    let obj = inner_resolver.get_obj_value(py, val, ctx, generator, ty)?.unwrap();

                    for (name, (field_ty, is_method)) in attributes {
                        if *is_method {
                            continue;
                        }
                        if gen_rpc_tag(ctx, *field_ty, &mut scratch_buffer).is_ok() {
                            fields.push(name.to_string());
                            let (index, _) = ctx.get_attr_index(ty, *name);
                            values.push((
                                *field_ty,
                                ctx.build_gep_and_load(
                                    obj.into_pointer_value(),
                                    &[zero, int32.const_int(index as u64, false)],
                                    None,
                                ),
                            ));
                        }
                    }
                    if !fields.is_empty() {
                        let pydict = PyDict::new(py);
                        pydict.set_item("obj", val)?;
                        pydict.set_item("fields", fields)?;
                        host_attributes.append(pydict)?;
                    }
                }
                _ => {}
            }
        }
        let fun = FunSignature {
            args: values
                .iter()
                .enumerate()
                .map(|(i, (ty, _))| FuncArg {
                    name: i.to_string().into(),
                    ty: *ty,
                    default_value: None,
                    is_vararg: false,
                })
                .collect(),
            ret: ctx.primitives.none,
            vars: VarMap::default(),
        };
        let args: Vec<_> =
            values.into_iter().map(|(_, val)| (None, ValueEnum::Dynamic(val))).collect();
        if let Err(e) =
            rpc_codegen_callback_fn(ctx, None, (&fun, PrimDef::Int32.id()), args, generator, true)
        {
            return Ok(Err(e));
        }
        Ok(Ok(()))
    })
    .unwrap()?;
    Ok(())
}

pub fn rpc_codegen_callback(is_async: bool) -> Arc<GenCall> {
    Arc::new(GenCall::new(Box::new(move |ctx, obj, fun, args, generator| {
        rpc_codegen_callback_fn(ctx, obj, fun, args, generator, is_async)
    })))
}

/// Returns the `fprintf` format constant for the given [`llvm_int_t`][`IntType`] on a platform with
/// [`llvm_usize`] as its native word size.
///
/// Note that, similar to format constants in `<inttypes.h>`, these constants need to be prepended
/// with `%`.
#[must_use]
fn get_fprintf_format_constant<'ctx>(
    llvm_usize: IntType<'ctx>,
    llvm_int_t: IntType<'ctx>,
    is_unsigned: bool,
) -> String {
    debug_assert!(matches!(llvm_usize.get_bit_width(), 8 | 16 | 32 | 64));

    let conv_spec = if is_unsigned { 'u' } else { 'd' };

    // https://en.cppreference.com/w/c/language/arithmetic_types
    // Note that NAC3 does **not** support LP32 and LLP64 configurations
    match llvm_int_t.get_bit_width() {
        8 => format!("hh{conv_spec}"),
        16 => format!("h{conv_spec}"),
        32 => conv_spec.to_string(),
        64 => format!("{}{conv_spec}", if llvm_usize.get_bit_width() == 64 { "l" } else { "ll" }),
        _ => todo!(
            "Not yet implemented for i{} on {}-bit platform",
            llvm_int_t.get_bit_width(),
            llvm_usize.get_bit_width()
        ),
    }
}

/// Prints one or more `values` to `core_log` or `rtio_log`.
///
/// * `separator` - The separator between multiple values.
/// * `suffix` - String to terminate the printed string, if any.
/// * `as_repr` - Whether the `repr()` output of values instead of `str()`.
/// * `as_rtio` - Whether to print to `rtio_log` instead of `core_log`.
fn polymorphic_print<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    generator: &mut dyn CodeGenerator,
    values: &[(Type, ValueEnum<'ctx>)],
    separator: &str,
    suffix: Option<&str>,
    as_repr: bool,
    as_rtio: bool,
) -> Result<(), String> {
    let printf = |ctx: &mut CodeGenContext<'ctx, '_>,
                  generator: &mut dyn CodeGenerator,
                  fmt: String,
                  args: Vec<BasicValueEnum<'ctx>>| {
        debug_assert!(!fmt.is_empty());
        debug_assert_eq!(fmt.as_bytes().last().unwrap(), &0u8);

        let llvm_i32 = ctx.ctx.i32_type();
        let llvm_pi8 = ctx.ctx.i8_type().ptr_type(AddressSpace::default());

        let fmt = ctx.gen_string(generator, fmt);
        let fmt = unsafe { fmt.get_field_at_index_unchecked(0) }.into_pointer_value();

        create_fn_and_call(
            ctx,
            if as_rtio { "rtio_log" } else { "core_log" },
            if as_rtio { None } else { Some(llvm_i32.into()) },
            &[llvm_pi8.into()],
            &once(fmt.into()).chain(args).collect_vec(),
            true,
            None,
            None,
        );
    };

    let llvm_i32 = ctx.ctx.i32_type();
    let llvm_i64 = ctx.ctx.i64_type();
    let llvm_usize = ctx.get_size_type();

    let suffix = suffix.unwrap_or_default();

    let mut fmt = String::new();
    let mut args = Vec::new();

    let flush = |ctx: &mut CodeGenContext<'ctx, '_>,
                 generator: &mut dyn CodeGenerator,
                 fmt: &mut String,
                 args: &mut Vec<BasicValueEnum<'ctx>>| {
        if !fmt.is_empty() {
            fmt.push('\0');
            printf(ctx, generator, mem::take(fmt), mem::take(args));
        }
    };

    for (ty, value) in values {
        let ty = *ty;
        let value = value.clone().to_basic_value_enum(ctx, generator, ty).unwrap();

        if !fmt.is_empty() {
            fmt.push_str(separator);
        }

        match &*ctx.unifier.get_ty_immutable(ty) {
            TypeEnum::TTuple { ty: tys, is_vararg_ctx: false } => {
                let pvalue = {
                    let pvalue = generator.gen_var_alloc(ctx, value.get_type(), None).unwrap();
                    ctx.builder.build_store(pvalue, value).unwrap();
                    pvalue
                };

                fmt.push('(');
                flush(ctx, generator, &mut fmt, &mut args);

                let tuple_vals = tys
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| {
                        (*ty, {
                            let pfield =
                                ctx.builder.build_struct_gep(pvalue, i as u32, "").unwrap();

                            ValueEnum::from(ctx.builder.build_load(pfield, "").unwrap())
                        })
                    })
                    .collect_vec();

                polymorphic_print(ctx, generator, &tuple_vals, ", ", None, true, as_rtio)?;

                if tuple_vals.len() == 1 {
                    fmt.push_str(",)");
                } else {
                    fmt.push(')');
                }
            }

            TypeEnum::TFunc { .. } => todo!(),
            TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::None.id() => {
                fmt.push_str("None");
            }

            TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::Bool.id() => {
                fmt.push_str("%.*s");

                let true_str = ctx.gen_string(generator, "True");
                let true_data =
                    unsafe { true_str.get_field_at_index_unchecked(0) }.into_pointer_value();
                let true_len = unsafe { true_str.get_field_at_index_unchecked(1) }.into_int_value();
                let false_str = ctx.gen_string(generator, "False");
                let false_data =
                    unsafe { false_str.get_field_at_index_unchecked(0) }.into_pointer_value();
                let false_len =
                    unsafe { false_str.get_field_at_index_unchecked(1) }.into_int_value();

                let bool_val = generator.bool_to_i1(ctx, value.into_int_value());

                args.extend([
                    ctx.builder.build_select(bool_val, true_len, false_len, "").unwrap(),
                    ctx.builder.build_select(bool_val, true_data, false_data, "").unwrap(),
                ]);
            }

            TypeEnum::TObj { obj_id, .. }
                if *obj_id == PrimDef::Int32.id()
                    || *obj_id == PrimDef::Int64.id()
                    || *obj_id == PrimDef::UInt32.id()
                    || *obj_id == PrimDef::UInt64.id() =>
            {
                let is_unsigned =
                    *obj_id == PrimDef::UInt32.id() || *obj_id == PrimDef::UInt64.id();

                let llvm_int_t = value.get_type().into_int_type();
                debug_assert!(matches!(llvm_usize.get_bit_width(), 32 | 64));
                debug_assert!(matches!(llvm_int_t.get_bit_width(), 32 | 64));

                let fmt_spec = format!(
                    "%{}",
                    get_fprintf_format_constant(llvm_usize, llvm_int_t, is_unsigned)
                );

                fmt.push_str(fmt_spec.as_str());
                args.push(value);
            }

            TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::Float.id() => {
                fmt.push_str("%g");
                args.push(value);
            }

            TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::Str.id() => {
                if as_repr {
                    fmt.push_str("\"%.*s\"");
                } else {
                    fmt.push_str("%.*s");
                }

                let str = value.into_struct_value();
                let str_data = unsafe { str.get_field_at_index_unchecked(0) }.into_pointer_value();
                let str_len = unsafe { str.get_field_at_index_unchecked(1) }.into_int_value();

                args.extend(&[str_len.into(), str_data.into()]);
            }

            TypeEnum::TObj { obj_id, params, .. } if *obj_id == PrimDef::List.id() => {
                let elem_ty = *params.iter().next().unwrap().1;

                fmt.push('[');
                flush(ctx, generator, &mut fmt, &mut args);

                let val =
                    ListValue::from_pointer_value(value.into_pointer_value(), llvm_usize, None);
                let len = val.load_size(ctx, None);
                let last =
                    ctx.builder.build_int_sub(len, llvm_usize.const_int(1, false), "").unwrap();

                gen_for_callback_incrementing(
                    generator,
                    ctx,
                    None,
                    llvm_usize.const_zero(),
                    (len, false),
                    |generator, ctx, _, i| {
                        let elem = unsafe { val.data().get_unchecked(ctx, generator, &i, None) };

                        polymorphic_print(
                            ctx,
                            generator,
                            &[(elem_ty, elem.into())],
                            "",
                            None,
                            true,
                            as_rtio,
                        )?;

                        gen_if_callback(
                            generator,
                            ctx,
                            |_, ctx| {
                                Ok(ctx
                                    .builder
                                    .build_int_compare(IntPredicate::ULT, i, last, "")
                                    .unwrap())
                            },
                            |generator, ctx| {
                                printf(ctx, generator, ", \0".into(), Vec::default());

                                Ok(())
                            },
                            |_, _| Ok(()),
                        )?;

                        Ok(())
                    },
                    llvm_usize.const_int(1, false),
                )?;

                fmt.push(']');
                flush(ctx, generator, &mut fmt, &mut args);
            }

            TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
                fmt.push_str("array([");
                flush(ctx, generator, &mut fmt, &mut args);

                let (dtype, _) = unpack_ndarray_var_tys(&mut ctx.unifier, ty);
                let ndarray = NDArrayType::from_unifier_type(generator, ctx, ty)
                    .map_pointer_value(value.into_pointer_value(), None);

                let num_0 = llvm_usize.const_zero();

                // Print `ndarray` as a flat list delimited by interspersed with ", \0"
                ndarray.foreach(generator, ctx, |generator, ctx, _, hdl| {
                    let i = hdl.get_index(ctx);
                    let scalar = hdl.get_scalar(ctx);

                    // if (i != 0) puts(", ");
                    gen_if_callback(
                        generator,
                        ctx,
                        |_, ctx| {
                            let not_first = ctx
                                .builder
                                .build_int_compare(IntPredicate::NE, i, num_0, "")
                                .unwrap();
                            Ok(not_first)
                        },
                        |generator, ctx| {
                            printf(ctx, generator, ", \0".into(), Vec::default());
                            Ok(())
                        },
                        |_, _| Ok(()),
                    )?;

                    // Print element
                    polymorphic_print(
                        ctx,
                        generator,
                        &[(dtype, scalar.into())],
                        "",
                        None,
                        true,
                        as_rtio,
                    )?;
                    Ok(())
                })?;

                fmt.push_str(")]");
                flush(ctx, generator, &mut fmt, &mut args);
            }

            TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::Range.id() => {
                fmt.push_str("range(");
                flush(ctx, generator, &mut fmt, &mut args);

                let val = RangeType::new(ctx).map_pointer_value(value.into_pointer_value(), None);

                let (start, stop, step) = destructure_range(ctx, val);

                polymorphic_print(
                    ctx,
                    generator,
                    &[
                        (ctx.primitives.int32, start.into()),
                        (ctx.primitives.int32, stop.into()),
                        (ctx.primitives.int32, step.into()),
                    ],
                    ", ",
                    None,
                    false,
                    as_rtio,
                )?;

                fmt.push(')');
            }

            TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::Exception.id() => {
                let fmt_str = format!(
                    "%{}(%{}, %{1:}, %{1:})",
                    get_fprintf_format_constant(llvm_usize, llvm_i32, false),
                    get_fprintf_format_constant(llvm_usize, llvm_i64, false),
                );

                let exn = value.into_pointer_value();
                let name = ctx
                    .build_in_bounds_gep_and_load(
                        exn,
                        &[llvm_i32.const_zero(), llvm_i32.const_zero()],
                        None,
                    )
                    .into_int_value();
                let param0 = ctx
                    .build_in_bounds_gep_and_load(
                        exn,
                        &[llvm_i32.const_zero(), llvm_i32.const_int(6, false)],
                        None,
                    )
                    .into_int_value();
                let param1 = ctx
                    .build_in_bounds_gep_and_load(
                        exn,
                        &[llvm_i32.const_zero(), llvm_i32.const_int(7, false)],
                        None,
                    )
                    .into_int_value();
                let param2 = ctx
                    .build_in_bounds_gep_and_load(
                        exn,
                        &[llvm_i32.const_zero(), llvm_i32.const_int(8, false)],
                        None,
                    )
                    .into_int_value();

                fmt.push_str(fmt_str.as_str());
                args.extend_from_slice(&[name.into(), param0.into(), param1.into(), param2.into()]);
            }

            _ => unreachable!(
                "Unsupported object type for polymorphic_print: {}",
                ctx.unifier.stringify(ty)
            ),
        }
    }

    fmt.push_str(suffix);
    flush(ctx, generator, &mut fmt, &mut args);

    Ok(())
}

/// Invokes the `core_log` intrinsic function.
pub fn call_core_log_impl<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    generator: &mut dyn CodeGenerator,
    arg: (Type, BasicValueEnum<'ctx>),
) -> Result<(), String> {
    let (arg_ty, arg_val) = arg;

    polymorphic_print(ctx, generator, &[(arg_ty, arg_val.into())], " ", Some("\n"), false, false)?;

    Ok(())
}

/// Invokes the `rtio_log` intrinsic function.
pub fn call_rtio_log_impl<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    generator: &mut dyn CodeGenerator,
    channel: StructValue<'ctx>,
    arg: (Type, BasicValueEnum<'ctx>),
) -> Result<(), String> {
    let (arg_ty, arg_val) = arg;

    polymorphic_print(
        ctx,
        generator,
        &[(ctx.primitives.str, channel.into())],
        " ",
        Some("\x1E"),
        false,
        true,
    )?;
    polymorphic_print(ctx, generator, &[(arg_ty, arg_val.into())], " ", Some("\x1D"), false, true)?;

    Ok(())
}

/// Generates a call to `core_log`.
pub fn gen_core_log<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    obj: Option<&(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
    generator: &mut dyn CodeGenerator,
) -> Result<(), String> {
    assert!(obj.is_none());
    assert_eq!(args.len(), 1);

    let value_ty = fun.0.args[0].ty;
    let value_arg = args[0].1.clone().to_basic_value_enum(ctx, generator, value_ty)?;

    call_core_log_impl(ctx, generator, (value_ty, value_arg))
}

/// Generates a call to `rtio_log`.
pub fn gen_rtio_log<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    obj: Option<&(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
    generator: &mut dyn CodeGenerator,
) -> Result<(), String> {
    assert!(obj.is_none());
    assert_eq!(args.len(), 2);

    let channel_ty = fun.0.args[0].ty;
    assert!(ctx.unifier.unioned(channel_ty, ctx.primitives.str));
    let channel_arg =
        args[0].1.clone().to_basic_value_enum(ctx, generator, channel_ty)?.into_struct_value();
    let value_ty = fun.0.args[1].ty;
    let value_arg = args[1].1.clone().to_basic_value_enum(ctx, generator, value_ty)?;

    call_rtio_log_impl(ctx, generator, channel_arg, (value_ty, value_arg))
}
