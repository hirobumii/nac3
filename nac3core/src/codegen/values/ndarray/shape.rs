use inkwell::values::{BasicValueEnum, IntValue};

use crate::{
    codegen::{
        stmt::gen_for_callback_incrementing,
        types::{ListType, TupleType},
        values::{
            ArraySliceValue, ProxyValue, TypedArrayLikeAccessor, TypedArrayLikeAdapter,
            TypedArrayLikeMutator, UntypedArrayLikeAccessor,
        },
        CodeGenContext, CodeGenerator,
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
pub fn parse_numpy_int_sequence<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    (input_seq_ty, input_seq): (Type, BasicValueEnum<'ctx>),
) -> impl TypedArrayLikeAccessor<'ctx, G, IntValue<'ctx>> {
    let llvm_usize = ctx.get_size_type();
    let zero = llvm_usize.const_zero();
    let one = llvm_usize.const_int(1, false);

    // The result `list` to return.
    match &*ctx.unifier.get_ty_immutable(input_seq_ty) {
        TypeEnum::TObj { obj_id, .. }
            if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
        {
            // 1. A list of `int32`; e.g., `np.empty([600, 800, 3])`

            let input_seq = ListType::from_unifier_type(generator, ctx, input_seq_ty)
                .map_pointer_value(input_seq.into_pointer_value(), None);

            let len = input_seq.load_size(ctx, None);
            // TODO: Find a way to remove this mid-BB allocation
            let result = ctx.builder.build_array_alloca(llvm_usize, len, "").unwrap();
            let result = TypedArrayLikeAdapter::from(
                ArraySliceValue::from_ptr_val(result, len, None),
                |_, _, val| val.into_int_value(),
                |_, _, val| val.into(),
            );

            // Load all the `int32`s from the input_sequence, cast them to `SizeT`, and store them into `result`
            gen_for_callback_incrementing(
                generator,
                ctx,
                None,
                zero,
                (len, false),
                |generator, ctx, _, i| {
                    // Load the i-th int32 in the input sequence
                    let int = unsafe {
                        input_seq.data().get_unchecked(ctx, generator, &i, None).into_int_value()
                    };

                    // Cast to SizeT
                    let int =
                        ctx.builder.build_int_s_extend_or_bit_cast(int, llvm_usize, "").unwrap();

                    // Store
                    unsafe { result.set_typed_unchecked(ctx, generator, &i, int) };

                    Ok(())
                },
                one,
            )
            .unwrap();

            result
        }

        TypeEnum::TTuple { .. } => {
            // 2. A tuple of ints; e.g., `np.empty((600, 800, 3))`

            let input_seq = TupleType::from_unifier_type(generator, ctx, input_seq_ty)
                .map_struct_value(input_seq.into_struct_value(), None);

            let len = input_seq.get_type().num_elements();

            let result = generator
                .gen_array_var_alloc(
                    ctx,
                    llvm_usize.into(),
                    llvm_usize.const_int(u64::from(len), false),
                    None,
                )
                .unwrap();
            let result = TypedArrayLikeAdapter::from(
                result,
                |_, _, val| val.into_int_value(),
                |_, _, val| val.into(),
            );

            for i in 0..input_seq.get_type().num_elements() {
                // Get the i-th element off of the tuple and load it into `result`.
                let int = input_seq.load_element(ctx, i).into_int_value();
                let int = ctx.builder.build_int_s_extend_or_bit_cast(int, llvm_usize, "").unwrap();

                unsafe {
                    result.set_typed_unchecked(
                        ctx,
                        generator,
                        &llvm_usize.const_int(u64::from(i), false),
                        int,
                    );
                }
            }

            result
        }

        TypeEnum::TObj { obj_id, .. }
            if *obj_id == ctx.primitives.int32.obj_id(&ctx.unifier).unwrap() =>
        {
            // 3. A scalar int; e.g., `np.empty(3)`, this is functionally equivalent to `np.empty([3])`

            let input_int = input_seq.into_int_value();

            let len = one;
            let result = generator.gen_array_var_alloc(ctx, llvm_usize.into(), len, None).unwrap();
            let result = TypedArrayLikeAdapter::from(
                result,
                |_, _, val| val.into_int_value(),
                |_, _, val| val.into(),
            );
            let int =
                ctx.builder.build_int_s_extend_or_bit_cast(input_int, llvm_usize, "").unwrap();

            // Storing into result[0]
            unsafe {
                result.set_typed_unchecked(ctx, generator, &zero, int);
            }

            result
        }

        _ => panic!("encountered unknown sequence type: {}", ctx.unifier.stringify(input_seq_ty)),
    }
}
