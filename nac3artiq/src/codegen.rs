use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    mem,
    sync::Arc,
};

use anyhow::{anyhow, bail};
use itertools::Itertools as _;
use nac3core::{
    codegen::{
        CodeGenContext, CodeGenerator, FunctionDecl, VarValue,
        allocator::AllocationScope,
        basic_type_all, bool_to_i1,
        expr::{call_extern, destructure_range, gen_call},
        llvm_intrinsics::{call_int_smax, call_memcpy, call_stackrestore, call_stacksave},
        stmt::{
            gen_block, gen_for_callback_incrementing, gen_if_callback, gen_while_callback, gen_with,
        },
        type_aligned_allocate,
        types::{
            ArrayLikeIndexer, ClassType, ExceptionType, ListType, NDArrayType, ProxyTypeBase,
            RangeType, TupleType, TupleValue, field, is_refcounted_type,
        },
    },
    inkwell::{
        IntPredicate,
        module::Linkage,
        types::{BasicTypeEnum, IntType, StructType},
        values::{BasicValueEnum, IntValue, PointerValue, StructValue},
    },
    nac3parser::ast::{Expr, ExprKind, Located, Stmt, StmtKind, StrRef},
    symbol_resolver::ValueEnum,
    toplevel::{
        DefinitionId, GenCall, TopLevelDef,
        helper::{PrimDef, extract_ndims},
        numpy::unpack_ndarray_var_tys,
    },
    typecheck::{
        type_inferencer::PrimitiveStore,
        typedef::{FunSignature, FuncArg, Type, TypeEnum, Unifier, VarMap, iter_type_vars},
    },
};
use pyo3::{
    Python,
    prelude::*,
    types::{PyDict, PyList},
};

use crate::{SpecialPythonId, symbol_resolver::InnerResolver, timeline::TimeFns};

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

/// ARTIQ-specific code generator that extends the default with timeline manipulation,
/// `with parallel`/`with sequential` block handling, and RPC support.
pub struct ArtiqCodeGenerator<'a> {
    name: String,

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
        timeline: &'a (dyn TimeFns + Sync),
        special_ids: SpecialPythonId,
    ) -> Self {
        ArtiqCodeGenerator {
            name,
            name_counter: 0,
            start: None,
            end: None,
            timeline,
            parallel_mode: ParallelMode::None,
            special_ids,
        }
    }

    /// If the generator is currently in a direct-`parallel` block context, emits IR that resets the
    /// position of the timeline to the initial timeline position before entering the `parallel`
    /// block.
    ///
    /// Direct-`parallel` block context refers to when the generator is generating statements whose
    /// closest parent `with` statement is a `with parallel` block.
    fn timeline_reset_start(&mut self, ctx: &mut CodeGenContext<'_, '_>) -> anyhow::Result<()> {
        if let Some(start) = self.start.clone() {
            let start_val = self.gen_expr(ctx, &start)?.to_basic_value_enum(ctx)?;
            self.timeline.emit_at_mu(ctx, start_val)?;
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
    ) -> anyhow::Result<()> {
        if let Some(end) = end {
            let old_end = self.gen_expr(ctx, &end)?.to_basic_value_enum(ctx)?;
            let now = self.timeline.emit_now_mu(ctx)?;
            let max =
                call_int_smax(ctx, old_end.into_int_value(), now.into_int_value(), Some("smax"))?;
            let end_store = self
                .gen_store_target(
                    ctx,
                    &end,
                    store_name.map(|name| format!("{name}.addr")).as_deref(),
                )?
                .unwrap();
            ctx.builder.build_store(end_store, max)?;
        }

        Ok(())
    }
}

