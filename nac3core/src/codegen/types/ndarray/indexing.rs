use std::borrow::Cow;

use inkwell::values::{IntValue, PointerValue};
use itertools::Itertools as _;
use nac3core_derive::{ProxyType, StructFields};
use nac3parser::ast::{Expr, ExprKind};

use crate::{
    codegen::{
        CodeGenContext, CodeGenerator, ModuleContext,
        allocator::AllocationScope,
        expr::call_extern,
        types::{
            NDArrayType, ProxyTypeBase, RawNDArrayValue, RefType, Value, WithTypeinfo,
            array::{ArrayLikeIndexer, ArraySliceValue},
            builtin::BuiltinStruct,
            field,
            ndarray::NDArrayValue,
            refcounted_fields_for_struct,
            structure::StructField,
        },
    },
    typecheck::typedef::Type,
};

#[derive(Clone, Copy, StructFields)]
pub struct NDIndexStructFields<'ctx> {
    #[value_type(i8)]
    pub type_: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(ptr)]
    pub data: StructField<'ctx, PointerValue<'ctx>>,
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.llvm_ty)]
pub struct NDIndexType<'ctx> {
    inner: BuiltinStruct<'ctx, NDIndexStructFields<'ctx>>,
}

impl<'ctx> WithTypeinfo<'ctx> for NDIndexType<'ctx> {
    fn typename(&self) -> Cow<'static, str> {
        Cow::Borrowed("__nac3_ndindex")
    }

    fn refcounted_fields_data(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> Vec<IntValue<'ctx>> {
        refcounted_fields_for_struct(ctx, Vec::new())
    }
}

impl<'ctx> NDIndexType<'ctx> {
    /// Creates a new instance of [`NDIndexType`].
    pub fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "ndindex") }
    }

    /// Constructs an array of [`NDIndexValue`]s from a list of [`RustNDIndex`].
    pub fn construct(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        in_ndindices: &[RustNDIndex<'ctx>],
    ) -> anyhow::Result<ArraySliceValue<'ctx>> {
        // Allocate the LLVM ndindices.
        let ty = self.alloca_ty(ctx);
        let ndindices = ctx.build_array_allocate(
            AllocationScope::Default,
            ty,
            in_ndindices.len() as u64,
            None,
        )?;

        // Initialize all of them.
        for (i, in_ndindex) in in_ndindices.iter().enumerate() {
            let pndindex =
                ndindices.ptr_offset_unchecked(ctx, &ctx.i64.const_int(i as _, false), None)?;
            in_ndindex.write_to_ndindex(ctx, self.map_value(pndindex, None))?;
        }

        Ok(ndindices)
    }
}

pub type NDIndexValue<'ctx> = Value<'ctx, NDIndexType<'ctx>>;

#[derive(Clone, Copy, StructFields)]
struct SliceStructFields<'ctx> {
    #[value_type(i1)]
    pub start_defined: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(i32)]
    pub start: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(i1)]
    pub stop_defined: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(i32)]
    pub stop: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(i1)]
    pub step_defined: StructField<'ctx, IntValue<'ctx>>,
    #[value_type(i32)]
    pub step: StructField<'ctx, IntValue<'ctx>>,
}

#[derive(Clone, Copy, ProxyType)]
#[llvm_ref(self.inner.llvm_ty)]
pub struct SliceType<'ctx> {
    inner: BuiltinStruct<'ctx, SliceStructFields<'ctx>>,
}

impl<'ctx> WithTypeinfo<'ctx> for SliceType<'ctx> {
    fn typename(&self) -> Cow<'static, str> {
        Cow::Borrowed("__nac3_slice")
    }

    fn refcounted_fields_data(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> Vec<IntValue<'ctx>> {
        refcounted_fields_for_struct(ctx, Vec::new())
    }
}

impl<'ctx> SliceType<'ctx> {
    fn new(ctx: &ModuleContext<'ctx>) -> Self {
        Self { inner: BuiltinStruct::new(ctx, "slice") }
    }
}

