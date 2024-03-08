use inkwell::{
    IntPredicate,
    types::{AnyTypeEnum, BasicTypeEnum, IntType, PointerType},
    values::{ArrayValue, BasicValueEnum, IntValue, PointerValue},
};
use crate::codegen::{
    CodeGenContext,
    CodeGenerator,
    irrt::{call_ndarray_calc_size, call_ndarray_flatten_index, call_ndarray_flatten_index_const},
    llvm_intrinsics::call_int_umin,
    stmt::gen_for_callback_incrementing,
};

#[cfg(not(debug_assertions))]
pub fn assert_is_list<'ctx>(_value: PointerValue<'ctx>, _llvm_usize: IntType<'ctx>) {}

#[cfg(debug_assertions)]
pub fn assert_is_list<'ctx>(value: PointerValue<'ctx>, llvm_usize: IntType<'ctx>) {
    if let Err(msg) = ListValue::is_instance(value, llvm_usize) {
        panic!("{msg}")
    }
}

/// Proxy type for accessing a `list` value in LLVM.
#[derive(Copy, Clone)]
pub struct ListValue<'ctx>(PointerValue<'ctx>, Option<&'ctx str>);

impl<'ctx> ListValue<'ctx> {
    /// Checks whether `value` is an instance of `list`, returning [Err] if `value` is not an
    /// instance.
    pub fn is_instance(
        value: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        let llvm_list_ty = value.get_type().get_element_type();
        let AnyTypeEnum::StructType(llvm_list_ty) = llvm_list_ty else {
            return Err(format!("Expected struct type for `list` type, got {llvm_list_ty}"))
        };
        if llvm_list_ty.count_fields() != 2 {
            return Err(format!("Expected 2 fields in `list`, got {}", llvm_list_ty.count_fields()))
        }

        let list_size_ty = llvm_list_ty.get_field_type_at_index(0).unwrap();
        let Ok(_) = PointerType::try_from(list_size_ty) else {
            return Err(format!("Expected pointer type for `list.0`, got {list_size_ty}"))
        };

        let list_data_ty = llvm_list_ty.get_field_type_at_index(1).unwrap();
        let Ok(list_data_ty) = IntType::try_from(list_data_ty) else {
            return Err(format!("Expected int type for `list.1`, got {list_data_ty}"))
        };
        if list_data_ty.get_bit_width() != llvm_usize.get_bit_width() {
            return Err(format!("Expected {}-bit int type for `list.1`, got {}-bit int",
                               llvm_usize.get_bit_width(),
                               list_data_ty.get_bit_width()))
        }

        Ok(())
    }

    /// Creates an [`ListValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_ptr_val(ptr: PointerValue<'ctx>, llvm_usize: IntType<'ctx>, name: Option<&'ctx str>) -> Self {
        assert_is_list(ptr, llvm_usize);
        ListValue(ptr, name)
    }

    /// Returns the underlying [`PointerValue`] pointing to the `list` instance.
    #[must_use]
    pub fn as_ptr_value(&self) -> PointerValue<'ctx> {
        self.0
    }

    /// Returns the double-indirection pointer to the `data` array, as if by calling `getelementptr`
    /// on the field.
    fn pptr_to_data(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.1.map(|v| format!("{v}.data.addr")).unwrap_or_default();

        unsafe {
            ctx.builder.build_in_bounds_gep(
                self.as_ptr_value(),
                &[llvm_i32.const_zero(), llvm_i32.const_zero()],
                var_name.as_str(),
            ).unwrap()
        }
    }

    /// Returns the pointer to the field storing the size of this `list`.
    fn ptr_to_size(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.1.map(|v| format!("{v}.size.addr")).unwrap_or_default();

        unsafe {
            ctx.builder.build_in_bounds_gep(
                self.0,
                &[llvm_i32.const_zero(), llvm_i32.const_int(1, true)],
                var_name.as_str(),
            ).unwrap()
        }
    }

    /// Stores the array of data elements `data` into this instance.
    fn store_data(&self, ctx: &CodeGenContext<'ctx, '_>, data: PointerValue<'ctx>) {
        ctx.builder.build_store(self.pptr_to_data(ctx), data).unwrap();
    }

    /// Convenience method for creating a new array storing data elements with the given element
    /// type `elem_ty` and `size`.
    ///
    /// If `size` is [None], the size stored in the field of this instance is used instead.
    pub fn create_data(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        elem_ty: BasicTypeEnum<'ctx>,
        size: Option<IntValue<'ctx>>,
    ) {
        let size = size.unwrap_or_else(|| self.load_size(ctx, None));
        self.store_data(ctx, ctx.builder.build_array_alloca(elem_ty, size, "").unwrap());
    }

    /// Returns the double-indirection pointer to the `data` array, as if by calling `getelementptr`
    /// on the field.
    #[must_use]
    pub fn data(&self) -> ListDataProxy<'ctx> {
        ListDataProxy(*self)
    }

    /// Stores the `size` of this `list` into this instance.
    pub fn store_size(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &dyn CodeGenerator,
        size: IntValue<'ctx>,
    ) {
        debug_assert_eq!(size.get_type(), generator.get_size_type(ctx.ctx));

        let psize = self.ptr_to_size(ctx);
        ctx.builder.build_store(psize, size).unwrap();
    }

    /// Returns the size of this `list` as a value.
    pub fn load_size(&self, ctx: &CodeGenContext<'ctx, '_>, name: Option<&str>) -> IntValue<'ctx> {
        let psize = self.ptr_to_size(ctx);
        let var_name = name
            .map(ToString::to_string)
            .or_else(|| self.1.map(|v| format!("{v}.size")))
            .unwrap_or_default();

        ctx.builder.build_load(psize, var_name.as_str())
            .map(BasicValueEnum::into_int_value)
            .unwrap()
    }
}