impl CodeGenerator for ArtiqCodeGenerator<'_> {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn gen_block<'ctx, 'a, 'c, I: Iterator<Item = &'c Stmt<Option<Type>>>>(
        &mut self,
        ctx: &mut CodeGenContext<'ctx, 'a>,
        stmts: I,
    ) -> anyhow::Result<()>
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
    ) -> anyhow::Result<Option<BasicValueEnum<'ctx>>> {
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
    ) -> anyhow::Result<()> {
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
                if let Some(static_value) = if let Some(VarValue { static_value, .. }) =
                    ctx.var_assignment.get(id)
                {
                    static_value.clone()
                } else if let Some(ValueEnum::Static(val)) = resolver.get_symbol_value(*id, ctx)? {
                    Some(val)
                } else {
                    None
                } {
                    let python_id = static_value.get_unique_identifier();
                    if python_id == self.special_ids.parallel
                        || python_id == self.special_ids.legacy_parallel
                    {
                        let old_start = self.start.take();
                        let old_end = self.end.take();
                        let old_parallel_mode = self.parallel_mode;

                        let now = if let Some(old_start) = &old_start {
                            self.gen_expr(ctx, old_start)?.to_basic_value_enum(ctx)?
                        } else {
                            self.timeline.emit_now_mu(ctx)?
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
                                ctx.builder.build_store(start, now)?;
                                anyhow::Ok(Some(start_expr))
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
                        ctx.builder.build_store(end, now)?;
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
                        let end_val = self.gen_expr(ctx, &end_expr)?.to_basic_value_enum(ctx)?;

                        // inside a sequential block
                        if old_start.is_none() {
                            self.timeline.emit_at_mu(ctx, end_val)?;
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

/// The type of wire descriptor for a value to be marshaled or demarshaled for RPC.
enum WireDescriptorKind {
    List { elem_ty: Type },
    Tuple(Vec<Type>),
    NDArray { dtype: Type, ndims: u64 },
    Default,
}

impl WireDescriptorKind {
    /// Categorizes the given [`Type`] into a [`WireDescriptorKind`] for RPC marshaling and
    /// demarshaling.
    fn classify(unifier: &mut Unifier, ty: Type) -> Self {
        match &*unifier.get_ty(ty) {
            TypeEnum::TObj { obj_id, params, .. } if *obj_id == PrimDef::List.id() => {
                Self::List { elem_ty: iter_type_vars(params).next().unwrap().ty }
            }
            TypeEnum::TTuple { ty: fields, is_vararg_ctx: false } => Self::Tuple(fields.clone()),
            TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
                let (dtype, ndims_ty) = unpack_ndarray_var_tys(unifier, ty);
                Self::NDArray { dtype, ndims: extract_ndims(unifier, ndims_ty) }
            }
            _ => Self::Default,
        }
    }
}

/// Returns whether a value of `ty` can be `memcpy`'d directly between NAC3's in-memory
/// representation and `libproto_artiq`'s wire representation without any layout conversion.
///
/// Only the primitive types (`int32`, `int64`, `float`, `bool`, `none`, `str`) qualify. All
/// composite types — including tuples of primitives — are not RPC-bit-compatible because NAC3
/// prefixes them with an `ObjectHeader` that the wire format does not carry.
fn is_rpc_bit_compatible(unifier: &mut Unifier, primitives: &PrimitiveStore, ty: Type) -> bool {
    unifier.unioned(ty, primitives.int32)
        || unifier.unioned(ty, primitives.int64)
        || unifier.unioned(ty, primitives.float)
        || unifier.unioned(ty, primitives.bool)
        || unifier.unioned(ty, primitives.none)
        || unifier.unioned(ty, primitives.str)
}

/// Returns the LLVM struct type matching `libproto_artiq`'s wire layout for a tuple with the
/// given field types. See [`wire_type_of`] for the per-field expansion.
fn wire_struct_type_of<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    field_tys: &[Type],
) -> StructType<'ctx> {
    let wire_field_tys: Vec<BasicTypeEnum<'ctx>> =
        field_tys.iter().map(|f| wire_type_of(ctx, *f)).collect();
    ctx.ctx.struct_type(&wire_field_tys, false)
}

/// Returns the LLVM type matching `libproto_artiq`'s wire layout for a value of `ty`.
///
/// - Bit-compatible scalars utilize the same representation.
/// - `list`s are both represented as a pointer to a struct containing
///   `{ elements: ptr, length: size_t }` in both layouts, but the NAC3 representation prepends an
///   `ObjectHeader`.
/// - `ndarray`s are represented as an inline struct `{ data[], shape: size_t[ndims] }` in the wire
///   format, while NAC3 represents them as a refcounted [`RawNDArrayType`] object.
/// - Tuples are represented as `{ field0, field1, ... }` (each field recursively converted to its
///   wire shape); the NAC3 layout differs in that it prepends an `ObjectHeader` to the inner
///   fields struct.
fn wire_type_of<'ctx>(ctx: &mut CodeGenContext<'ctx, '_>, ty: Type) -> BasicTypeEnum<'ctx> {
    match WireDescriptorKind::classify(&mut ctx.unifier, ty) {
        WireDescriptorKind::NDArray { ndims, .. } => ctx
            .ctx
            .struct_type(&[ctx.ptr.into(), ctx.size_t.array_type(ndims as u32).into()], false)
            .into(),
        WireDescriptorKind::Tuple(fields) => wire_struct_type_of(ctx, &fields).into(),
        _ => ctx.get_llvm_type(ty),
    }
}

/// Writes an `ndarray`'s wire-shape descriptor `[*data, shape[ndims]]` (`(1 + ndims) *
/// sizeof(usize)` bytes) at the given destination buffer.
///
/// `dest` must be a writable buffer of at least that size, and must be suitably aligned for
/// `*data` (i.e. pointer alignment). The function makes the NAC3 ndarray contiguous and writes
/// the resulting data pointer plus the shape array.
fn write_ndarray_wire_descriptor<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    dtype: Type,
    ndims: u64,
    value: BasicValueEnum<'ctx>,
    dest: PointerValue<'ctx>,
) -> anyhow::Result<()> {
    let sizeof_ptr = ctx.size_t.const_int(ctx.sizeof(ctx.ptr), false);

    let dtype = ctx.get_llvm_type(dtype);
    let ndarray =
        NDArrayType::create(ctx, dtype, ndims).map_value(value.into_pointer_value(), None);
    let carray = ndarray.make_contiguous_ndarray(ctx)?;

    let carray_nbytes = ndarray.nbytes(ctx)?;
    let carray_data =
        carray.inner_value(ctx)?.data(ctx)?.inner_value(ctx, Some(carray_nbytes))?.value.0;
    ctx.builder.build_store(dest, carray_data)?;

    let dest_shape = unsafe { ctx.builder.build_gep(ctx.i8, dest, &[sizeof_ptr], "")? };
    let carray_shape = ndarray.shape(ctx)?.inner_value(ctx, None)?.value.0;
    let sizeof_buf_shape = ctx.size_t.const_int(ctx.sizeof(ctx.size_t) * ndims, false);
    call_memcpy(ctx, dest_shape, carray_shape, sizeof_buf_shape)?;

    Ok(())
}

/// Recursively converts a NAC3 value into the firmware's on-the-wire representation.
///
/// Returns a pointer to a freshly stack-allocated buffer holding the wire-format value.
fn marshal_to_wire<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    ty: Type,
    value: BasicValueEnum<'ctx>,
    name: &str,
) -> anyhow::Result<PointerValue<'ctx>> {
    Ok(match WireDescriptorKind::classify(&mut ctx.unifier, ty) {
        WireDescriptorKind::List { elem_ty } => {
            // NAC3: list = { _: ObjectHeader, data: ptr, length: size_t }
            // libartiq_proto: list = {elements: ptr, length: size_t}
            let list = ListType::create(ctx, elem_ty)
                .map_value(value.into_pointer_value(), None)
                .inner_value(ctx)?;
            let length = list.load(ctx, field!(len))?;

            let elements_array =
                if is_rpc_bit_compatible(&mut ctx.unifier, &ctx.primitives, elem_ty) {
                    // element type of list is bit-compatible - can be directly memcpy'ed
                    list.data(ctx)?.inner_value(ctx, Some(length))?
                } else if let WireDescriptorKind::NDArray { dtype, ndims } =
                    WireDescriptorKind::classify(&mut ctx.unifier, elem_ty)
                {
                    // element type of list is ndarray - needs to be separately marshaled as
                    // [...data, shape[ndims]]
                    let descriptor_size = ctx.sizeof(ctx.ptr) + ctx.sizeof(ctx.size_t) * ndims;
                    let descriptor_size = ctx.size_t.const_int(descriptor_size, false);
                    let total_bytes = ctx.builder.build_int_mul(length, descriptor_size, "")?;
                    let wire_array = ctx.build_dyn_array_allocate(
                        AllocationScope::StackCurrentLoc,
                        ctx.i8,
                        total_bytes,
                        Some("rpc.arr.wire"),
                    )?;
                    gen_for_callback_incrementing(
                        &mut (),
                        ctx,
                        None,
                        ctx.size_t.const_zero(),
                        (length, false),
                        |(), ctx, _hooks, i| {
                            let nac3_elem = list
                                .data(ctx)?
                                .inner_value(ctx, Some(length))?
                                .get_unchecked(ctx, &i, None)?;
                            let offset = ctx.builder.build_int_mul(i, descriptor_size, "")?;
                            let dest = wire_array.ptr_offset_unchecked(ctx, &offset, None)?;
                            write_ndarray_wire_descriptor(ctx, dtype, ndims, nac3_elem, dest)
                        },
                        ctx.size_t.const_int(1, false),
                        |(), _| Ok(()),
                    )?;
                    wire_array
                } else {
                    // element type of list is a composite type - build a runtime array of wire pointers
                    // and marshal each element recursively
                    let wire_array = ctx.build_dyn_array_allocate(
                        AllocationScope::StackCurrentLoc,
                        ctx.ptr,
                        length,
                        Some("rpc.list.wire"),
                    )?;
                    gen_for_callback_incrementing(
                        &mut (),
                        ctx,
                        None,
                        ctx.size_t.const_zero(),
                        (length, false),
                        |(), ctx, _hooks, i| {
                            let nac3_elem = list
                                .data(ctx)?
                                .inner_value(ctx, Some(length))?
                                .get_unchecked(ctx, &i, None)?;
                            let wire_elem = marshal_to_wire(ctx, elem_ty, nac3_elem, "")?;
                            let wire_slot = wire_array
                                .ptr_offset_unchecked(ctx, &i, None)?;
                            ctx.builder.build_store(wire_slot, wire_elem)?;
                            Ok(())
                        },
                        ctx.size_t.const_int(1, false),
                        |(), _| Ok(()),
                    )?;
                    wire_array
                };

            let wire_ty = ctx.ctx.struct_type(&[ctx.ptr.into(), ctx.size_t.into()], false);
            let buf = ctx.build_allocate(AllocationScope::StackCurrentLoc, wire_ty, Some(name))?;
            let elements_field = ctx.builder.build_struct_gep(wire_ty, buf, 0, "")?;
            ctx.builder.build_store(elements_field, elements_array.value.0)?;
            let length_field = ctx.builder.build_struct_gep(wire_ty, buf, 1, "")?;
            ctx.builder.build_store(length_field, length)?;
            buf
        }

        WireDescriptorKind::Tuple(field_tys) => {
            // NAC3: tuple = { ObjectHeader, { field0, field1, ... } }
            // libartiq_proto: tuple = { field0, field1, ... }

            let nac3_tuple =
                TupleType::from_unifier_type(ctx, ty).map_value(value.into_struct_value(), None);
            let wire_ty = wire_struct_type_of(ctx, field_tys.as_slice());
            let buf = ctx.build_allocate(AllocationScope::StackCurrentLoc, wire_ty, Some(name))?;
            for (i, field_ty) in field_tys.iter().enumerate() {
                let field_val = nac3_tuple.extract(ctx, i as u32)?;
                let field_slot = ctx.builder.build_struct_gep(wire_ty, buf, i as u32, "")?;
                if is_rpc_bit_compatible(&mut ctx.unifier, &ctx.primitives, *field_ty) {
                    ctx.builder.build_store(field_slot, field_val)?;
                } else {
                    let wire_field_ptr = marshal_to_wire(ctx, *field_ty, field_val, "")?;
                    if let WireDescriptorKind::List { .. } =
                        WireDescriptorKind::classify(&mut ctx.unifier, *field_ty)
                    {
                        // Refcounted fields (e.g. `list`) live behind a pointer in the wire
                        // format, matching the pointer-sized NAC3 field slot.
                        ctx.builder.build_store(field_slot, wire_field_ptr)?;
                    } else {
                        // Inline fields (ndarray descriptor, nested tuple) are copied
                        // directly into the field slot.
                        let inner_wire_ty = wire_type_of(ctx, *field_ty);
                        let inner_size = ctx.size_t.const_int(ctx.sizeof(inner_wire_ty), false);
                        call_memcpy(ctx, field_slot, wire_field_ptr, inner_size)?;
                    }
                }
            }
            buf
        }

        WireDescriptorKind::NDArray { dtype, ndims } => {
            // Marshal the ndarray as an inline `[*data, shape[ndims]]` descriptor.
            let descriptor_size = ctx.sizeof(ctx.ptr) + ctx.sizeof(ctx.size_t) * ndims;
            let buf = ctx.build_array_allocate(
                AllocationScope::StackCurrentLoc,
                ctx.i8,
                descriptor_size,
                None,
            )?;
            write_ndarray_wire_descriptor(ctx, dtype, ndims, value, buf.value.0)?;
            buf.value.0
        }

        WireDescriptorKind::Default => {
            // marshal bit-compatible scalars as-is
            let buf =
                ctx.build_allocate(AllocationScope::StackCurrentLoc, value.get_type(), Some(name))?;
            ctx.builder.build_store(buf, value)?;
            buf
        }
    })
}

