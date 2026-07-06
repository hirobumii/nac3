#[cfg(feature = "malloc")]
use inkwell::types::BasicTypeEnum;
use inkwell::{
    builder::Builder,
    types::BasicType,
    values::{IntValue, PointerValue},
};

use crate::codegen::{CodeGenContext, types::ArraySliceValue};

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
            #[cfg(feature = "malloc")]
            AllocationScope::Heap => b.build_malloc(ty, name.unwrap_or_default())?,
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

        let size = self.size_t.const_int(size, false);
        let b = self.alloc_builder(scope);
        let ty = ty.as_basic_type_enum();
        let ptr = match scope {
            #[cfg(feature = "malloc")]
            AllocationScope::Heap => b.build_array_malloc(
                ty,
                size,
                &name.map(|n| format!("{n}.malloc")).unwrap_or_default(),
            )?,
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
        &self,
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

        let b = self.alloc_builder(scope);
        let ty = ty.as_basic_type_enum();
        let ptr = match scope {
            #[cfg(feature = "malloc")]
            AllocationScope::Heap => b.build_array_malloc(
                ty,
                size,
                &name.map(|n| format!("{n}.malloc")).unwrap_or_default(),
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