impl<'ctx> From<ListValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: ListValue<'ctx>) -> Self {
        value.as_ptr_value()
    }
}

/// Proxy type for accessing the `data` array of an `list` instance in LLVM.
#[derive(Copy, Clone)]
pub struct ListDataProxy<'ctx>(ListValue<'ctx>);

impl<'ctx> ListDataProxy<'ctx> {
    /// Returns the single-indirection pointer to the array.
    pub fn as_ptr_value(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let var_name = self.0.1.map(|v| format!("{v}.data")).unwrap_or_default();

        ctx.builder.build_load(self.0.pptr_to_data(ctx), var_name.as_str())
            .map(BasicValueEnum::into_pointer_value)
            .unwrap()
    }

    /// # Safety
    /// 
    /// This function should be called with a valid index.
    pub unsafe fn ptr_offset_unchecked(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        idx: IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let var_name = name
            .map(|v| format!("{v}.addr"))
            .unwrap_or_default();

        ctx.builder.build_in_bounds_gep(
            self.as_ptr_value(ctx),
            &[idx],
            var_name.as_str(),
        ).unwrap()
    }

    /// Returns the pointer to the data at the `idx`-th index.
    pub fn ptr_offset(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
        idx: IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        debug_assert_eq!(idx.get_type(), generator.get_size_type(ctx.ctx));

        let in_range = ctx.builder.build_int_compare(
            IntPredicate::ULT,
            idx,
            self.0.load_size(ctx, None),
            ""
        ).unwrap();
        ctx.make_assert(
            generator,
            in_range,
            "0:IndexError",
            "list index out of range",
            [None, None, None],
            ctx.current_loc,
        );

        unsafe {
            self.ptr_offset_unchecked(ctx, idx, name)
        }
    }

    /// # Safety
    ///
    /// This function should be called with a valid index.
    pub unsafe fn get_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: IntValue<'ctx>,
        name: Option<&str>,
    ) -> BasicValueEnum<'ctx> {
        let ptr = self.ptr_offset_unchecked(ctx, idx, name);
        ctx.builder.build_load(ptr, name.unwrap_or_default()).unwrap()
    }

    /// Returns the data at the `idx`-th flattened index.
    pub fn get(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
        idx: IntValue<'ctx>,
        name: Option<&str>,
    ) -> BasicValueEnum<'ctx> {
        let ptr = self.ptr_offset(ctx, generator, idx, name);
        ctx.builder.build_load(ptr, name.unwrap_or_default()).unwrap()
    }
}

#[cfg(not(debug_assertions))]
pub fn assert_is_range(_value: PointerValue) {}

