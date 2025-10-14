use inkwell::{
    types::IntType,
    values::{IntValue, PointerValue, StructValue},
};
use itertools::Itertools;

use nac3parser::ast::{Expr, ExprKind};

use crate::{
    codegen::{
        CodeGenContext, CodeGenerator, irrt,
        stmt::gen_var,
        types::{
            ndarray::{NDArrayType, NDIndexType},
            structure::{StructField, StructProxyType},
            utils::SliceType,
        },
        values::{
            ProxyValue, ndarray::NDArrayValue, structure::StructProxyValue, utils::RustSlice,
        },
    },
    typecheck::typedef::Type,
};

/// An IRRT representation of an ndarray subscript index.
#[derive(Copy, Clone)]
pub struct NDIndexValue<'ctx> {
    value: PointerValue<'ctx>,
    llvm_usize: IntType<'ctx>,
    name: Option<&'ctx str>,
}

impl<'ctx> NDIndexValue<'ctx> {
    /// Creates an [`NDIndexValue`] from a [`StructValue`].
    #[must_use]
    pub fn from_struct_value(
        ctx: &mut CodeGenContext<'ctx, '_>,
        val: StructValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        let pval =
            gen_var(ctx, val.get_type().into(), name.map(|name| format!("{name}.addr")).as_deref())
                .unwrap();
        ctx.builder.build_store(pval, val).unwrap();
        Self::from_pointer_value(pval, llvm_usize, name)
    }

    /// Creates an [`NDIndexValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_pointer_value(
        ptr: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        debug_assert!(Self::is_instance(ptr, llvm_usize).is_ok());

        Self { value: ptr, llvm_usize, name }
    }

    fn type_field(&self) -> StructField<'ctx, IntValue<'ctx>> {
        self.get_type().get_fields().type_
    }

    pub fn load_type(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        self.type_field().load(ctx, self.value, self.name)
    }

    pub fn store_type(&self, ctx: &mut CodeGenContext<'ctx, '_>, value: IntValue<'ctx>) {
        self.type_field().store(ctx, self.value, value, self.name);
    }

    fn data_field(&self) -> StructField<'ctx, PointerValue<'ctx>> {
        self.get_type().get_fields().data
    }

    pub fn load_data(&self, ctx: &mut CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        self.data_field().load(ctx, self.value, self.name)
    }

    pub fn store_data(&self, ctx: &mut CodeGenContext<'ctx, '_>, value: PointerValue<'ctx>) {
        self.data_field().store(ctx, self.value, value, self.name);
    }
}

impl<'ctx> ProxyValue<'ctx> for NDIndexValue<'ctx> {
    type ABI = PointerValue<'ctx>;
    type Base = PointerValue<'ctx>;
    type Type = NDIndexType<'ctx>;

    fn get_type(&self) -> Self::Type {
        Self::Type::from_pointer_type(self.value.get_type(), self.llvm_usize)
    }

    fn as_base_value(&self) -> Self::Base {
        self.value
    }

    fn as_abi_value(&self, _: &CodeGenContext<'ctx, '_>) -> Self::ABI {
        self.as_base_value()
    }
}

impl<'ctx> StructProxyValue<'ctx> for NDIndexValue<'ctx> {}

impl<'ctx> From<NDIndexValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: NDIndexValue<'ctx>) -> Self {
        value.as_base_value()
    }
}

impl<'ctx> NDArrayValue<'ctx> {
    /// Get the expected `ndims` after indexing with `indices`.
    #[must_use]
    fn deduce_ndims_after_indexing_with(&self, indices: &[RustNDIndex<'ctx>]) -> u64 {
        let mut ndims = self.ndims;

        for index in indices {
            match index {
                RustNDIndex::SingleElement(_) => {
                    ndims -= 1; // Single elements decrements ndims
                }
                RustNDIndex::NewAxis => {
                    ndims += 1; // `np.newaxis` / `none` adds a new axis
                }
                RustNDIndex::Ellipsis | RustNDIndex::Slice(_) => {}
            }
        }

        ndims
    }

    /// Index into the ndarray, and return a newly-allocated view on this ndarray.
    ///
    /// This function behaves like NumPy's ndarray indexing, but if the indices index
    /// into a single element, an unsized ndarray is returned.
    #[must_use]
    pub fn index(&self, ctx: &mut CodeGenContext<'ctx, '_>, indices: &[RustNDIndex<'ctx>]) -> Self {
        let dst_ndims = self.deduce_ndims_after_indexing_with(indices);
        let dst_ndarray =
            NDArrayType::new(ctx, self.dtype, dst_ndims).construct_uninitialized(ctx, None);

        let indices = NDIndexType::new(ctx).construct_ndindices(ctx, indices);
        irrt::ndarray::call_nac3_ndarray_index(ctx, indices, *self, dst_ndarray);

        dst_ndarray
    }
}

/// A convenience enum representing a [`NDIndexValue`].
// TODO: Rename to CTConstNDIndex
#[derive(Debug, Clone)]
pub enum RustNDIndex<'ctx> {
    SingleElement(IntValue<'ctx>),
    Slice(RustSlice<'ctx>),
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
    ) -> Result<Vec<RustNDIndex<'ctx>>, String> {
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
                let slice = RustSlice::from_slice_expr(generator, ctx, lower, upper, step)?;
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

    /// Get the value to set `NDIndex::type` for this variant.
    #[must_use]
    pub fn get_type_id(&self) -> u64 {
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
    ) {
        let llvm_pi8 = ctx.ptr;

        // Set `dst_ndindex.type`
        dst_ndindex.store_type(ctx, ctx.i8.const_int(self.get_type_id(), false));

        // Set `dst_ndindex_ptr->data`
        match self {
            RustNDIndex::SingleElement(in_index) => {
                let index_ptr = ctx.builder.build_alloca(ctx.i32, "").unwrap();
                ctx.builder.build_store(index_ptr, *in_index).unwrap();

                dst_ndindex.store_data(
                    ctx,
                    ctx.builder.build_pointer_cast(index_ptr, llvm_pi8, "").unwrap(),
                );
            }
            RustNDIndex::Slice(in_rust_slice) => {
                let user_slice_ptr = SliceType::new(ctx, ctx.i32).alloca_var(ctx, None);
                in_rust_slice.write_to_slice(ctx, user_slice_ptr);

                dst_ndindex.store_data(
                    ctx,
                    ctx.builder.build_pointer_cast(user_slice_ptr.into(), llvm_pi8, "").unwrap(),
                );
            }
            RustNDIndex::NewAxis | RustNDIndex::Ellipsis => {}
        }
    }
}