/// Reads an `ndarray`'s wire-shape descriptor `[*data, shape[ndims]]` from the given location,
/// and constructs a fresh NAC3 ndarray with that shape and data.
///
/// The `descriptor` should contain `sizeof(usize) * (1 + ndims)` bytes of data.
fn demarshal_ndarray_descriptor<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    dtype: Type,
    ndims: u64,
    descriptor: PointerValue<'ctx>,
) -> anyhow::Result<PointerValue<'ctx>> {
    let dtype = ctx.get_llvm_type(dtype);

    let wire_data =
        ctx.builder.build_load(ctx.ptr, descriptor, "rpc.nd.data")?.into_pointer_value();
    let sizeof_pdata = ctx.size_t.const_int(ctx.sizeof(ctx.ptr), false);
    let descriptor_shape =
        unsafe { ctx.builder.build_gep(ctx.i8, descriptor, &[sizeof_pdata], "")? };

    let ndarray = NDArrayType::create(ctx, dtype, ndims).construct(ctx, None)?;
    ndarray.shape(ctx)?.inner_value(ctx, None)?.memcpy_from(ctx, descriptor_shape)?;
    ndarray.create_data(ctx)?;

    let nelems = ndarray.size(ctx)?;
    let itemsize = ctx.sizeof(ndarray.ty.object.dtype);
    let nbytes = ctx.builder.build_int_mul(nelems, ctx.size_t.const_int(itemsize, false), "")?;

    let ndarray_offset = ndarray.inner_value(ctx)?.load(ctx, field!(offset))?;
    let ndarray_data = ndarray
        .inner_value(ctx)?
        .base_data(ctx)?
        .inner_value(ctx, Some(nelems))?
        .cast(ctx, ctx.i8, None, None)?
        .ptr_offset_unchecked(ctx, &ndarray_offset, None)?;

    call_memcpy(ctx, ndarray_data, wire_data, nbytes)?;
    ctx.builder.build_free(wire_data)?;

    Ok(ndarray.value)
}