#[cfg(debug_assertions)]
pub fn assert_is_range(value: PointerValue) {
    if let Err(msg) = RangeValue::is_instance(value) {
        panic!("{msg}")
    }
}

/// Proxy type for accessing a `range` value in LLVM.
#[derive(Copy, Clone)]
pub struct RangeValue<'ctx>(PointerValue<'ctx>, Option<&'ctx str>);

impl<'ctx> RangeValue<'ctx> {
    /// Checks whether `value` is an instance of `range`, returning [Err] if `value` is not an instance.
    pub fn is_instance(value: PointerValue<'ctx>) -> Result<(), String> {
        let llvm_range_ty = value.get_type().get_element_type();
        let AnyTypeEnum::ArrayType(llvm_range_ty) = llvm_range_ty else {
            return Err(format!("Expected array type for `range` type, got {llvm_range_ty}"))
        };
        if llvm_range_ty.len() != 3 {
            return Err(format!("Expected 3 elements for `range` type, got {}", llvm_range_ty.len()))
        }

        let llvm_range_elem_ty = llvm_range_ty.get_element_type();
        let Ok(llvm_range_elem_ty) = IntType::try_from(llvm_range_elem_ty) else {
            return Err(format!("Expected int type for `range` element type, got {llvm_range_elem_ty}"))
        };
        if llvm_range_elem_ty.get_bit_width() != 32 {
            return Err(format!("Expected 32-bit int type for `range` element type, got {}",
                               llvm_range_elem_ty.get_bit_width()))
        }

        Ok(())
    }

    /// Creates an [`RangeValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_ptr_val(ptr: PointerValue<'ctx>, name: Option<&'ctx str>) -> Self {
        assert_is_range(ptr);
        RangeValue(ptr, name)
    }

    /// Returns the underlying [`PointerValue`] pointing to the `range` instance.
    #[must_use]
    pub fn as_ptr_value(&self) -> PointerValue<'ctx> {
        self.0
    }

    fn ptr_to_start(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.1.map(|v| format!("{v}.start.addr")).unwrap_or_default();

        unsafe {
            ctx.builder.build_in_bounds_gep(
                self.0,
                &[llvm_i32.const_zero(), llvm_i32.const_int(0, false)],
                var_name.as_str(),
            ).unwrap()
        }
    }

    fn ptr_to_end(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.1.map(|v| format!("{v}.end.addr")).unwrap_or_default();

        unsafe {
            ctx.builder.build_in_bounds_gep(
                self.0,
                &[llvm_i32.const_zero(), llvm_i32.const_int(1, false)],
                var_name.as_str(),
            ).unwrap()
        }
    }

    fn ptr_to_step(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.1.map(|v| format!("{v}.step.addr")).unwrap_or_default();

        unsafe {
            ctx.builder.build_in_bounds_gep(
                self.0,
                &[llvm_i32.const_zero(), llvm_i32.const_int(2, false)],
                var_name.as_str(),
            ).unwrap()
        }
    }

    /// Stores the `start` value into this instance.
    pub fn store_start(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        start: IntValue<'ctx>,
    ) {
        debug_assert_eq!(start.get_type().get_bit_width(), 32);

        let pstart = self.ptr_to_start(ctx);
        ctx.builder.build_store(pstart, start).unwrap();
    }

    /// Returns the `start` value of this `range`.
    pub fn load_start(&self, ctx: &CodeGenContext<'ctx, '_>, name: Option<&str>) -> IntValue<'ctx> {
        let pstart = self.ptr_to_start(ctx);
        let var_name = name
            .map(ToString::to_string)
            .or_else(|| self.1.map(|v| format!("{v}.start")))
            .unwrap_or_default();

        ctx.builder.build_load(pstart, var_name.as_str())
            .map(BasicValueEnum::into_int_value)
            .unwrap()
    }

    /// Stores the `end` value into this instance.
    pub fn store_end(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        end: IntValue<'ctx>,
    ) {
        debug_assert_eq!(end.get_type().get_bit_width(), 32);

        let pend = self.ptr_to_start(ctx);
        ctx.builder.build_store(pend, end).unwrap();
    }

    /// Returns the `end` value of this `range`.
    pub fn load_end(&self, ctx: &CodeGenContext<'ctx, '_>, name: Option<&str>) -> IntValue<'ctx> {
        let pend = self.ptr_to_end(ctx);
        let var_name = name
            .map(ToString::to_string)
            .or_else(|| self.1.map(|v| format!("{v}.end")))
            .unwrap_or_default();

        ctx.builder.build_load(pend, var_name.as_str())
            .map(BasicValueEnum::into_int_value)
            .unwrap()
    }

    /// Stores the `step` value into this instance.
    pub fn store_step(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        step: IntValue<'ctx>,
    ) {
        debug_assert_eq!(step.get_type().get_bit_width(), 32);

        let pstep = self.ptr_to_start(ctx);
        ctx.builder.build_store(pstep, step).unwrap();
    }

    /// Returns the `step` value of this `range`.
    pub fn load_step(&self, ctx: &CodeGenContext<'ctx, '_>, name: Option<&str>) -> IntValue<'ctx> {
        let pstep = self.ptr_to_step(ctx);
        let var_name = name
            .map(ToString::to_string)
            .or_else(|| self.1.map(|v| format!("{v}.step")))
            .unwrap_or_default();

        ctx.builder.build_load(pstep, var_name.as_str())
            .map(BasicValueEnum::into_int_value)
            .unwrap()
    }
}

