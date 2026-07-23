#[cfg(feature = "malloc")]
use anyhow::bail;
#[cfg(feature = "malloc")]
use inkwell::types::BasicTypeEnum;
use inkwell::{
    builder::Builder,
    types::BasicType,
    values::{IntValue, PointerValue},
};

#[cfg(feature = "malloc")]
use crate::codegen::llvm_intrinsics::call_umul_with_overflow;
use crate::codegen::{CodeGenContext, types::ArraySliceValue};

/// The number of CTRC pages made available by `with critical():` when no page count is given.
///
/// One page holds `CTRC_CELLS_PER_PAGE` (31) cells of `CTRC_CELL_SIZE` (128) bytes each, so the
/// default guarantees 496 free objects / 64 KiB at block entry.
pub const CTRC_DEFAULT_RESERVED_PAGES: i32 = 16;

/// The scope where an allocation should take place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationScope {
    /// The default allocation strategy. Refer to the relevant function documentation for details.
    Default,

    /// Force allocation on the heap.
    #[cfg(feature = "malloc")]
    Heap,

    /// Force allocation at the start of the function stack frame.
    StackStartOfFunc,

    /// Force allocation at the current location in the stack frame.
    StackCurrentLoc,
}

impl AllocationScope {
    /// Returns the default allocation scope for a given type.
    ///
    /// The default allocation scope is [`AllocationScope::Heap`] for array and struct values, and
    /// [`AllocationScope::Stack`] for all other values. However, members of structs and arrays should
    /// always inherit the allocation scope of their parent, and should be handled separately.
    pub fn default_for_type<'ctx>(ty: &impl BasicType<'ctx>) -> Self {
        match ty.as_basic_type_enum() {
            #[cfg(feature = "malloc")]
            BasicTypeEnum::ArrayType(_) | BasicTypeEnum::StructType(_) => Self::Heap,
            _ => Self::StackStartOfFunc,
        }
    }
}