pub type SliceValue<'ctx> = Value<'ctx, SliceType<'ctx>>;

impl<'ctx> SliceValue<'ctx> {
    /// Decodes components of a [`Slice`][ExprKind::Slice] expression into a [`SliceValue`].
    #[allow(clippy::type_complexity)]
    #[allow(clippy::ref_option)]
    fn from_slice_expr(
        generator: &mut impl CodeGenerator,
        ctx: &mut CodeGenContext<'ctx, '_>,
        lower: &Option<Box<Expr<Option<Type>>>>,
        upper: &Option<Box<Expr<Option<Type>>>>,
        step: &Option<Box<Expr<Option<Type>>>>,
    ) -> anyhow::Result<Self> {
        fn write_value<'ctx>(
            generator: &mut impl CodeGenerator,
            ctx: &mut CodeGenContext<'ctx, '_>,
            value_expr: &Option<Box<Expr<Option<Type>>>>,
            result: SliceValue<'ctx>,
            defined: impl FnOnce(&SliceType<'ctx>) -> StructField<'ctx, IntValue<'ctx>>,
            val: impl FnOnce(&SliceType<'ctx>) -> StructField<'ctx, IntValue<'ctx>>,
        ) -> anyhow::Result<()> {
            match value_expr {
                // Not defined
                None => result.store(ctx, defined, ctx.i1.const_zero())?,
                Some(value_expr) => {
                    let value = generator.gen_expr(ctx, value_expr)?.to_basic_value_enum(ctx)?;
                    result.store(ctx, defined, ctx.i1.const_int(1, false))?;
                    result.store(ctx, val, value.into_int_value())?;
                }
            }
            Ok(())
        }

        let ty = SliceType::new(ctx);
        let result = ty.allocate(ctx, None)?;

        write_value(generator, ctx, lower, result, field!(start_defined), field!(start))?;
        write_value(generator, ctx, upper, result, field!(stop_defined), field!(stop))?;
        write_value(generator, ctx, step, result, field!(step_defined), field!(step))?;
        Ok(result)
    }
}

/// A convenience enum representing a [`NDIndexValue`].
// TODO: Rename to CTConstNDIndex
#[derive(Clone, Copy)]
pub enum RustNDIndex<'ctx> {
    SingleElement(IntValue<'ctx>),
    Slice(SliceValue<'ctx>),
    NewAxis,
    Ellipsis,
}

impl<'ctx> RustNDIndex<'ctx> {
    /// Generate LLVM code to transform an ndarray subscript expression to
    /// its list of [`RustNDIndex`]
    ///
    /// i.e.,
    /// ```python
    /// my_ndarray[::3, 1, :2:]
    ///            ^^^^^^^^^^^ Then these into a three `RustNDIndex`es
    /// ```
    pub fn from_subscript_expr<G: CodeGenerator>(
        generator: &mut G,
        ctx: &mut CodeGenContext<'ctx, '_>,
        subscript: &Expr<Option<Type>>,
    ) -> anyhow::Result<Vec<Self>> {
        // Annoying notes about `slice`
        //  - `my_array[5]`
        //    - slice is a `Constant`
        //  - `my_array[:5]`
        //    - slice is a `Slice`
        //  - `my_array[:]`
        //    - slice is a `Slice`, but lower upper step would all be `Option::None`
        //  - `my_array[:, :]`
        //    - slice is now a `Tuple` of two `Slice`-s
        //
        // In summary:
        //  - when there is a comma "," within [], `slice` will be a `Tuple` of the entries.
        //  - when there is not comma "," within [] (i.e., just a single entry), `slice` will be that entry itself.
        //
        // So we first "flatten" out the slice expression
        let index_exprs = match &subscript.node {
            ExprKind::Tuple { elts, .. } => elts.iter().collect_vec(),
            _ => vec![subscript],
        };

        // Process all index expressions
        let mut rust_ndindices: Vec<RustNDIndex> = Vec::with_capacity(index_exprs.len()); // Not using iterators here because `?` is used here.
        for index_expr in index_exprs {
            // NOTE: Currently nac3core's slices do not have an object representation,
            // so the code/implementation looks awkward - we have to do pattern matching on the expression
            let ndindex = if let ExprKind::Slice { lower, upper, step } = &index_expr.node {
                // Handle slices
                let slice = SliceValue::from_slice_expr(generator, ctx, lower, upper, step)?;
                RustNDIndex::Slice(slice)
            } else {
                // Treat and handle everything else as a single element index.
                let index =
                    generator.gen_expr(ctx, index_expr)?.to_basic_value_enum(ctx)?.into_int_value();

                RustNDIndex::SingleElement(index)
            };
            rust_ndindices.push(ndindex);
        }
        Ok(rust_ndindices)
    }

    /// Returns the index type for this variant.
    #[must_use]
    const fn get_type_id(&self) -> u64 {
        // Defined in IRRT, must be in sync
        match self {
            RustNDIndex::SingleElement(_) => 0,
            RustNDIndex::Slice(_) => 1,
            RustNDIndex::NewAxis => 2,
            RustNDIndex::Ellipsis => 3,
        }
    }

    /// Serialize this [`RustNDIndex`] by writing it into an LLVM [`NDIndexValue`].
    pub fn write_to_ndindex(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        dst_ndindex: NDIndexValue<'ctx>,
    ) -> anyhow::Result<()> {
        // Set `dst_ndindex.type`
        dst_ndindex.store(ctx, field!(type_), ctx.i8.const_int(self.get_type_id(), false))?;

        // Set `dst_ndindex_ptr->data`
        match *self {
            RustNDIndex::SingleElement(in_index) => {
                let index_ptr = ctx.build_allocate(AllocationScope::Default, ctx.i32, None)?;
                ctx.builder.build_store(index_ptr, in_index)?;
                dst_ndindex.store(ctx, field!(data), index_ptr)?;
            }
            RustNDIndex::Slice(slice) => {
                dst_ndindex.store(ctx, field!(data), slice.value)?;
            }
            RustNDIndex::NewAxis | RustNDIndex::Ellipsis => {}
        }

        Ok(())
    }
}

impl<'ctx> RawNDArrayValue<'ctx> {
    /// Get the expected `ndims` after indexing with `indices`.
    #[must_use]
    fn deduce_ndims_after_indexing_with(&self, indices: &[RustNDIndex<'ctx>]) -> u64 {
        let mut ndims = self.ty.ndims;
        for index in indices {
            match index {
                // Single elements decrements ndims
                RustNDIndex::SingleElement(_) => ndims -= 1,
                // `np.newaxis` / `none` adds a new axis
                RustNDIndex::NewAxis => ndims += 1,

                RustNDIndex::Ellipsis | RustNDIndex::Slice(_) => {}
            }
        }
        ndims
    }
}

impl<'ctx> NDArrayValue<'ctx> {
    /// Index into the ndarray, and return a newly-allocated view on this ndarray.
    ///
    /// This function behaves like NumPy's ndarray indexing, but if the indices index
    /// into a single element, an unsized ndarray is returned.
    pub fn index(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        indices: &[RustNDIndex<'ctx>],
    ) -> anyhow::Result<Self> {
        let dst_ndims = self.inner_value(ctx)?.deduce_ndims_after_indexing_with(indices);
        let dst = NDArrayType::create(ctx, self.inner_value(ctx)?.ty.dtype, dst_ndims)
            .construct(ctx, None)?;
        let indices = NDIndexType::new(ctx).construct(ctx, indices)?;

        let (idx_ptr, idx_len) = indices.value;
        call_extern!(ctx: void _ = "__nac3_ndarray_index"(idx_len, idx_ptr, self.value, dst.value))?;

        Ok(dst)
    }
}