impl<'ctx> From<RangeValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: RangeValue<'ctx>) -> Self {
        value.as_ptr_value()
    }
}

#[cfg(not(debug_assertions))]
pub fn assert_is_ndarray<'ctx>(_value: PointerValue<'ctx>, _llvm_usize: IntType<'ctx>) {}

#[cfg(debug_assertions)]
pub fn assert_is_ndarray<'ctx>(value: PointerValue<'ctx>, llvm_usize: IntType<'ctx>) {
    if let Err(msg) = NDArrayValue::is_instance(value, llvm_usize) {
        panic!("{msg}")
    }
}

/// Proxy type for accessing an `NDArray` value in LLVM.
#[derive(Copy, Clone)]
pub struct NDArrayValue<'ctx>(PointerValue<'ctx>, Option<&'ctx str>);

impl<'ctx> NDArrayValue<'ctx> {
    /// Checks whether `value` is an instance of `NDArray`, returning [Err] if `value` is not an
    /// instance.
    pub fn is_instance(
        value: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
    ) -> Result<(), String> {
        let llvm_ndarray_ty = value.get_type().get_element_type();
        let AnyTypeEnum::StructType(llvm_ndarray_ty) = llvm_ndarray_ty else {
            return Err(format!("Expected struct type for `NDArray` type, got {llvm_ndarray_ty}"))
        };
        if llvm_ndarray_ty.count_fields() != 3 {
            return Err(format!("Expected 3 fields in `NDArray`, got {}", llvm_ndarray_ty.count_fields()))
        }

        let ndarray_ndims_ty = llvm_ndarray_ty.get_field_type_at_index(0).unwrap();
        let Ok(ndarray_ndims_ty) = IntType::try_from(ndarray_ndims_ty) else {
            return Err(format!("Expected int type for `ndarray.0`, got {ndarray_ndims_ty}"))
        };
        if ndarray_ndims_ty.get_bit_width() != llvm_usize.get_bit_width() {
            return Err(format!("Expected {}-bit int type for `ndarray.0`, got {}-bit int",
                               llvm_usize.get_bit_width(),
                               ndarray_ndims_ty.get_bit_width()))
        }

        let ndarray_dims_ty = llvm_ndarray_ty.get_field_type_at_index(1).unwrap();
        let Ok(ndarray_pdims) = PointerType::try_from(ndarray_dims_ty) else {
            return Err(format!("Expected pointer type for `ndarray.1`, got {ndarray_dims_ty}"))
        };
        let ndarray_dims = ndarray_pdims.get_element_type();
        let Ok(ndarray_dims) = IntType::try_from(ndarray_dims) else {
            return Err(format!("Expected pointer-to-int type for `ndarray.1`, got pointer-to-{ndarray_dims}"))
        };
        if ndarray_dims.get_bit_width() != llvm_usize.get_bit_width() {
            return Err(format!("Expected pointer-to-{}-bit int type for `ndarray.1`, got pointer-to-{}-bit int",
                               llvm_usize.get_bit_width(),
                               ndarray_dims.get_bit_width()))
        }

        let ndarray_data_ty = llvm_ndarray_ty.get_field_type_at_index(2).unwrap();
        let Ok(_) = PointerType::try_from(ndarray_data_ty) else {
            return Err(format!("Expected pointer type for `ndarray.2`, got {ndarray_data_ty}"))
        };

        Ok(())
    }

