use inkwell::{
    IntPredicate,
    types::{AnyTypeEnum, BasicTypeEnum, IntType, PointerType},
    values::{BasicValueEnum, IntValue, PointerValue},
};
use crate::codegen::{CodeGenContext, CodeGenerator};

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
            panic!("Expected struct type for `list` type, got {llvm_list_ty}")
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

    /// Creates an [ListValue] from a [PointerValue].
    pub fn from_ptr_val(ptr: PointerValue<'ctx>, llvm_usize: IntType<'ctx>, name: Option<&'ctx str>) -> Self {
        assert_is_list(ptr, llvm_usize);
        ListValue(ptr, name)
    }

    /// Returns the underlying [PointerValue] pointing to the `list` instance.
    pub fn get_ptr(&self) -> PointerValue<'ctx> {
        self.0
    }

    /// Returns the double-indirection pointer to the `data` array, as if by calling `getelementptr`
    /// on the field.
    fn get_data_pptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.1.map(|v| format!("{v}.data.addr")).unwrap_or_default();

        unsafe {
            ctx.builder.build_in_bounds_gep(
                self.get_ptr(),
                &[llvm_i32.const_zero(), llvm_i32.const_zero()],
                var_name.as_str(),
            )
        }
    }

    /// Returns the pointer to the field storing the size of this `list`.
    fn get_size_ptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let llvm_i32 = ctx.ctx.i32_type();
        let var_name = self.1.map(|v| format!("{v}.size.addr")).unwrap_or_default();

        unsafe {
            ctx.builder.build_in_bounds_gep(
                self.0,
                &[llvm_i32.const_zero(), llvm_i32.const_int(1, true)],
                var_name.as_str(),
            )
        }
    }

    /// Stores the array of data elements `data` into this instance.
    fn store_data(&self, ctx: &CodeGenContext<'ctx, '_>, data: PointerValue<'ctx>) {
        ctx.builder.build_store(self.get_data_pptr(ctx), data);
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
        self.store_data(ctx, ctx.builder.build_array_alloca(elem_ty, size, ""));
    }

    /// Returns the double-indirection pointer to the `data` array, as if by calling `getelementptr`
    /// on the field.
    pub fn get_data(&self) -> ListDataProxy<'ctx> {
        ListDataProxy(self.clone())
    }

    /// Stores the `size` of this `list` into this instance.
    pub fn store_size(
        &self,
        ctx: &CodeGenContext<'ctx, '_>,
        generator: &dyn CodeGenerator,
        size: IntValue<'ctx>,
    ) {
        debug_assert_eq!(size.get_type(), generator.get_size_type(ctx.ctx));

        let psize = self.get_size_ptr(ctx);
        ctx.builder.build_store(psize, size);
    }

    /// Returns the size of this `list` as a value.
    pub fn load_size(&self, ctx: &CodeGenContext<'ctx, '_>, name: Option<&str>) -> IntValue<'ctx> {
        let psize = self.get_size_ptr(ctx);
        let var_name = name
            .map(|v| v.to_string())
            .or_else(|| self.1.map(|v| format!("{v}.size")))
            .unwrap_or_default();

        ctx.builder.build_load(psize, var_name.as_str()).into_int_value()
    }
}

/// Proxy type for accessing the `data` array of an `list` instance in LLVM.
#[derive(Copy, Clone)]
pub struct ListDataProxy<'ctx>(ListValue<'ctx>);

impl<'ctx> ListDataProxy<'ctx> {
    /// Returns the single-indirection pointer to the array.
    pub fn get_ptr(&self, ctx: &CodeGenContext<'ctx, '_>) -> PointerValue<'ctx> {
        let var_name = self.0.1.map(|v| format!("{v}.data")).unwrap_or_default();

        ctx.builder.build_load(self.0.get_data_pptr(ctx), var_name.as_str()).into_pointer_value()
    }

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
            self.get_ptr(ctx),
            &[idx],
            var_name.as_str(),
        )
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
        );
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

    pub unsafe fn get_unchecked(
        &self,
        ctx: &mut CodeGenContext<'ctx, '_>,
        idx: IntValue<'ctx>,
        name: Option<&str>,
    ) -> BasicValueEnum<'ctx> {
        let ptr = self.ptr_offset_unchecked(ctx, idx, name);
        ctx.builder.build_load(ptr, name.unwrap_or_default())
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
        ctx.builder.build_load(ptr, name.unwrap_or_default())
    }
}