/// Recursively constructs a NAC3 value from the firmware's on-the-wire representation in
/// `wire_buf`.
///
/// `wire_buf` points to the start of the wire data for `ty`:
/// - **List**: `wire_buf` is the firmware-allocated `*{elements, length, [pad], data…}`. After
///   demarshaling, the wire block is `build_free`'d and a fresh refcount-aware NAC3 list is
///   returned. Non-bit-compatible elements are walked recursively.
/// - **Tuple**: `wire_buf` points to the wire-shape inline tuple (which has the same field
///   offsets as NAC3's tuple — both are non-refcounted, so layouts coincide *except* at
///   list-typed fields). Such fields are demarshaled in place; the loaded tuple value is
///   returned.
/// - **Scalar / `str` / `none`**: load directly.
fn demarshal_from_wire<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    ty: Type,
    wire_buf: PointerValue<'ctx>,
) -> anyhow::Result<BasicValueEnum<'ctx>> {
    Ok(match WireDescriptorKind::classify(&mut ctx.unifier, ty) {
        WireDescriptorKind::List { elem_ty } => {
            // NAC3: list = { _: ObjectHeader, data: ptr, length: size_t }
            // libartiq_proto: list = {elements: ptr, length: size_t}
            let wire_ty = ctx.ctx.struct_type(&[ctx.ptr.into(), ctx.size_t.into()], false);
            let elements_field = ctx.builder.build_struct_gep(wire_ty, wire_buf, 0, "")?;
            let elements_ptr = ctx
                .builder
                .build_load(ctx.ptr, elements_field, "rpc.elements")?
                .into_pointer_value();
            let length_field = ctx.builder.build_struct_gep(wire_ty, wire_buf, 1, "")?;
            let length =
                ctx.builder.build_load(ctx.size_t, length_field, "rpc.length")?.into_int_value();

            let list = ListType::create(ctx, elem_ty).construct(ctx, length, Some("rpc.list"))?;
            let nac3_data = list.inner_value(ctx)?.data(ctx)?.inner_value(ctx, Some(length))?;

            if is_rpc_bit_compatible(&mut ctx.unifier, &ctx.primitives, elem_ty) {
                // element type of list is bit-compatible - can be directly memcpy'ed
                let elem_llvm_ty = ctx.get_llvm_type(elem_ty);
                let elem_size = ctx.size_t.const_int(ctx.sizeof(elem_llvm_ty), false);
                let total_bytes = ctx.builder.build_int_mul(length, elem_size, "")?;
                call_memcpy(ctx, nac3_data.value.0, elements_ptr, total_bytes)?;
            } else if let WireDescriptorKind::NDArray { ndims, .. } =
                WireDescriptorKind::classify(&mut ctx.unifier, elem_ty)
            {
                // element type of list is ndarray - needs to be separately demarshaled into
                // NAC3 ndarrays
                let descriptor_size = ctx.sizeof(ctx.ptr) + ctx.sizeof(ctx.size_t) * ndims;
                let descriptor_size = ctx.size_t.const_int(descriptor_size, false);
                gen_for_callback_incrementing(
                    &mut (),
                    ctx,
                    None,
                    ctx.size_t.const_zero(),
                    (length, false),
                    |(), ctx, _hooks, i| {
                        let offset = ctx.builder.build_int_mul(i, descriptor_size, "")?;
                        let descriptor =
                            unsafe { ctx.builder.build_gep(ctx.i8, elements_ptr, &[offset], "")? };
                        let nac3_ndarray = demarshal_from_wire(ctx, elem_ty, descriptor)?;
                        let nac3_elem_slot = nac3_data.ptr_offset_unchecked(ctx, &i, None)?;
                        ctx.builder.build_store(nac3_elem_slot, nac3_ndarray)?;
                        Ok(())
                    },
                    ctx.size_t.const_int(1, false),
                    |(), _| Ok(()),
                )?;
            } else {
                // element type of list is a composite type - demarshal each element recursively.
                //
                // The wire stride depends on how the firmware stored each element:
                // - Refcounted elements (`list`) are stored as pointers — stride is `sizeof(ptr)`,
                //   and we load the pointer before recursing.
                // - Inline composites (tuple) are stored as packed wire-shape values — stride is
                //   `sizeof(wire_type_of(elem_ty))`, and the slot address is used directly.
                let is_indirect = is_refcounted_type(&mut ctx.unifier, elem_ty);
                let stride_ty: BasicTypeEnum<'ctx> =
                    if is_indirect { ctx.ptr.into() } else { wire_type_of(ctx, elem_ty) };
                gen_for_callback_incrementing(
                    &mut (),
                    ctx,
                    None,
                    ctx.size_t.const_zero(),
                    (length, false),
                    |(), ctx, _hooks, i| {
                        let wire_elem_slot =
                            unsafe { ctx.builder.build_gep(stride_ty, elements_ptr, &[i], "")? };
                        let nac3_elem_slot = nac3_data.ptr_offset_unchecked(ctx, &i, None)?;
                        let inner_wire = if is_indirect {
                            ctx.builder
                                .build_load(ctx.ptr, wire_elem_slot, "")?
                                .into_pointer_value()
                        } else {
                            wire_elem_slot
                        };
                        let nac3_val = demarshal_from_wire(ctx, elem_ty, inner_wire)?;
                        ctx.builder.build_store(nac3_elem_slot, nac3_val)?;
                        Ok(())
                    },
                    ctx.size_t.const_int(1, false),
                    |(), _| Ok(()),
                )?;
            }

            // free the wire block after we demarshaling - inner wire blocks are freed by their own
            // recursive demarshal calls
            ctx.builder.build_free(wire_buf)?;

            list.value.into()
        }

        WireDescriptorKind::Tuple(field_tys) => {
            // NAC3: tuple = { ObjectHeader, { field0, field1, ... } }
            // libartiq_proto: tuple = { field0, field1, ... }

            let wire_ty = wire_struct_type_of(ctx, field_tys.as_slice());
            let tuple_fields = field_tys
                .iter()
                .enumerate()
                .map(|(i, field_ty)| -> anyhow::Result<BasicValueEnum<'ctx>> {
                    let wire_field_slot =
                        ctx.builder.build_struct_gep(wire_ty, wire_buf, i as u32, "")?;
                    Ok(if is_rpc_bit_compatible(&mut ctx.unifier, &ctx.primitives, *field_ty) {
                        let llvm_field_ty = ctx.get_llvm_type(*field_ty);
                        ctx.builder.build_load(llvm_field_ty, wire_field_slot, "")?
                    } else {
                        // `list` fields in the wire format are stored as pointers; all other
                        // non-bit-compatible types (ndarray descriptor, nested tuple) are inline.
                        let inner_wire = if matches!(
                            WireDescriptorKind::classify(&mut ctx.unifier, *field_ty),
                            WireDescriptorKind::List { .. }
                        ) {
                            ctx.builder
                                .build_load(ctx.ptr, wire_field_slot, "")?
                                .into_pointer_value()
                        } else {
                            wire_field_slot
                        };
                        demarshal_from_wire(ctx, *field_ty, inner_wire)?
                    })
                })
                .try_collect::<_, Vec<_>, anyhow::Error>()?;
            TupleValue::new(ctx, &tuple_fields, Some("rpc.tup"))?.value.into()
        }

        WireDescriptorKind::NDArray { dtype, ndims } => {
            // `wire_buf` points directly to the inline `[*data, shape[ndims]]` descriptor.
            demarshal_ndarray_descriptor(ctx, dtype, ndims, wire_buf)?.into()
        }

        WireDescriptorKind::Default => {
            // demarshal bit-compatible scalars as-is
            let llvm_ty = ctx.get_llvm_type(ty);
            ctx.builder.build_load(llvm_ty, wire_buf, "rpc.val")?
        }
    })
}

fn gen_rpc_tag(
    ctx: &mut CodeGenContext<'_, '_>,
    ty: Type,
    buffer: &mut Vec<u8>,
) -> anyhow::Result<()> {
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
            TypeEnum::TTuple { ty, is_vararg_ctx: false } => {
                buffer.push(b't');
                buffer.push(ty.len() as u8);
                for ty in ty {
                    gen_rpc_tag(ctx, *ty, buffer)?;
                }
            }
            TypeEnum::TObj { obj_id, params, .. } if *obj_id == PrimDef::List.id() => {
                let ty = iter_type_vars(params).next().unwrap().ty;

                buffer.push(b'l');
                gen_rpc_tag(ctx, ty, buffer)?;
            }
            TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
                let (ndarray_dtype, ndarray_ndims) = unpack_ndarray_var_tys(&mut ctx.unifier, ty);
                let ndarray_ndims = if let TypeEnum::TLiteral { values, .. } =
                    &*ctx.unifier.get_ty_immutable(ndarray_ndims)
                {
                    if values.len() != 1 {
                        bail!(
                            "NDArray types with multiple literal bounds for ndims is not supported: {}",
                            ctx.unifier.stringify(ty)
                        );
                    }

                    let value = values[0].clone();
                    u64::try_from(value.clone())
                        .map_err(|()| anyhow!("Expected u64 for ndarray.ndims, got {value}"))?
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
            _ => bail!("Unsupported type: {:?}", ctx.unifier.stringify(ty)),
        }
    }
    Ok(())
}