    /// Creates an [`NDArrayValue`] from a [`PointerValue`].
    #[must_use]
    pub fn from_ptr_val(
        ptr: PointerValue<'ctx>,
        llvm_usize: IntType<'ctx>,
        name: Option<&'ctx str>,
    ) -> Self {
        assert_is_ndarray(ptr, llvm_usize);
        NDArrayValue(ptr, name)
    }

    /// Returns the underlying [`PointerValue`] pointing to the `NDArray` instance.
    #[must_use]
    pub fn as_ptr_value(&self) -> PointerValue<'ctx> {
        self.0
    }

    /// Returns the pointer to the field storing the number of dimensions of this `NDArray`.
    fn ptr_to_ndims(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.1.map(|v| format!("{v}.ndims.addr")).unwrap_or_default();

        unsafe {
            ctx.builder.build_in_bounds_gep(
                self.0,
                &[llvm_i32.const_zero(), llvm_i32.const_zero()],
                var_name.as_str(),
            ).unwrap()
        }
    }

    /// Stores the number of dimensions `ndims` into this instance.
    pub fn store_ndims(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &dyn CodeGenerator,
        ndims: IntValue<'ctx>,
    ) {
        debug_assert_eq!(ndims.get_type(), generator.get_size_type(ctx.ctx));

        let pndims = self.ptr_to_ndims(ctx);
        ctx.builder.build_store(pndims, ndims).unwrap();
    }

    /// Returns the number of dimensions of this `NDArray` as a value.
    pub fn load_ndims(&self, ctx: &CodeGenContext<'ctx, '_>) -> IntValue<'ctx> {
        let pndims = self.ptr_to_ndims(ctx);
        ctx.builder.build_load(pndims, "")
            .map(BasicValueEnum::into_int_value)
            .unwrap()
    }

    /// Returns the double-indirection pointer to the `dims` array, as if by calling `getelementptr`
    /// on the field.
    fn ptr_to_dims(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.1.map(|v| format!("{v}.dims.addr")).unwrap_or_default();

        unsafe {
            ctx.builder.build_in_bounds_gep(
                self.as_ptr_value(),
                &[llvm_i32.const_zero(), llvm_i32.const_int(1, true)],
                var_name.as_str(),
            ).unwrap()
        }
    }

    /// Stores the array of dimension sizes `dims` into this instance.
    fn store_dim_sizes(&self, ctx: &CodeGenContext<'ctx, '_>, dims: PointerValue<'ctx>) {
        ctx.builder.build_store(self.ptr_to_dims(ctx), dims).unwrap();
    }

    /// Convenience method for creating a new array storing dimension sizes with the given `size`.
    pub fn create_dim_sizes(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        llvm_usize: IntType<'ctx>,
        size: IntValue<'ctx>,
    ) {
        self.store_dim_sizes(ctx, ctx.builder.build_array_alloca(llvm_usize, size, "").unwrap());
    }

    /// Returns a proxy object to the field storing the size of each dimension of this `NDArray`.
    #[must_use]
    pub fn dim_sizes(&self) -> NDArrayDimsProxy<'ctx> {
        NDArrayDimsProxy(*self)
    }

    /// Returns the double-indirection pointer to the `data` array, as if by calling `getelementptr`
    /// on the field.
    fn ptr_to_data(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.1.map(|v| format!("{v}.data.addr")).unwrap_or_default();

        unsafe {
            ctx.builder.build_in_bounds_gep(
                self.as_ptr_value(),
                &[llvm_i32.const_zero(), llvm_i32.const_int(2, true)],
                var_name.as_str(),
            ).unwrap()
        }
    }

    /// Stores the array of data elements `data` into this instance.
    fn store_data(&self, ctx: &CodeGenContext<'ctx, '_>, data: PointerValue<'ctx>) {
        ctx.builder.build_store(self.ptr_to_data(ctx), data).unwrap();
    }

    /// Convenience method for creating a new array storing data elements with the given element
    /// type `elem_ty` and `size`.
    pub fn create_data(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        elem_ty: BasicTypeEnum<'ctx>,
        size: IntValue<'ctx>,
    ) {
        self.store_data(ctx, ctx.builder.build_array_alloca(elem_ty, size, "").unwrap());
    }

    /// Returns a proxy object to the field storing the data of this `NDArray`.
    #[must_use]
    pub fn data(&self) -> NDArrayDataProxy<'ctx> {
        NDArrayDataProxy(*self)
    }
}

