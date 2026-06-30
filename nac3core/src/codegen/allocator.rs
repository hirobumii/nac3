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
    /// Helper function that builds an allocation instruction by calling `build_alloc_instr_fn` at
    /// the current builder location if `late` is `true`, or the start of the function otherwise.
    fn build_allocate_impl<T>(
        &self,
        late: bool,
        build_alloc_instr_fn: impl FnOnce(&Builder<'ctx>) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        // Restore debug location
        let di_loc = self.debug_info.0.create_debug_location(
            self.ctx,
            self.current_loc.row as u32,
            self.current_loc.column as u32,
            self.debug_info.2,
            None,
        );

        let new_builder;
        let builder = if late {
            self.builder
        } else {
            new_builder = self.ctx.create_builder();
            // position before the last branching instruction...
            new_builder.position_before(&self.init_bb.get_last_instruction().unwrap());
            &new_builder
        };

        builder.set_current_debug_location(di_loc);
        build_alloc_instr_fn(builder)
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
    ///   [`init_bb` basic block][BasicBlock].
    /// - [`AllocationScope::StackCurrentLoc`]: Allocates memory on the stack using `alloca` at the
    ///   [current builder location][CodeGenContext::builder].
    pub fn build_allocate(
        &self,
        scope: AllocationScope,
        ty: impl BasicType<'ctx>,
        name: Option<&str>,
    ) -> anyhow::Result<PointerValue<'ctx>> {
        if scope == AllocationScope::Default {
            return self.build_allocate(AllocationScope::default_for_type(&ty), ty, name);
        }

        let ty = ty.as_basic_type_enum();
        #[cfg(feature = "malloc")]
        let late = matches!(scope, AllocationScope::Heap | AllocationScope::StackCurrentLoc);
        #[cfg(not(feature = "malloc"))]
        let late = matches!(scope, AllocationScope::StackCurrentLoc);

        self.build_allocate_impl(late, |b| {
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
        })
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
    ///   [`init_bb` basic block][BasicBlock].
    /// - [`AllocationScope::StackCurrentLoc`]: Allocates memory on the stack using `alloca` at the
    ///   [current builder location][CodeGenContext::builder], similar to variable-length arrays in
    ///   C.
    pub fn build_array_allocate(
        &self,
        scope: AllocationScope,
        ty: impl BasicType<'ctx> + Copy,
        size: u64,
        name: Option<&'ctx str>,
    ) -> anyhow::Result<ArraySliceValue<'ctx>> {
        if scope == AllocationScope::Default {
            #[cfg(feature = "malloc")]
            return self.build_array_allocate(AllocationScope::Heap, ty, size, name);
            #[cfg(not(feature = "malloc"))]
            return self.build_array_allocate(AllocationScope::StackStartOfFunc, ty, size, name);
        }

        let size = self.size_t.const_int(size, false);
        let ty = ty.as_basic_type_enum();
        #[cfg(feature = "malloc")]
        let late = matches!(scope, AllocationScope::Heap | AllocationScope::StackCurrentLoc);
        #[cfg(not(feature = "malloc"))]
        let late = matches!(scope, AllocationScope::StackCurrentLoc);
        let ptr = self.build_allocate_impl(late, |b| {
            Ok(match scope {
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
            })
        })?;
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
    pub fn build_dyn_array_allocate(
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
            return self.build_dyn_array_allocate(AllocationScope::Heap, ty, size, name);
            #[cfg(not(feature = "malloc"))]
            return self.build_dyn_array_allocate(
                AllocationScope::StackStartOfFunc,
                ty,
                size,
                name,
            );
        }

        let ty = ty.as_basic_type_enum();
        #[cfg(feature = "malloc")]
        let late = matches!(scope, AllocationScope::Heap | AllocationScope::StackCurrentLoc);
        #[cfg(not(feature = "malloc"))]
        let late = matches!(scope, AllocationScope::StackCurrentLoc);
        let ptr = self.build_allocate_impl(late, |b| {
            Ok(match scope {
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
            })
        })?;
        Ok(ArraySliceValue::new(ty, ptr, size, name))
    }
}
