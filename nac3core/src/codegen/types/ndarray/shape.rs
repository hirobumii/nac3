use inkwell::values::{BasicValueEnum, IntValue};

use crate::{
    codegen::{
        CodeGenContext,
        allocator::AllocationScope,
        stmt::gen_for_callback_incrementing,
        types::{
            ArrayLikeIndexer, ArraySliceValue, ListType, ProxyTypeBase, TupleType,
            TypedRefCountedType, field,
        },
    },
    typecheck::typedef::{Type, TypeEnum},
};

/// Parse a NumPy-like "int sequence" input and return the int sequence as an array and its length.
///
/// * `sequence` - The `sequence` parameter.
/// * `sequence_ty` - The typechecker type of `sequence`
///
/// The `sequence` argument type may only be one of the following:
///   1. A list of `int32`;   e.g., `np.empty([600, 800, 3])`
///   2. A tuple of `int32`;  e.g., `np.empty((600, 800, 3))`
///   3. A scalar `int32`;    e.g., `np.empty(3)`, this is functionally equivalent to
///      `np.empty([3])`
///
/// All `int32` values will be sign-extended to `SizeT`.
pub fn parse_numpy_int_sequence<'ctx>(
    ctx: &mut CodeGenContext<'ctx, '_>,
    (input_seq_ty, input_seq): (Type, BasicValueEnum<'ctx>),
) -> anyhow::Result<ArraySliceValue<'ctx>> {
    let llvm_usize = ctx.size_t;
    let zero = llvm_usize.const_zero();
    let one = llvm_usize.const_int(1, false);

    // The result `list` to return.
    Ok(match &*ctx.unifier.get_ty_immutable(input_seq_ty) {
        TypeEnum::TObj { obj_id, .. }
            if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
        {
            // 1. A list of `int32`; e.g., `np.empty([600, 800, 3])`

            let llvm_list_ty = ListType::from_unifier_type(ctx, input_seq_ty);
            let input_seq = TypedRefCountedType::new(ctx, llvm_list_ty)
                .map_value(input_seq.into_pointer_value(), None);

            let len = input_seq.inner_value(ctx)?.load(ctx, field!(len))?;
            // TODO: Find a way to remove this mid-BB allocation
            let result = {
                #[cfg(feature = "malloc")]
                let scope = AllocationScope::Heap;
                #[cfg(not(feature = "malloc"))]
                let scope = AllocationScope::StackCurrentLoc;
                ctx.build_dyn_array_allocate(scope, llvm_usize, len, None)?
            };

            // Load all the `int32`s from the input_sequence, cast them to `SizeT`, and store them into `result`
            gen_for_callback_incrementing(
                &mut (),
                ctx,
                None,
                zero,
                (len, false),
                |(), ctx, _, i| {
                    // Load the i-th int32 in the input sequence
                    let int: IntValue<'ctx> =
                        input_seq.inner_value(ctx)?.data(ctx)?.get_unchecked(ctx, &i, None)?;

                    // Cast to SizeT
                    let int = ctx.builder.build_int_s_extend_or_bit_cast(int, llvm_usize, "")?;

                    // Store
                    result.set_unchecked(ctx, &i, int, None)?;

                    Ok(())
                },
                one,
                |(), _| Ok(()),
            )?;

            result
        }

        TypeEnum::TTuple { .. } => {
            // 2. A tuple of ints; e.g., `np.empty((600, 800, 3))`

            let input_seq = TupleType::from_unifier_type(ctx, input_seq_ty)
                .map_value(input_seq.into_struct_value(), None);

            let len = input_seq.ty.num_elements();

            let result = ctx.build_array_allocate(
                AllocationScope::Default,
                llvm_usize,
                u64::from(len),
                None,
            )?;

            for i in 0..input_seq.ty.num_elements() {
                // Get the i-th element off of the tuple and load it into `result`.
                let int = input_seq.extract(ctx, i)?.into_int_value();
                let int = ctx.builder.build_int_s_extend_or_bit_cast(int, llvm_usize, "")?;

                result.set_unchecked(ctx, &llvm_usize.const_int(u64::from(i), false), int, None)?;
            }

            result
        }

        TypeEnum::TObj { obj_id, .. }
            if *obj_id == ctx.primitives.int32.obj_id(&ctx.unifier).unwrap() =>
        {
            // 3. A scalar int; e.g., `np.empty(3)`, this is functionally equivalent to `np.empty([3])`

            let input_int = input_seq.into_int_value();

            let result = ctx.build_array_allocate(AllocationScope::Default, llvm_usize, 1, None)?;
            let int = ctx.builder.build_int_s_extend_or_bit_cast(input_int, llvm_usize, "")?;

            // Storing into result[0]
            result.set_unchecked(ctx, &zero, int, None)?;
            result
        }

        _ => panic!("encountered unknown sequence type: {}", ctx.unifier.stringify(input_seq_ty)),
    })
}