impl<'ctx> From<NDArrayValue<'ctx>> for PointerValue<'ctx> {
    fn from(value: NDArrayValue<'ctx>) -> Self {
        value.as_ptr_value()
    }
}

/// Proxy type for accessing the `dims` array of an `NDArray` instance in LLVM.
#[derive(Copy, Clone)]
pub struct NDArrayDimsProxy<'ctx>(NDArrayValue<'ctx>);

impl<'ctx> NDArrayDimsProxy<'ctx> {
    /// Returns the single-indirection pointer to the array.
    pub fn as_ptr_value(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let var_name = self.0.1.map(|v| format!("{v}.dims")).unwrap_or_default();

        ctx.builder.build_load(self.0.ptr_to_dims(ctx), var_name.as_str())
            .map(BasicValueEnum::into_pointer_value)
            .unwrap()
    }

    /// # Safety
    ///
    /// This function should be called with a valid index.
    pub unsafe fn ptr_offset_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let var_name = name
            .map(|v| format!("{v}.addr"))
            .unwrap_or_default();

        ctx.builder.build_in_bounds_gep(
            self.as_ptr_value(ctx),
            &[idx],
            var_name.as_str(),
        ).unwrap()
    }

    /// Returns the pointer to the size of the `idx`-th dimension.
    pub fn ptr_offset(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
        idx: IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let in_range = ctx.builder.build_int_compare(
            IntPredicate::ULT,
            idx,
            self.0.load_ndims(ctx),
            ""
        ).unwrap();
        ctx.make_assert(
            generator,
            in_range,
            "0:IndexError",
            "index {0} is out of bounds for axis 0 with size {1}",
            [Some(idx), Some(self.0.load_ndims(ctx)), None],
            ctx.current_loc,
        );

        unsafe {
            self.ptr_offset_unchecked(ctx, idx, name)
        }
    }

    /// # Safety
    ///
    /// This function should be called with a valid index.
    pub unsafe fn get_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: IntValue<'ctx>,
        name: Option<&str>,
    ) -> IntValue<'ctx> {
        let ptr = self.ptr_offset_unchecked(ctx, idx, name);
        ctx.builder.build_load(ptr, name.unwrap_or_default())
            .map(BasicValueEnum::into_int_value)
            .unwrap()
    }

    /// Returns the size of the `idx`-th dimension.
    pub fn get(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
        idx: IntValue<'ctx>,
        name: Option<&str>,
    ) -> IntValue<'ctx> {
        let ptr = self.ptr_offset(ctx, generator, idx, name);
        ctx.builder.build_load(ptr, name.unwrap_or_default())
            .map(BasicValueEnum::into_int_value)
            .unwrap()
    }
}

/// Proxy type for accessing the `data` array of an `NDArray` instance in LLVM.
#[derive(Copy, Clone)]
pub struct NDArrayDataProxy<'ctx>(NDArrayValue<'ctx>);

impl<'ctx> NDArrayDataProxy<'ctx> {
    /// Returns the single-indirection pointer to the array.
    pub fn as_ptr_value(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let var_name = self.0.1.map(|v| format!("{v}.data")).unwrap_or_default();

        ctx.builder.build_load(self.0.ptr_to_data(ctx), var_name.as_str())
            .map(BasicValueEnum::into_pointer_value)
            .unwrap()
    }

    /// # Safety
    ///
    /// This function should be called with a valid index.
    pub unsafe fn ptr_to_data_flattened_unchecked(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        idx: IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        ctx.builder.build_in_bounds_gep(
            self.as_ptr_value(ctx),
            &[idx],
            name.unwrap_or_default(),
        ).unwrap()
    }

