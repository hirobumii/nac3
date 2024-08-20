use crate::{
    codegen::{
        model::*,
        object::{any::AnyObject, list::ListObject, tuple::TupleObject},
        CodeGenContext, CodeGenerator,
    },
    typecheck::typedef::TypeEnum,
};
use util::gen_for_model;

/// Parse a NumPy-like "int sequence" input and return the int sequence as an array and its length.
///
/// * `sequence` - The `sequence` parameter.
/// * `sequence_ty` - The typechecker type of `sequence`
///
/// The `sequence` argument type may only be one of the following:
///   1. A list of `int32`;   e.g., `np.empty([600, 800, 3])`
///   2. A tuple of `int32`;  e.g., `np.empty((600, 800, 3))`
///   3. A scalar `int32`;    e.g., `np.empty(3)`, this is functionally equivalent to `np.empty([3])`
///
/// All `int32` values will be sign-extended to `SizeT`.
pub fn parse_numpy_int_sequence<'ctx, G: CodeGenerator + ?Sized>(
    generator: &mut G,
    ctx: &mut CodeGenContext<'ctx, '_>,
    input_sequence: AnyObject<'ctx>,
) -> (Instance<'ctx, Int<SizeT>>, Instance<'ctx, Ptr<Int<SizeT>>>) {
    let zero = Int(SizeT).const_0(generator, ctx.ctx);
    let one = Int(SizeT).const_1(generator, ctx.ctx);

    // The result `list` to return.
    match &*ctx.unifier.get_ty(input_sequence.ty) {
        TypeEnum::TObj { obj_id, .. }
            if *obj_id == ctx.primitives.list.obj_id(&ctx.unifier).unwrap() =>
        {
            // 1. A list of `int32`; e.g., `np.empty([600, 800, 3])`

            // Check `input_sequence`
            let input_sequence = ListObject::from_object(generator, ctx, input_sequence);

            let len = input_sequence.instance.get(generator, ctx, |f| f.len);
            let result = Int(SizeT).array_alloca(generator, ctx, len.value);

            // Load all the `int32`s from the input_sequence, cast them to `SizeT`, and store them into `result`
            gen_for_model(generator, ctx, zero, len, one, |generator, ctx, _hooks, i| {
                // Load the i-th int32 in the input sequence
                let int = input_sequence
                    .instance
                    .get(generator, ctx, |f| f.items)
                    .get_index(generator, ctx, i.value)
                    .value
                    .into_int_value();

                // Cast to SizeT
                let int = Int(SizeT).s_extend_or_bit_cast(generator, ctx, int);

                // Store
                result.set_index(ctx, i.value, int);

                Ok(())
            })
            .unwrap();

            (len, result)
        }
        TypeEnum::TTuple { .. } => {
            // 2. A tuple of ints; e.g., `np.empty((600, 800, 3))`

            let input_sequence = TupleObject::from_object(ctx, input_sequence);

            let len = input_sequence.len(generator, ctx);

            let result = Int(SizeT).array_alloca(generator, ctx, len.value);

            for i in 0..input_sequence.num_elements() {
                // Get the i-th element off of the tuple and load it into `result`.
                let int = input_sequence.index(ctx, i).value.into_int_value();
                let int = Int(SizeT).s_extend_or_bit_cast(generator, ctx, int);

                result.set_index_const(ctx, i64::try_from(i).unwrap(), int);
            }

            (len, result)
        }
        TypeEnum::TObj { obj_id, .. }
            if *obj_id == ctx.primitives.int32.obj_id(&ctx.unifier).unwrap() =>
        {
            // 3. A scalar int; e.g., `np.empty(3)`, this is functionally equivalent to `np.empty([3])`
            let input_int = input_sequence.value.into_int_value();

            let len = Int(SizeT).const_1(generator, ctx.ctx);
            let result = Int(SizeT).array_alloca(generator, ctx, len.value);
            let int = Int(SizeT).s_extend_or_bit_cast(generator, ctx, input_int);

            // Storing into result[0]
            result.store(ctx, int);

            (len, result)
        }
        _ => panic!(
            "encountered unknown sequence type: {}",
            ctx.unifier.stringify(input_sequence.ty)
        ),
    }
}