/// Formats an RPC argument to conform to the expected format required by `send_value`.
///
/// See `artiq/firmware/libproto_artiq/rpc_proto.rs` for the expected format.
fn format_rpc_arg<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (arg, arg_ty, arg_idx): (BasicValueEnum<'ctx>, Type, usize),
) -> anyhow::Result<PointerValue<'ctx>> {
    let llvm_i8 = ctx.i8;
    let llvm_pi8 = ctx.ptr;

    let arg_slot = match WireDescriptorKind::classify(&mut ctx.unifier, arg_ty) {
        WireDescriptorKind::NDArray { dtype, ndims } => {
            // NAC3: NDArray = { _: ObjectHeader, ndims: usize, shape: usize*, data: T* }
            // libproto_artiq: NDArray = { data: [T; N], dim_sz: [usize; ndims] }

            // Top-level ndarray args use AllocationScope::Default so they survive across potential
            // stack-restore points in the RPC send loop
            let sizeof_buf = ctx.sizeof(ctx.ptr) + ctx.sizeof(ctx.size_t) * ndims;
            let buf = ctx.build_array_allocate(
                AllocationScope::Default,
                llvm_i8,
                sizeof_buf,
                Some("rpc.arg"),
            )?;
            write_ndarray_wire_descriptor(ctx, dtype, ndims, arg, buf.value.0)?;
            buf.value.0
        }

        WireDescriptorKind::List { .. } => {
            // NAC3: list = { _: ObjectHeader, data: ptr, length: size_t }
            // libproto_artiq: list = { ptr, size_t } - same order without ObjectHeader
            // arg slot: { ptr, size_t }* - firmware consumes as a pointer and derefs
            let list_ptr = marshal_to_wire(ctx, arg_ty, arg, &format!("rpc.arg{arg_idx}"))?;
            let slot = ctx.build_allocate(
                AllocationScope::StackCurrentLoc,
                ctx.ptr,
                Some("rpc.arg.list.slot"),
            )?;
            ctx.builder.build_store(slot, list_ptr)?;
            slot
        }

        // All other types are directly passed to `marshal_to_wire` for recursive handling
        _ => marshal_to_wire(ctx, arg_ty, arg, &format!("rpc.arg{arg_idx}"))?,
    };

    debug_assert_eq!(arg_slot.get_type(), llvm_pi8);

    Ok(arg_slot)
}