impl<'ctx> CodeGenContext<'ctx, '_> {
    /// Builds a call to `__nac3_alloc`, which allocates `size` bytes of memory with `align` bytes
    /// of alignment for an object.
    ///
    /// This function internally invokes the system allocator or the CTRC slab allocator depending
    /// on whether the current program allocator is in CTRC mode. If allocation fails for whatever
    /// reason (oversized object or unsatisfiable alignment in CTRC mode, or memory exhaustion),
    /// this function returns `null`, and the caller is expected to raise a `0:MemoryError` to
    /// signal the allocation failure.
    ///
    /// In CTRC mode, the maximum allocatable size is governed by a constant defined in IRRT - See
    /// `CTRC_CELL_SIZE`.
    ///
    /// The `align` argument is ignored when *not* in CTRC mode; `malloc`'ed objects are always
    /// aligned based on their allocation size.
    #[cfg(feature = "ctrc")]
    fn build_generalized_alloc(
        &self,
        b: &Builder<'ctx>,
        size: IntValue<'ctx>,
        align: u32,
        name: &str,
    ) -> anyhow::Result<PointerValue<'ctx>> {
        const FUNC_NAME: &str = "__nac3_alloc";

        let f = self.module.get_function(FUNC_NAME).unwrap_or_else(|| {
            self.module.add_function(
                FUNC_NAME,
                self.ptr.fn_type(&[self.size_t.into(), self.size_t.into()], false),
                None,
            )
        });
        let align = self.size_t.const_int(u64::from(align), false);
        Ok(b.build_call(f, &[size.into(), align.into()], name)?
            .try_as_basic_value()
            .basic()
            .unwrap()
            .into_pointer_value())
    }

    /// Builds the byte size of an allocation of `size` elements of type `ty`, guarding against
    /// `size_t` overflow of the product.
    ///
    /// A too-large element count would otherwise wrap around and under-allocate. Under CTRC this is
    /// especially dangerous: a logically huge array would be handed a single cell, and writing its
    /// elements corrupts neighboring live cells in the same page.
    ///
    /// A compile-time-known `size` is checked in Rust and returned as a folded constant, so no
    /// guard is emitted for allocations whose size is already known to fit.
    #[cfg(feature = "malloc")]
    fn build_checked_alloc_size(
        &mut self,
        size: IntValue<'ctx>,
        ty: impl BasicType<'ctx> + Copy,
    ) -> anyhow::Result<IntValue<'ctx>> {
        let sizeof_ty = self.sizeof(ty);

        if let Some(size) = size.get_zero_extended_constant() {
            let Some(nbytes) = size.checked_mul(sizeof_ty).filter(|n| *n <= self.size_t_max())
            else {
                bail!(
                    "Allocation of {size} elements of {sizeof_ty} bytes exceeds maximum value of {} bytes",
                    self.size_t_max(),
                );
            };

            return Ok(self.size_t.const_int(nbytes, false));
        }

        let sizeof_ty = self.size_t.const_int(sizeof_ty, false);
        let (nbytes, overflow) = call_umul_with_overflow(self, size, sizeof_ty, None)?;
        let no_overflow = self.builder.build_not(overflow, "")?;
        let size_zext = self.builder.build_int_z_extend_or_bit_cast(size, self.i64, "")?;
        let sizeof_ty_zext =
            self.builder.build_int_z_extend_or_bit_cast(sizeof_ty, self.i64, "")?;
        self.make_assert(
            no_overflow,
            "0:OverflowError",
            &format!(
                "Allocation of {{0}} elements of {{1}} bytes exceeds maximum value of {} bytes",
                self.size_t_max(),
            ),
            [Some(size_zext), Some(sizeof_ty_zext), None],
        )?;

        Ok(nbytes)
    }

    /// Builds an instruction which allocates memory for a value of type `ty`.
    ///
    /// The allocation is performed at [`AllocationScope::Default`]. See
    /// [`alloc_at`][CodeGenContext::alloc_at] for details on allocation scope.
    pub fn alloc(
        &self,
        ty: impl BasicType<'ctx>,
        name: Option<&str>,
    ) -> anyhow::Result<PointerValue<'ctx>> {
        self.alloc_at(AllocationScope::Default, ty, name)
    }

    fn alloc_builder(&self, scope: AllocationScope) -> &Builder<'ctx> {
        match scope {
            #[cfg(feature = "malloc")]
            AllocationScope::Heap | AllocationScope::StackCurrentLoc => &self.builder,
            #[cfg(not(feature = "malloc"))]
            AllocationScope::StackCurrentLoc => &self.builder,
            AllocationScope::StackStartOfFunc => {
                if let Some(loc) = self.builder.get_current_debug_location() {
                    self.init_builder.set_current_debug_location(loc);
                }
                &self.init_builder
            }
            AllocationScope::Default => unreachable!(),
        }
    }

    /// Builds an instruction which allocates memory for a value of type `ty`.
    ///
    /// The actual location of the allocation depends on `scope`:
    ///
    /// - [`AllocationScope::Default`]: Allocates on the stack for primitive types and on the heap
    ///   for composite types (arrays and structs). Always falls back to
    ///   `AllocationScope::StackStartOfFunc` if `malloc` feature is disabled.
    /// - [`AllocationScope::Heap`]: Allocates memory on the heap using `malloc`; Requires `malloc`
    ///   feature.
    /// - [`AllocationScope::StackStartOfFunc`]: Allocates memory on the stack using `alloca` at the
    ///   start of the function.
    /// - [`AllocationScope::StackCurrentLoc`]: Allocates memory on the stack using `alloca` at the
    ///   [current builder location][CodeGenContext::builder].
    pub fn alloc_at(
        &self,
        scope: AllocationScope,
        ty: impl BasicType<'ctx>,
        name: Option<&str>,
    ) -> anyhow::Result<PointerValue<'ctx>> {
        if scope == AllocationScope::Default {
            return self.alloc_at(AllocationScope::default_for_type(&ty), ty, name);
        }

        let b = self.alloc_builder(scope);
        let ty = ty.as_basic_type_enum();
        let ptr = match scope {
            #[cfg(all(feature = "malloc", not(feature = "ctrc")))]
            AllocationScope::Heap => b.build_malloc(ty, name.unwrap_or_default())?,
            #[cfg(feature = "ctrc")]
            AllocationScope::Heap => {
                let size = self.size_t.const_int(self.sizeof(ty), false);
                self.build_generalized_alloc(b, size, self.alignof(ty), name.unwrap_or_default())?
            }
            AllocationScope::StackStartOfFunc | AllocationScope::StackCurrentLoc => {
                b.build_alloca(ty, name.unwrap_or_default())?
            }
            AllocationScope::Default => unreachable!(),
        };
        if scope == AllocationScope::StackStartOfFunc && ty.is_pointer_type() {
            b.build_store(ptr, ty.const_zero())?;
        }
        Ok(ptr)
    }

    /// Builds an instruction which allocates memory for an array of type `ty` with a compile-time
    /// known size.
    ///
    /// The actual location of the allocation depends on `scope`:
    ///
    /// - [`AllocationScope::Default`]: Same as `AllocationScope::Heap` or
    ///   `Allocation::StackStartOfFunc` if `malloc` feature is enabled and disabled respectively.
    /// - [`AllocationScope::Heap`]: Allocates memory on the heap using `malloc`; Requires `malloc`
    ///   feature.
    /// - [`AllocationScope::StackStartOfFunc`]: Allocates memory on the stack using `alloca` at the
    ///   start of the function.
    /// - [`AllocationScope::StackCurrentLoc`]: Allocates memory on the stack using `alloca` at the
    ///   [current builder location][CodeGenContext::builder], similar to variable-length arrays in
    ///   C.
    pub fn alloc_array(
        &self,
        scope: AllocationScope,
        ty: impl BasicType<'ctx> + Copy,
        size: u64,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<ArraySliceValue<'ctx>> {
        if scope == AllocationScope::Default {
            #[cfg(feature = "malloc")]
            return self.alloc_array(AllocationScope::Heap, ty, size, name);
            #[cfg(not(feature = "malloc"))]
            return self.alloc_array(AllocationScope::StackStartOfFunc, ty, size, name);
        }

        // `size` is known at compile time here, so the allocation size is checked in Rust rather
        // than by emitting a runtime guard - An oversized allocation becomes a compile-time error.
        #[cfg(feature = "malloc")]
        if scope == AllocationScope::Heap {
            let sizeof_ty = self.sizeof(ty);
            let nbytes = size.checked_mul(sizeof_ty).filter(|n| *n <= self.size_t_max());
            if nbytes.is_none() {
                bail!(
                    "Allocation of {size} x {sizeof_ty} bytes exceeds maximum value of {} bytes",
                    self.size_t_max(),
                );
            }
        }

        let size = self.size_t.const_int(size, false);
        let b = self.alloc_builder(scope);
        let ty = ty.as_basic_type_enum();
        let ptr = match scope {
            #[cfg(all(feature = "malloc", not(feature = "ctrc")))]
            AllocationScope::Heap => b.build_array_malloc(
                ty,
                size,
                &name.map(|n| format!("{n}.malloc")).unwrap_or_default(),
            )?,
            #[cfg(feature = "ctrc")]
            AllocationScope::Heap => {
                let nbytes =
                    b.build_int_mul(size, self.size_t.const_int(self.sizeof(ty), false), "")?;
                self.build_generalized_alloc(
                    b,
                    nbytes,
                    self.alignof(ty),
                    &name.map(|n| format!("{n}.alloc")).unwrap_or_default(),
                )?
            }
            AllocationScope::StackStartOfFunc | AllocationScope::StackCurrentLoc => b
                .build_array_alloca(
                    ty,
                    size,
                    &name.map(|n| format!("{n}.alloca")).unwrap_or_default(),
                )?,
            AllocationScope::Default => unreachable!(),
        };
        Ok(ArraySliceValue::new(ty, ptr, size, name))
    }

    /// Builds an instruction which allocates memory for an array of type `ty` with a dynamic size.
    ///
    /// The actual location of the allocation depends on `scope`:
    ///
    /// - [`AllocationScope::Default`]: Same as `AllocationScope::Heap` or
    ///   `Allocation::StackCurrentLoc` if `malloc` feature is enabled and disabled respectively.
    /// - [`AllocationScope::Heap`]: Allocates memory on the heap using `malloc`; Requires `malloc`
    ///   feature.
    /// - [`AllocationScope::StackCurrentLoc`]: Allocates memory on the stack using `alloca` at the
    ///   [current builder location][CodeGenContext::builder], similar to variable-length arrays in
    ///   C.
    ///
    /// # Panics
    ///
    /// Panics if `scope` is `AllocationScope::StackStartOfFunc`, as dynamic arrays cannot be
    /// allocated at the start of a function.
    pub fn alloc_dyn_array(
        &mut self,
        scope: AllocationScope,
        ty: impl BasicType<'ctx> + Copy,
        size: IntValue<'ctx>,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<ArraySliceValue<'ctx>> {
        assert_ne!(
            scope,
            AllocationScope::StackStartOfFunc,
            "Cannot allocate dynamic array at the start of function"
        );

        if scope == AllocationScope::Default {
            #[cfg(feature = "malloc")]
            return self.alloc_dyn_array(AllocationScope::Heap, ty, size, name);
            #[cfg(not(feature = "malloc"))]
            return self.alloc_dyn_array(AllocationScope::StackStartOfFunc, ty, size, name);
        }

        // Note: Use `self` before `alloc_builder` borrows a builder out of it
        #[cfg(feature = "ctrc")]
        let nbytes = if scope == AllocationScope::Heap {
            let size = self.builder.build_int_truncate_or_bit_cast(size, self.size_t, "")?;
            Some(self.build_checked_alloc_size(size, ty)?)
        } else {
            None
        };
        // `build_array_malloc` recomputes the byte size itself, so only the guard is needed here.
        #[cfg(all(feature = "malloc", not(feature = "ctrc")))]
        if scope == AllocationScope::Heap {
            let size = self.builder.build_int_truncate_or_bit_cast(size, self.size_t, "")?;
            self.build_checked_alloc_size(size, ty)?;
        }

        let b = self.alloc_builder(scope);
        let ty = ty.as_basic_type_enum();
        let ptr = match scope {
            #[cfg(all(feature = "malloc", not(feature = "ctrc")))]
            AllocationScope::Heap => b.build_array_malloc(
                ty,
                size,
                &name.map(|n| format!("{n}.malloc")).unwrap_or_default(),
            )?,
            #[cfg(feature = "ctrc")]
            AllocationScope::Heap => self.build_generalized_alloc(
                b,
                nbytes.unwrap(),
                self.alignof(ty),
                &name.map(|n| format!("{n}.alloc")).unwrap_or_default(),
            )?,
            AllocationScope::StackCurrentLoc => b.build_array_alloca(
                ty,
                size,
                &name.map(|n| format!("{n}.alloca")).unwrap_or_default(),
            )?,
            AllocationScope::Default | AllocationScope::StackStartOfFunc => unreachable!(),
        };
        Ok(ArraySliceValue::new(ty, ptr, size, name))
    }
}