    /// Returns the pointer to the data at the `idx`-th flattened index.
    pub fn ptr_to_data_flattened(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
        idx: IntValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let ndims = self.0.load_ndims(ctx);
        let dims = self.0.dim_sizes().as_ptr_value(ctx);
        let data_sz = call_ndarray_calc_size(generator, ctx, ndims, dims);

        let in_range = ctx.builder.build_int_compare(
            IntPredicate::ULT,
            idx,
            data_sz,
            ""
        ).unwrap();
        ctx.make_assert(
            generator,
            in_range,
            "0:IndexError",
            "index {0} is out of bounds with size {1}",
            [Some(idx), Some(self.0.load_ndims(ctx)), None],
            ctx.current_loc,
        );

        unsafe {
            self.ptr_to_data_flattened_unchecked(ctx, idx, name)
        }
    }

    /// # Safety
    ///
    /// This function should be called with a valid index.
    pub unsafe fn get_flattened_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: IntValue<'ctx>,
        name: Option<&str>,
    ) -> BasicValueEnum<'ctx> {
        let ptr = self.ptr_to_data_flattened_unchecked(ctx, idx, name);
        ctx.builder.build_load(ptr, name.unwrap_or_default()).unwrap()
    }

    /// Returns the data at the `idx`-th flattened index.
    pub fn get_flattened(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
        idx: IntValue<'ctx>,
        name: Option<&str>,
    ) -> BasicValueEnum<'ctx> {
        let ptr = self.ptr_to_data_flattened(ctx, generator, idx, name);
        ctx.builder.build_load(ptr, name.unwrap_or_default()).unwrap()
    }

    /// # Safety
    ///
    /// This function should be called with valid indices.
    pub unsafe fn ptr_offset_unchecked(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &dyn CodeGenerator,
        indices: ListValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let indices_elem_ty = indices.data().as_ptr_value(ctx).get_type().get_element_type();
        let Ok(indices_elem_ty) = IntType::try_from(indices_elem_ty) else {
            panic!("Expected list[int32] but got {indices_elem_ty}")
        };
        assert_eq!(indices_elem_ty.get_bit_width(), 32, "Expected list[int32] but got {indices_elem_ty}");

        let index = call_ndarray_flatten_index(
            generator,
            ctx,
            self.0,
            indices,
        );

        unsafe {
            ctx.builder.build_in_bounds_gep(
                self.as_ptr_value(ctx),
                &[index],
                name.unwrap_or_default(),
            ).unwrap()
        }
    }

    /// # Safety
    ///
    /// This function should be called with valid indices.
    pub unsafe fn ptr_offset_unchecked_const(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
        indices: ArrayValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let index = call_ndarray_flatten_index_const(
            generator,
            ctx,
            self.0,
            indices,
        );

        unsafe {
            ctx.builder.build_in_bounds_gep(
                self.as_ptr_value(ctx),
                &[index],
                name.unwrap_or_default(),
            )
        }.unwrap()
    }

    /// Returns the pointer to the data at the index specified by `indices`.
    pub fn ptr_offset_const(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
        indices: ArrayValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let llvm_usize = generator.get_size_type(ctx.ctx);

        let indices_elem_ty = indices.get_type().get_element_type();
        let Ok(indices_elem_ty) = IntType::try_from(indices_elem_ty) else {
            panic!("Expected [int32] but got [{indices_elem_ty}]")
        };
        assert_eq!(indices_elem_ty.get_bit_width(), 32, "Expected [int32] but got [{indices_elem_ty}]");

        let nidx_leq_ndims = ctx.builder.build_int_compare(
            IntPredicate::SLE,
            llvm_usize.const_int(indices.get_type().len() as u64, false),
            self.0.load_ndims(ctx),
            ""
        ).unwrap();
        ctx.make_assert(
            generator,
            nidx_leq_ndims,
            "0:IndexError",
            "invalid index to scalar variable",
            [None, None, None],
            ctx.current_loc,
        );

        for idx in 0..indices.get_type().len() {
            let i = llvm_usize.const_int(idx as u64, false);

            let dim_idx = ctx.builder
                .build_extract_value(indices, idx, "")
                .map(BasicValueEnum::into_int_value)
                .map(|v| ctx.builder.build_int_z_extend_or_bit_cast(v, llvm_usize, "").unwrap())
                .unwrap();
            let dim_sz = unsafe {
                self.0.dim_sizes().get_unchecked(ctx, i, None)
            };

            let dim_lt = ctx.builder.build_int_compare(
                IntPredicate::SLT,
                dim_idx,
                dim_sz,
                ""
            ).unwrap();

            ctx.make_assert(
                generator,
                dim_lt,
                "0:IndexError",
                "index {0} is out of bounds for axis 0 with size {1}",
                [Some(dim_idx), Some(dim_sz), None],
                ctx.current_loc,
            );
        }

        unsafe {
            self.ptr_offset_unchecked_const(ctx, generator, indices, name)
        }
    }

    /// Returns the pointer to the data at the index specified by `indices`.
    pub fn ptr_offset(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
        indices: ListValue<'ctx>,
        name: Option<&str>,
    ) -> PointerValue<'ctx> {
        let llvm_usize = generator.get_size_type(ctx.ctx);

        let nidx_leq_ndims = ctx.builder.build_int_compare(
            IntPredicate::SLE,
            indices.load_size(ctx, None),
            self.0.load_ndims(ctx),
            ""
        ).unwrap();
        ctx.make_assert(
            generator,
            nidx_leq_ndims,
            "0:IndexError",
            "invalid index to scalar variable",
            [None, None, None],
            ctx.current_loc,
        );

        let indices_len = indices.load_size(ctx, None);
        let ndarray_len = self.0.load_ndims(ctx);
        let len = call_int_umin(ctx, indices_len, ndarray_len, None);
        gen_for_callback_incrementing(
            generator,
            ctx,
            llvm_usize.const_zero(),
            (len, false),
            |generator, ctx, i| {
                let (dim_idx, dim_sz) = unsafe {
                    (
                        indices.data().get_unchecked(ctx, i, None).into_int_value(),
                        self.0.dim_sizes().get_unchecked(ctx, i, None),
                    )
                };

                let dim_lt = ctx.builder.build_int_compare(
                    IntPredicate::SLT,
                    dim_idx,
                    dim_sz,
                    ""
                ).unwrap();

                ctx.make_assert(
                    generator,
                    dim_lt,
                    "0:IndexError",
                    "index {0} is out of bounds for axis 0 with size {1}",
                    [Some(dim_idx), Some(dim_sz), None],
                    ctx.current_loc,
                );

                Ok(())
            },
            llvm_usize.const_int(1, false),
        ).unwrap();

        unsafe {
            self.ptr_offset_unchecked(ctx, generator, indices, name)
        }
    }

    /// # Safety
    ///
    /// This function should be called with valid indices.
    pub unsafe fn get_unchecked_const(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
        indices: ArrayValue<'ctx>,
        name: Option<&str>,
    ) -> BasicValueEnum<'ctx> {
        let ptr = self.ptr_offset_unchecked_const(ctx, generator, indices, name);
        ctx.builder.build_load(ptr, name.unwrap_or_default()).unwrap()
    }

    /// # Safety
    ///
    /// This function should be called with valid indices.
    pub unsafe fn get_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &dyn CodeGenerator,
        indices: ListValue<'ctx>,
        name: Option<&str>,
    ) -> BasicValueEnum<'ctx> {
        let ptr = self.ptr_offset_unchecked(ctx, generator, indices, name);
        ctx.builder.build_load(ptr, name.unwrap_or_default()).unwrap()
    }

    /// Returns the data at the index specified by `indices`.
    pub fn get_const(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
        indices: ArrayValue<'ctx>,
        name: Option<&str>,
    ) -> BasicValueEnum<'ctx> {
        let ptr = self.ptr_offset_const(ctx, generator, indices, name);
        ctx.builder.build_load(ptr, name.unwrap_or_default()).unwrap()
    }

    /// Returns the data at the index specified by `indices`.
    pub fn get(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        generator: &mut dyn CodeGenerator,
        indices: ListValue<'ctx>,
        name: Option<&str>,
    ) -> BasicValueEnum<'ctx> {
        let ptr = self.ptr_offset(ctx, generator, indices, name);
        ctx.builder.build_load(ptr, name.unwrap_or_default()).unwrap()
    }
}