/// Drives the RPC receive protocol loop, calling `rpc_recv` repeatedly until it signals
/// completion (returns 0), and allocating storage on demand via `alloc_fn` for each non-zero
/// response.
fn gen_rpc_recv_loop<'ctx, 'a, AllocFn>(
    ctx: &mut CodeGenContext<'ctx, 'a>,
    rpc_recv: &FunctionDecl<'ctx>,
    initial_ptr: PointerValue<'ctx>,
    alloc_fn: AllocFn,
) -> anyhow::Result<()>
where
    AllocFn:
        FnOnce(&mut CodeGenContext<'ctx, 'a>, IntValue<'ctx>) -> anyhow::Result<PointerValue<'ctx>>,
{
    let llvm_i32 = ctx.i32;
    let llvm_pi8 = ctx.ptr;

    let loop_stackptr = call_stacksave(ctx, None)?;
    let ptr_slot =
        ctx.build_allocate(AllocationScope::StackCurrentLoc, llvm_pi8, Some("rpc.ptr.slot"))?;
    let size_slot =
        ctx.build_allocate(AllocationScope::StackCurrentLoc, llvm_i32, Some("rpc.size.slot"))?;
    ctx.builder.build_store(ptr_slot, initial_ptr)?;

    gen_while_callback(
        &mut (),
        ctx,
        Some("rpc"),
        |(), ctx| {
            let ptr = ctx.builder.build_load(llvm_pi8, ptr_slot, "rpc.ptr")?.into_pointer_value();
            let alloc_size = ctx
                .build_call_or_invoke(rpc_recv, &[ptr.into()], "rpc.size.next")?
                .map(BasicValueEnum::into_int_value)
                .unwrap();
            ctx.builder.build_store(size_slot, alloc_size)?;
            Ok(ctx.builder.build_int_compare(
                IntPredicate::NE,
                alloc_size,
                llvm_i32.const_zero(),
                "rpc.continue",
            )?)
        },
        |(), ctx| {
            let alloc_size =
                ctx.builder.build_load(llvm_i32, size_slot, "rpc.size")?.into_int_value();
            let alloc_ptr = alloc_fn(ctx, alloc_size)?;
            ctx.builder.build_store(ptr_slot, alloc_ptr)?;
            Ok(())
        },
        |(), _ctx| Ok(()),
    )?;

    call_stackrestore(ctx, loop_stackptr)
}

/// Formats an RPC return value to conform to the expected format required by NAC3.
fn format_rpc_ret<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    ret_ty: Type,
) -> anyhow::Result<Option<BasicValueEnum<'ctx>>> {
    // -- receive value:
    // T result = {
    //   void *ret_ptr = alloca(sizeof(T));
    //   void *ptr = ret_ptr;
    //   loop: int size = rpc_recv(ptr);
    //   // Non-zero: Provide `size` bytes of extra storage for variable-length data.
    //   if(size) { ptr = alloca(size); goto loop; }
    //   else *(T*)ret_ptr
    // }

    let llvm_i32 = ctx.i32;
    let llvm_i8_8 = ctx.ctx.struct_type(&[ctx.i8.array_type(8).into()], false);
    let llvm_pi8 = ctx.ptr;

    let rpc_recv =
        ctx.declare_external("rpc_recv", Some(llvm_i32.into()), &[llvm_pi8.into()], false, &[]);

    if ctx.unifier.unioned(ret_ty, ctx.primitives.none) {
        let _ = ctx.build_call_or_invoke(&rpc_recv, &[llvm_pi8.const_null().into()], "rpc_recv")?;
        return Ok(None);
    }

    let result = match &*ctx.unifier.get_ty(ret_ty) {
        TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
            let num_0 = ctx.size_t.const_zero();

            // Round `val` up to its modulo `power_of_two`
            let round_up = |ctx: &mut CodeGenContext<'ctx, '_>,
                            val: IntValue<'ctx>,
                            power_of_two: IntValue<'ctx>| {
                debug_assert_eq!(
                    val.get_type().get_bit_width(),
                    power_of_two.get_type().get_bit_width()
                );

                let llvm_val_t = val.get_type();

                let max_rem =
                    ctx.builder.build_int_sub(power_of_two, llvm_val_t.const_int(1, false), "")?;
                anyhow::Ok(ctx.builder.build_and(
                    ctx.builder.build_int_add(val, max_rem, "")?,
                    ctx.builder.build_not(max_rem, "")?,
                    "",
                )?)
            };

            // Allocate the resulting ndarray
            // A condition after format_rpc_ret ensures this will not be popped this off.
            let (dtype, ndims) = unpack_ndarray_var_tys(&mut ctx.unifier, ret_ty);
            let dtype_llvm = ctx.get_llvm_type(dtype);
            let ndims = extract_ndims(&ctx.unifier, ndims);
            let ndarray = NDArrayType::create(ctx, dtype_llvm, ndims).construct(ctx, None)?;

            // NOTE: Current content of `ndarray`:
            //   - * `data` - **NOT YET** allocated.
            //   - * `itemsize` - initialized to be size_of(dtype).
            //   - * `ndims` - initialized.
            //   - * `shape` - allocated; has uninitialized values.
            //   - * `strides` - allocated; has uninitialized values.

            let stackptr = call_stacksave(ctx, None)?;

            let itemsize = ctx.sizeof(ndarray.ty.object.dtype);
            let sizeof_ptr = ctx.sizeof(ctx.ptr);
            let sizeof_shape = ndims * ctx.sizeof(ctx.size_t);
            // Size of the buffer for the initial `rpc_recv()`.
            let unaligned_buffer_size = sizeof_ptr + sizeof_shape;

            // Force an aligned allocation.
            let buffer = type_aligned_allocate(
                ctx,
                AllocationScope::StackCurrentLoc,
                llvm_i8_8,
                ctx.size_t.const_int(unaligned_buffer_size, false),
                Some("rpc.buffer"),
            )?;

            // The first call to `rpc_recv` reads the top-level ndarray object: [pdata, shape]
            //
            // The returned value is the number of bytes for `ndarray.data`.
            let ndarray_nbytes = ctx
                .build_call_or_invoke(
                    &rpc_recv,
                    &[buffer.value.0.into()], // Reads [usize; ndims]
                    "rpc.size.next",
                )?
                .map(BasicValueEnum::into_int_value)
                .unwrap();

            // debug_assert(ndarray_nbytes > 0)
            if ctx.registry.codegen_options.debug {
                let cmp =
                    ctx.builder.build_int_compare(IntPredicate::UGT, ndarray_nbytes, num_0, "")?;

                ctx.make_assert(
                    cmp,
                    "0:AssertionError",
                    "Unexpected RPC termination for ndarray - Expected data buffer next",
                    [None, None, None],
                    ctx.current_loc,
                )?;
            }

            // Copy shape from the buffer to `ndarray.shape`.
            // We need to skip the first `sizeof(ptr)` bytes to skip the `pdata` in `[pdata, shape]`.
            let sizeof_ptr = ctx.size_t.const_int(sizeof_ptr, false);
            let pbuffer_shape = buffer.cast(ctx, ctx.i8, None, None)?.ptr_offset_unchecked(
                ctx,
                &sizeof_ptr,
                None,
            )?;
            ndarray.shape(ctx)?.inner_value(ctx, None)?.memcpy_from(ctx, pbuffer_shape)?;

            // Restore stack from before allocation of buffer
            call_stackrestore(ctx, stackptr)?;

            // Allocate `ndarray.data`.
            // `ndarray.shape` must be initialized beforehand in this implementation
            //   (for ndarray.create_data() to know how many elements to allocate)
            ndarray.create_data(ctx)?; // NOTE: the strides of `ndarray` has also been set to contiguous in `create_data`.

            let itemsize = ctx.size_t.const_int(itemsize, false);
            // debug_assert(nelems * sizeof(T) >= ndarray_nbytes)
            if ctx.registry.codegen_options.debug {
                let num_elements = ndarray.size(ctx)?;

                let expected_ndarray_nbytes =
                    ctx.builder.build_int_mul(num_elements, itemsize, "")?;
                let cmp = ctx.builder.build_int_compare(
                    IntPredicate::UGE,
                    expected_ndarray_nbytes,
                    ndarray_nbytes,
                    "",
                )?;

                ctx.make_assert(
                    cmp,
                    "0:AssertionError",
                    "Unexpected allocation size request for ndarray data - Expected up to {0} bytes, got {1} bytes",
                    [Some(expected_ndarray_nbytes), Some(ndarray_nbytes), None],
                    ctx.current_loc,
                )?;
            }

            let ndarray_offset = ndarray.inner_value(ctx)?.load(ctx, field!(offset))?;
            let ndarray_num_elements = ndarray.size(ctx)?;
            let ndarray_data = ndarray
                .inner_value(ctx)?
                .base_data(ctx)?
                .inner_value(ctx, Some(ndarray_num_elements))?
                .cast(ctx, ctx.i8, None, None)?
                .ptr_offset_unchecked(ctx, &ndarray_offset, None)?;

            gen_rpc_recv_loop(ctx, &rpc_recv, ndarray_data, |ctx, alloc_size| {
                // Align the allocation to sizeof(T)
                let alloc_size = round_up(ctx, alloc_size, itemsize)?;
                let size = ctx.builder.build_int_unsigned_div(alloc_size, itemsize, "")?;
                Ok(ctx
                    .build_dyn_array_allocate(
                        AllocationScope::Default,
                        dtype_llvm,
                        size,
                        Some("rpc.alloc"),
                    )?
                    .value
                    .0)
            })?;

            ndarray.value.into()
        }

        _ => {
            // The slot must be sized to the wire layout, which diverges when `ret_ty` contains an
            // inline `ndarray` field.
            let wire_ret_ty = wire_type_of(ctx, ret_ty);
            let slot = ctx.build_allocate(
                AllocationScope::StackStartOfFunc,
                wire_ret_ty,
                Some("rpc.ret.slot"),
            )?;

            gen_rpc_recv_loop(ctx, &rpc_recv, slot, |ctx, alloc_size| {
                Ok(ctx
                    .build_dyn_array_allocate(
                        AllocationScope::Default,
                        llvm_pi8,
                        alloc_size,
                        Some("rpc.alloc"),
                    )?
                    .value
                    .0)
            })?;

            // Obtain the wire buffer for demarshaling:
            // - For refcounted types the slot is a pointer to the wire buffer
            // - For inline types the slot is the wire buffer itself
            let wire_buf = if is_refcounted_type(&mut ctx.unifier, ret_ty) {
                ctx.builder.build_load(ctx.ptr, slot, "rpc.wire")?.into_pointer_value()
            } else {
                slot
            };
            demarshal_from_wire(ctx, ret_ty, wire_buf)?
        }
    };

    Ok(Some(result))
}

fn rpc_codegen_callback_fn<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    obj: Option<(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: Vec<(Option<StrRef>, ValueEnum<'ctx>)>,
    is_async: bool,
) -> anyhow::Result<Option<BasicValueEnum<'ctx>>> {
    let int8 = ctx.i8;
    let int32 = ctx.i32;
    let size_type = ctx.size_t;
    let ptr_type = ctx.ptr;
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
                format!("tagptr{}", fun.1.0).as_str(),
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

    let arg_length = args.len() as u64 + u64::from(obj.is_some());

    let stackptr = call_stacksave(ctx, Some("rpc.stack"))?;
    let args_ptr = ctx.build_array_allocate(
        AllocationScope::StackCurrentLoc,
        ctx.ptr,
        arg_length,
        Some("argptr"),
    )?;

    // -- rpc args handling
    let mut keys = fun.0.args.clone();
    let mut mapping = HashMap::new();
    for (key, value) in args {
        mapping.insert(key.unwrap_or_else(|| keys.remove(0).name), value);
    }
    // default value handling
    for k in keys {
        mapping.insert(k.name, ctx.gen_symbol_val(&k.default_value.unwrap(), k.ty)?.into());
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
                .to_basic_value_enum(ctx, arg.ty)
                .map(|llvm_val| (llvm_val, arg.ty))
        })
        .collect::<Result<Vec<(_, _)>, _>>()?;
    if let Some(obj) = obj {
        if let ValueEnum::Static(obj_val) = obj.1 {
            real_params.insert(0, (obj_val.get_const_obj(ctx)?, obj.0));
        } else {
            // should be an error here...
            panic!("only host object is allowed");
        }
    }

    for (i, (arg, arg_ty)) in real_params.iter().enumerate() {
        let arg_slot = format_rpc_arg(ctx, (*arg, *arg_ty, i))?;
        let name = format!("rpc.arg{i}");
        let i = ctx.size_t.const_int(i as u64, false);
        args_ptr.set_unchecked(ctx, &i, arg_slot, Some(&name))?;
    }

    call_extern!(ctx: void "rpc.send" =
        (if is_async { "rpc_send_async" } else { "rpc_send" })(service_id, tag_ptr, args_ptr.value.0))?;

    // reclaim stack space used by arguments
    call_stackrestore(ctx, stackptr)?;

    if is_async {
        // async RPCs do not return any values
        Ok(None)
    } else {
        let result = format_rpc_ret(ctx, fun.0.ret)?;

        // Here we call `basic_type_all` to ensure that the return type is not, nor contains, a
        // pointer type which may require further allocation, in which case the stack should not
        // be restored, as this will lead to undefined behavior.
        if result.is_some_and(|res| basic_type_all(&res.get_type(), &|t| !t.is_pointer_type())) {
            call_stackrestore(ctx, stackptr)?;
        }

        Ok(result)
    }
}

pub fn attributes_writeback<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    inner_resolver: &InnerResolver,
    host_attributes: &Py<PyAny>,
    return_obj: Option<(Type, ValueEnum<'ctx>)>,
) -> anyhow::Result<()> {
    Python::attach(|py| -> anyhow::Result<()> {
        let host_attributes = host_attributes.cast_bound::<PyList>(py).map_err(PyErr::from)?;
        let int32 = ctx.i32;
        let zero = int32.const_zero();
        let mut values = Vec::new();
        let mut scratch_buffer = Vec::new();

        if let Some((ty, obj)) = return_obj {
            values.push((ty, obj.to_basic_value_enum(ctx, ty)?));
        }

        for val in (*inner_resolver.global_value_ids.read()).values() {
            let val = val.bind(py);
            let ty = inner_resolver.get_obj_type(
                py,
                val,
                &mut ctx.unifier,
                &ctx.top_level.definitions.read(),
                &ctx.primitives,
            )?;
            match &*ctx.unifier.get_ty(ty) {
                TypeEnum::TObj { obj_id, params, .. } if *obj_id == PrimDef::List.id() => {
                    let elem_ty = iter_type_vars(params).next().unwrap().ty;

                    if gen_rpc_tag(ctx, elem_ty, &mut scratch_buffer).is_ok() {
                        let pydict = PyDict::new(py);
                        pydict.set_item("obj", val)?;
                        host_attributes.append(pydict)?;
                        let value = inner_resolver.get_obj_value(py, val, ctx, ty)?.unwrap();
                        values.push((ty, value));
                    }
                }
                TypeEnum::TObj { fields, obj_id, .. }
                    if *obj_id != ctx.primitives.option.obj_id(&ctx.unifier).unwrap() =>
                {
                    // we only care about primitive attributes
                    // for non-primitive attributes, they should be in another global
                    let mut attributes = Vec::new();
                    let obj = inner_resolver.get_obj_value(py, val, ctx, ty)?.unwrap();

                    // Walk fields in source declaration order so the writeback's tag string and
                    // the host-side `attributes_writeback` list stay deterministic between
                    // compiles (the unifier's `fields` map is a HashMap and would otherwise
                    // randomize the order each run).
                    let source_field_names: Vec<StrRef> = {
                        let top_level_defs = ctx.top_level.definitions.read();
                        let TopLevelDef::Class { fields: source_fields, .. } =
                            &*top_level_defs[obj_id.0].read()
                        else {
                            unreachable!()
                        };
                        source_fields.iter().map(|(name, _, _)| *name).collect()
                    };

                    for name in source_field_names {
                        let Some(&(field_ty, attr_kind)) = fields.get(&name) else {
                            continue;
                        };
                        if !attr_kind.is_mutable() {
                            continue;
                        }
                        if gen_rpc_tag(ctx, field_ty, &mut scratch_buffer).is_ok() {
                            attributes.push(name.to_string());
                            let (index, _) = ctx.get_attr_index(ty, name);

                            let field_val = if is_refcounted_type(&mut ctx.unifier, ty) {
                                let class_val = ClassType::from_unifier_type(ctx, ty)
                                    .map_value(obj.into_pointer_value(), None);
                                class_val.inner_value(ctx)?.load_field(ctx, index as u32)?
                            } else {
                                let obj_alloca_ty = ctx.get_alloca_type(ty);
                                let field_llvm_ty = obj_alloca_ty
                                    .into_struct_type()
                                    .get_field_type_at_index(index as u32)
                                    .unwrap();
                                ctx.build_gep_and_load(
                                    obj_alloca_ty,
                                    obj.into_pointer_value(),
                                    &[zero, int32.const_int(index as u64, false)],
                                    None,
                                    field_llvm_ty,
                                )?
                            };
                            values.push((field_ty, field_val));
                        }
                    }
                    if !attributes.is_empty() {
                        let pydict = PyDict::new(py);
                        pydict.set_item("obj", val)?;
                        pydict.set_item("fields", attributes)?;
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
        rpc_codegen_callback_fn(ctx, None, (&fun, DefinitionId(0)), args, true)?;
        anyhow::Ok(())
    })?;
    Ok(())
}

pub fn rpc_codegen_callback(is_async: bool) -> Arc<GenCall> {
    Arc::new(GenCall::new(Box::new(move |ctx, obj, fun, args| {
        rpc_codegen_callback_fn(ctx, obj, fun, args, is_async)
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
    values: &[(Type, ValueEnum<'ctx>)],
    separator: &str,
    suffix: Option<&str>,
    as_repr: bool,
    as_rtio: bool,
) -> anyhow::Result<()> {
    let printf = |ctx: &mut CodeGenContext<'ctx, '_>,
                  fmt: String,
                  args: Vec<BasicValueEnum<'ctx>>|
     -> anyhow::Result<()> {
        debug_assert!(!fmt.is_empty());
        debug_assert_eq!(fmt.as_bytes().last().unwrap(), &0u8);

        let llvm_i32 = ctx.i32;

        let fmt = ctx.gen_string(fmt)?;
        let fmt = unsafe { fmt.get_field_at_index_unchecked(0) }.into_pointer_value();

        if as_rtio {
            call_extern!(ctx: void _ = "rtio_log"(fmt; ...args))?;
        } else {
            call_extern!(ctx: llvm_i32 _ = "core_log"(fmt; ...args))?;
        }

        Ok(())
    };

    let llvm_i32 = ctx.i32;
    let llvm_i64 = ctx.i64;
    let llvm_usize = ctx.size_t;

    let suffix = suffix.unwrap_or_default();

    let mut fmt = String::new();
    let mut args = Vec::new();

    let flush = |ctx: &mut CodeGenContext<'ctx, '_>,
                 fmt: &mut String,
                 args: &mut Vec<BasicValueEnum<'ctx>>| {
        if !fmt.is_empty() {
            fmt.push('\0');
            printf(ctx, mem::take(fmt), mem::take(args))?;
        }
        anyhow::Ok(())
    };

    for (ty, value) in values {
        let ty = *ty;
        let value = value.to_basic_value_enum(ctx, ty)?;

        if !fmt.is_empty() {
            fmt.push_str(separator);
        }

        match &*ctx.unifier.get_ty_immutable(ty) {
            TypeEnum::TTuple { ty: tys, is_vararg_ctx: false } => {
                let pvalue = {
                    let pvalue = ctx.build_allocate(
                        AllocationScope::StackStartOfFunc,
                        value.get_type(),
                        None,
                    )?;
                    ctx.builder.build_store(pvalue, value)?;
                    pvalue
                };

                fmt.push('(');
                flush(ctx, &mut fmt, &mut args)?;

                let value_struct_ty = value.get_type().into_struct_type();
                let tuple_vals = tys
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| {
                        anyhow::Ok((*ty, {
                            let field_ty =
                                value_struct_ty.get_field_type_at_index(i as u32).unwrap();
                            let pfield = ctx.builder.build_struct_gep(
                                value_struct_ty,
                                pvalue,
                                i as u32,
                                "",
                            )?;
                            ValueEnum::from(ctx.builder.build_load(field_ty, pfield, "")?)
                        }))
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                polymorphic_print(ctx, &tuple_vals, ", ", None, true, as_rtio)?;

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

                let true_str = ctx.gen_string("True")?;
                let true_data =
                    unsafe { true_str.get_field_at_index_unchecked(0) }.into_pointer_value();
                let true_len = unsafe { true_str.get_field_at_index_unchecked(1) }.into_int_value();
                let false_str = ctx.gen_string("False")?;
                let false_data =
                    unsafe { false_str.get_field_at_index_unchecked(0) }.into_pointer_value();
                let false_len =
                    unsafe { false_str.get_field_at_index_unchecked(1) }.into_int_value();

                let bool_val = bool_to_i1(ctx, value.into_int_value())?;

                args.extend([
                    ctx.builder.build_select(bool_val, true_len, false_len, "")?,
                    ctx.builder.build_select(bool_val, true_data, false_data, "")?,
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
                flush(ctx, &mut fmt, &mut args)?;

                let val = ListType::from_unifier_type(ctx, ty)
                    .map_value(value.into_pointer_value(), None);
                let len = val.inner_value(ctx)?.load(ctx, field!(len))?;
                let last = ctx.builder.build_int_sub(len, llvm_usize.const_int(1, false), "")?;

                gen_for_callback_incrementing(
                    &mut (),
                    ctx,
                    None,
                    llvm_usize.const_zero(),
                    (len, false),
                    |(), ctx, _, i| {
                        let elem = val
                            .inner_value(ctx)?
                            .data(ctx)?
                            .inner_value(ctx, Some(len))?
                            .get_unchecked(ctx, &i, None)?;

                        polymorphic_print(ctx, &[(elem_ty, elem)], "", None, true, as_rtio)?;

                        gen_if_callback(
                            &mut (),
                            ctx,
                            |(), ctx| {
                                Ok(ctx.builder.build_int_compare(IntPredicate::ULT, i, last, "")?)
                            },
                            |(), ctx| {
                                printf(ctx, ", \0".into(), Vec::default())?;

                                Ok(())
                            },
                            |(), _| Ok(()),
                        )?;

                        Ok(())
                    },
                    llvm_usize.const_int(1, false),
                    |(), _| Ok(()),
                )?;

                fmt.push(']');
                flush(ctx, &mut fmt, &mut args)?;
            }

            TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::NDArray.id() => {
                fmt.push_str("array([");
                flush(ctx, &mut fmt, &mut args)?;

                let (dtype, _) = unpack_ndarray_var_tys(&mut ctx.unifier, ty);
                let ndarray = NDArrayType::from_unifier_type(ctx, ty)
                    .map_value(value.into_pointer_value(), None);

                let num_0 = llvm_usize.const_zero();

                // Print `ndarray` as a flat list delimited by interspersed with ", \0"
                ndarray.foreach(ctx, |ctx, _, hdl| {
                    let i = hdl.inner_value(ctx)?.get_index(ctx)?;
                    let scalar = hdl.inner_value(ctx)?.get_scalar(ctx)?;

                    // if (i != 0) puts(", ");
                    gen_if_callback(
                        &mut (),
                        ctx,
                        |(), ctx| {
                            let not_first =
                                ctx.builder.build_int_compare(IntPredicate::NE, i, num_0, "")?;
                            Ok(not_first)
                        },
                        |(), ctx| {
                            printf(ctx, ", \0".into(), Vec::default())?;
                            Ok(())
                        },
                        |(), _| Ok(()),
                    )?;

                    // Print element
                    polymorphic_print(ctx, &[(dtype, scalar.into())], "", None, true, as_rtio)?;
                    Ok(())
                })?;

                fmt.push_str(")]");
                flush(ctx, &mut fmt, &mut args)?;
            }

            TypeEnum::TObj { obj_id, .. } if *obj_id == PrimDef::Range.id() => {
                fmt.push_str("range(");
                flush(ctx, &mut fmt, &mut args)?;

                let val = RangeType::new(ctx).map_value(value.into_pointer_value(), None);

                let (start, stop, step) = destructure_range(ctx, val)?;

                polymorphic_print(
                    ctx,
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

                let exn = ExceptionType::new(ctx).map_value(value.into_pointer_value(), None);
                let name = exn.load(ctx, field!(name))?;
                let param0 = exn.load(ctx, field!(param0))?;
                let param1 = exn.load(ctx, field!(param1))?;
                let param2 = exn.load(ctx, field!(param2))?;

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
    flush(ctx, &mut fmt, &mut args)?;

    Ok(())
}

/// Invokes the `core_log` intrinsic function.
pub fn call_core_log_impl<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    arg: (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<()> {
    let (arg_ty, arg_val) = arg;

    polymorphic_print(ctx, &[(arg_ty, arg_val.into())], " ", Some("\n"), false, false)?;

    Ok(())
}

/// Invokes the `rtio_log` intrinsic function.
pub fn call_rtio_log_impl<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    channel: StructValue<'ctx>,
    arg: (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<()> {
    let (arg_ty, arg_val) = arg;

    polymorphic_print(
        ctx,
        &[(ctx.primitives.str, channel.into())],
        " ",
        Some("\x1E"),
        false,
        true,
    )?;
    polymorphic_print(ctx, &[(arg_ty, arg_val.into())], " ", Some("\x1D"), false, true)?;

    Ok(())
}

/// Generates a call to `core_log`.
pub fn gen_core_log<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    obj: Option<&(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
) -> anyhow::Result<()> {
    assert!(obj.is_none());
    assert_eq!(args.len(), 1);

    let value_ty = fun.0.args[0].ty;
    let value_arg = args[0].1.clone().to_basic_value_enum(ctx, value_ty)?;

    call_core_log_impl(ctx, (value_ty, value_arg))
}

/// Generates a call to `rtio_log`.
pub fn gen_rtio_log<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    obj: Option<&(Type, ValueEnum<'ctx>)>,
    fun: (&FunSignature, DefinitionId),
    args: &[(Option<StrRef>, ValueEnum<'ctx>)],
) -> anyhow::Result<()> {
    assert!(obj.is_none());
    assert_eq!(args.len(), 2);

    let channel_ty = fun.0.args[0].ty;
    assert!(ctx.unifier.unioned(channel_ty, ctx.primitives.str));
    let channel_arg = args[0].1.clone().to_basic_value_enum(ctx, channel_ty)?.into_struct_value();
    let value_ty = fun.0.args[1].ty;
    let value_arg = args[1].1.clone().to_basic_value_enum(ctx, value_ty)?;

    call_rtio_log_impl(ctx, channel_arg, (value_ty, value_arg))
}
